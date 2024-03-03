use core::panic;
use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fs::{self, File},
    io::{Read, Stdin, Write},
    path::{Path, PathBuf},
    process::Stdio,
    rc::Rc,
    vec,
};

use minijinja::{
    value::{self, Kwargs, StructObject},
    Environment,
};
use serde::{Deserialize, Serialize};

use crate::{
    Component, ComponentId, Define, Feature, IncludeEntry, IntermediateTemplateContribution,
    Library, Parent, Require, ResolvedDefine, ResolvedIncludeEntry, ResolvedLibrary,
    ResolvedSourceFile, ResolvedTemplateFile, ResolvedWithParent, SDKId, Satisfied, SourceFile,
    TemplateContribution, TemplateFile, WithRootPath, SDK,
};

#[derive(Debug, Clone, Deserialize)]
pub struct ProjectRaw {
    pub project_name: String,
    pub sdk: SDKId,
    pub source: Option<Vec<SourceFile>>,
    pub include: Option<Vec<IncludeEntry>>,
    pub component: Option<Vec<ComponentId>>,
    pub define: Option<Vec<Define>>,
    pub requires: Option<Vec<Require>>,
    pub provides: Option<Vec<Feature>>,
    pub library: Option<Vec<Library>>,
    pub template_contribution: Option<Vec<TemplateContribution>>,
}

#[derive(Debug, Clone)]
pub struct Project {
    pub project_name: String,
    pub sdk: SDKId,
    pub root_path: PathBuf,
    pub source: Option<Vec<SourceFile>>,
    pub include: Option<Vec<IncludeEntry>>,
    pub component: Option<Vec<ComponentId>>,
    pub define: Option<Vec<Define>>,
    pub requires: Option<Vec<Require>>,
    pub provides: Option<Vec<Feature>>,
    pub library: Option<Vec<Library>>,
    pub template_contribution: Option<Vec<TemplateContribution>>,
}

impl Project {
    pub fn parse(path: impl AsRef<Path>) -> Result<Project, Box<dyn Error>> {
        let path = path.as_ref();
        let mut file = File::open(path)?;
        let mut data = String::new();
        file.read_to_string(&mut data)?;

        let raw: ProjectRaw = serde_yaml::from_str(&data)?;
        let root_path = path.parent().unwrap().to_path_buf();

        let ret = Self {
            project_name: raw.project_name,
            sdk: raw.sdk,
            root_path,
            source: raw.source,
            include: raw.include,
            component: raw.component,
            define: raw.define,
            requires: raw.requires,
            provides: raw.provides,
            library: raw.library,
            template_contribution: raw.template_contribution,
        };

        Ok(ret)
    }

    pub fn resolve_components(&self, sdk: &SDK) -> ResolveResult {
        // https://siliconlabs.github.io/slc-specification/1.0/features/#dependency-resolution

        let mut components: Vec<Rc<Component>> = Vec::new();

        if let Some(ref comp_ids) = self.component {
            for id in comp_ids {
                let c = sdk
                    .components()
                    .iter()
                    .find(|c| c.id == id.id)
                    .expect(format!("unknown component {}", id.id).as_str());

                components.push(c.clone());
            }
        }

        let mut project_requires: BTreeSet<String> = BTreeSet::new();
        let mut project_provides: BTreeSet<String> = BTreeSet::new();

        if let Some(ref provides) = self.provides {
            for prov in provides {
                project_requires.insert(prov.name.clone());
            }
        }

        if let Some(ref requires) = self.requires {
            for req in requires {
                project_provides.insert(req.name.clone());
            }
        }

        loop {
            let mut required_features: BTreeSet<String> =
                BTreeSet::from_iter(project_requires.iter().cloned());
            let mut provided_features: BTreeSet<String> =
                BTreeSet::from_iter(project_provides.iter().cloned());
            let mut conflicts: BTreeSet<String> = BTreeSet::new();

            let mut added_component = false;
            let mut had_multiple_candidates = false;

            // Compute the sets of required, provided, and conflicting features.
            loop {
                // Keep adding until there's nothing new to add
                let additional_provides: Vec<_> = components
                    .iter()
                    .filter_map(|c: &Rc<Component>| c.provides.as_ref())
                    .flat_map(|f| {
                        f.iter()
                            .filter(|f| {
                                !provided_features.contains(&f.name)
                                    && f.satisfied(&provided_features)
                            })
                            .map(|f| f.name.to_string())
                    })
                    .collect();
                if additional_provides.is_empty() {
                    break;
                }
                provided_features.extend(additional_provides.into_iter());
            }

            required_features.extend(
                components
                    .iter()
                    .filter_map(|c: &Rc<Component>| c.requires.as_ref())
                    .flat_map(|f| {
                        f.iter()
                            .filter(|f| f.satisfied(&provided_features))
                            .map(|f| f.name.to_string())
                    }),
            );

            conflicts.extend(
                components
                    .iter()
                    .filter_map(|c: &Rc<Component>| c.conflicts.as_ref())
                    .flat_map(|f| {
                        f.iter()
                            .filter(|f| f.satisfied(&provided_features))
                            .map(|f| f.name.to_string())
                    }),
            );

            let unsatisfied = BTreeSet::from_iter(required_features.difference(&provided_features));
            if unsatisfied.is_empty() {
                if !conflicts.is_disjoint(&provided_features) {
                    panic!("Dependency resolution failed: conflicting features");
                }
                // We're done
                return ResolveResult {
                    components,
                    provided_features,
                };
            }

            let required_and_provided =
                BTreeSet::from_iter(required_features.union(&provided_features).cloned());

            for req in unsatisfied.iter() {
                // Find components...
                let candidates: Vec<_> = sdk
                    .components()
                    .iter()
                    // ... that provide the unsatisfied requirement
                    .filter(|c| {
                        let Some(provides) = &c.provides else {
                            return false;
                        };
                        provides
                            .iter()
                            .any(|f| &&f.name == req && f.satisfied(&required_and_provided))
                    })
                    // ... while not causing one of the known conflicts
                    .filter(|c| {
                        let Some(conflicts) = &c.conflicts else {
                            return true;
                        };
                        conflicts
                            .iter()
                            .all(|c| !c.satisfied(&required_and_provided))
                    })
                    .collect();

                if candidates.len() == 1 {
                    let c = *candidates.first().unwrap();
                    components.push(c.clone());
                    added_component = true;
                } else if candidates.len() > 1 {
                    had_multiple_candidates = true;
                }
            }

            // FIXME: Ensure that No two components provide the same feature (unless the allow_multiple flag is set on all instances of the provide)

            if !added_component {
                if had_multiple_candidates {
                    // Components may use the recommends key to recommend specific components by id. A recommended component is only considered
                    // for inclusion in the project if it provides a feature in the set of unsatisfied requirements U, and if its provided features
                    // are disjoint from those of all other recommended components.

                    // Given the list of recommended components considered for inclusion, only one recommendation is added to C at a time, re-starting
                    // the dependency resolution process after each addition. If multiple candidate components are considered for inclusion (each
                    // satisfying a different requirement), the first candiate component as sorted alphabetically by id is always selected.
                    let mut recommendation_ids: Vec<_> = components
                        .iter()
                        .filter_map(|c| c.recommends.as_ref())
                        .flatten()
                        .filter_map(|r| {
                            if r.satisfied(&provided_features) {
                                Some(r.id.clone())
                            } else {
                                None
                            }
                        })
                        .collect();
                    recommendation_ids.sort_by(|a, b| a.cmp(b));

                    let recommended_components: Vec<_> = recommendation_ids
                        .into_iter()
                        .map(|id| {
                            sdk.components()
                                .iter()
                                .find(|c| c.id == id)
                                .expect(format!("unknown component {}", id).as_str())
                        })
                        // Filter only components that satisfy at least one requirement
                        .filter(|c| {
                            let Some(ref provides) = c.provides else {
                                return false;
                            };
                            provides.iter().any(|f| {
                                f.satisfied(&provided_features)
                                    && !conflicts.contains(&f.name)
                                    && unsatisfied.contains(&f.name)
                            })
                        })
                        // and extract their feature set
                        .map(|c| {
                            let feature_set =
                                BTreeSet::from_iter(c.provides.iter().flat_map(|p| {
                                    p.iter()
                                        .filter(|f| f.satisfied(&provided_features))
                                        .map(|f| f.name.clone())
                                }));
                            (c, feature_set)
                        })
                        .collect();

                    // eprintln!("recommendations:");
                    // for r in &recommended_components {
                    //     eprintln!("  {}", r.0.id);
                    // }

                    // Find recommendations that don't conflict with any of the other recommendations
                    let non_conflicting_recommendations = recommended_components
                        .iter()
                        .enumerate()
                        .filter(|(i, (_, f))| {
                            recommended_components
                                .iter()
                                .enumerate()
                                .all(|(j, (_, g))| *i == j || f.is_disjoint(g))
                        })
                        .map(|(_, (c, _))| c)
                        .collect::<Vec<_>>();

                    // We found at least one. Take the first and continue
                    if non_conflicting_recommendations.len() >= 1 {
                        let c = **non_conflicting_recommendations.first().unwrap();
                        components.push(c.clone());
                        continue;
                    }
                }
                // eprintln!("provided:");
                // for p in &provided_features {
                //     eprintln!("  {}", p);
                // }
                // eprintln!("unsatisfied:");
                // for u in &unsatisfied {
                //     eprintln!("  {}", u);
                // }
                // eprintln!("recommendations:");
                // for c in &components {
                //     if let Some(ref recs) = c.recommends {
                //         for r in recs {
                //             eprintln!("  {}", r.id);
                //         }
                //     }
                // }

                panic!(
                    "Dependency resolution failed: no component found to satisfy open requirements"
                );
            }
        }
    }
}

pub struct ResolveResult {
    pub components: Vec<Rc<Component>>,
    pub provided_features: BTreeSet<String>,
}

#[derive(Debug)]
pub struct ParsedProject {
    pub source: Vec<ResolvedSourceFile>,
    pub include: Vec<ResolvedIncludeEntry>,
    pub define: Vec<ResolvedDefine>,
    pub library: Vec<ResolvedLibrary>,
    pub template_file: Vec<ResolvedTemplateFile>,
    pub template_contribution: BTreeMap<String, Vec<minijinja::Value>>,
    pub root_path: PathBuf,
    pub sdk_root_path: PathBuf,
    pub provided_features: BTreeSet<String>,
}

impl ParsedProject {
    pub fn new(sdk: &SDK, project: &Project, resolved: &ResolveResult) -> Self {
        let mut source: Vec<ResolvedSourceFile> = Vec::new();
        let mut include: Vec<ResolvedIncludeEntry> = Vec::new();
        let mut define: Vec<ResolvedDefine> = Vec::new();
        let mut library: Vec<ResolvedLibrary> = Vec::new();
        let mut template_file: Vec<ResolvedTemplateFile> = Vec::new();
        let mut template_contribution: BTreeMap<String, Vec<IntermediateTemplateContribution>> =
            BTreeMap::new();

        let all_features = &resolved.provided_features;

        if let Some(s) = &project.source {
            source.extend(s.iter().filter_map(|e| {
                if e.satisfied(all_features) {
                    Some(e.resolved(Parent::Project))
                    // let relative = e.relative_to(&project.root_path);
                    // Some((&relative).into())
                } else {
                    None
                }
            }));
        }

        if let Some(i) = &project.include {
            include.extend(i.iter().filter_map(|e| {
                if e.satisfied(all_features) {
                    let file_list = e.file_list.as_ref().map(|list| {
                        list.iter()
                            .filter_map(|h| {
                                if h.satisfied(all_features) {
                                    Some(h.into())
                                } else {
                                    None
                                }
                            })
                            .collect()
                    });
                    Some(ResolvedIncludeEntry {
                        path: e.path.clone(),
                        parent: Parent::Project,
                        directory: e.directory.clone(),
                        file_list,
                    })
                } else {
                    None
                }
            }));
        }

        if let Some(d) = &project.define {
            define.extend(d.iter().filter_map(|e| {
                if e.satisfied(all_features) {
                    Some(e.into())
                } else {
                    None
                }
            }));
        }

        if let Some(l) = &project.library {
            library.extend(l.iter().filter_map(|e| {
                if e.satisfied(all_features) {
                    Some(e.into())
                } else {
                    None
                }
            }));
        }

        if let Some(contrib) = &project.template_contribution {
            for t in contrib.iter().filter_map(|e| {
                if e.satisfied(all_features) {
                    Some(Into::<IntermediateTemplateContribution>::into(e))
                } else {
                    None
                }
            }) {
                template_contribution
                    .entry(t.name.clone())
                    .and_modify(|v| {
                        v.push(t.clone());
                        v.sort_by_key(|e| e.priority.unwrap_or(0));
                    })
                    .or_insert_with(|| vec![t]);
            }
        }

        for comp in &resolved.components {
            if let Some(s) = &comp.source {
                source.extend(s.iter().filter_map(|e: &SourceFile| {
                    if e.satisfied(all_features) {
                        Some(e.with_root_path(&comp.root_path).resolved(Parent::SDK))
                    // let relative = e.relative_to(&comp.root_path);
                    // Some((&relative).into())
                    } else {
                        None
                    }
                }));
            }

            if let Some(i) = &comp.include {
                include.extend(i.iter().filter_map(|e: &IncludeEntry| {
                    if e.satisfied(all_features) {
                        let file_list = e.file_list.as_ref().map(|list| {
                            list.iter()
                                .filter_map(|h| {
                                    if h.satisfied(all_features) {
                                        Some(h.into())
                                    } else {
                                        None
                                    }
                                })
                                .collect()
                        });
                        let e = e.with_root_path(&comp.root_path);
                        Some(ResolvedIncludeEntry {
                            path: e.path.clone(),
                            parent: Parent::SDK,
                            directory: e.directory.clone(),
                            file_list,
                        })
                    } else {
                        None
                    }
                }));
            }

            if let Some(d) = &comp.define {
                define.extend(d.iter().filter_map(|e: &Define| {
                    if e.satisfied(all_features) {
                        Some(e.into())
                    } else {
                        None
                    }
                }));
            }

            if let Some(l) = &comp.library {
                library.extend(l.iter().filter_map(|e: &Library| {
                    if e.satisfied(all_features) {
                        Some(e.into())
                    } else {
                        None
                    }
                }));
            }

            if let Some(t) = &comp.template_file {
                template_file.extend(t.iter().filter_map(|e: &TemplateFile| {
                    if e.satisfied(all_features) {
                        Some(e.with_root_path(&comp.root_path).resolved(Parent::SDK))
                    } else {
                        None
                    }
                }));
            }

            if let Some(contrib) = &comp.template_contribution {
                for t in contrib.iter().filter_map(|e: &TemplateContribution| {
                    if e.satisfied(all_features) {
                        Some(Into::<IntermediateTemplateContribution>::into(e))
                    } else {
                        None
                    }
                }) {
                    template_contribution
                        .entry(t.name.clone())
                        .and_modify(|v| {
                            v.push(t.clone());
                            v.sort_by_key(|e| e.priority.unwrap_or(0));
                        })
                        .or_insert_with(|| vec![t]);
                }
            }
        }

        let template_contribution: BTreeMap<String, Vec<minijinja::Value>> = template_contribution
            .into_iter()
            .map(|(k, v)| {
                let values: Vec<_> = v.into_iter().map(|t| t.value).collect();
                (k, values)
            })
            .collect();

        Self {
            root_path: project.root_path.clone(),
            sdk_root_path: sdk.root_path.clone(),
            source,
            include,
            define,
            library,
            template_file,
            template_contribution,
            provided_features: resolved.provided_features.clone(),
        }
    }

    pub fn generate_templates(&self, out_dir: impl AsRef<Path>) -> Result<(), Box<dyn Error>> {
        let vars = serde_yaml::to_string(&self.template_contribution)?;
        let script = include_str!("render_template.py");

        let out_dir = out_dir.as_ref();

        for template in &self.template_file {
            let root_path = match template.parent {
                Parent::Project => &self.root_path,
                Parent::SDK => &self.sdk_root_path,
            };
            let template_path = root_path.join(&template.path);
            let out_filename = if let Some(n) = template.path.strip_suffix(".jinja") {
                n
            } else if let Some(n) = template.path.strip_suffix(".jinja2") {
                n
            } else {
                panic!("template file must have .jinja or .jinja2 extension")
            };
            let out_path = out_dir.join(out_filename);

            // read the file at template_path into a string
            let mut file = File::open(&template_path)?;
            let mut template_content = String::new();
            file.read_to_string(&mut template_content)?;

            // Use python to render the template
            let mut python = std::process::Command::new("python3")
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .arg("-")
                .env("VARS", vars.clone())
                .env("TEMPLATE", template_content)
                .spawn()
                .expect("failed to spawn python3");

            // Write embedded script to python stdin
            let mut stdin = python.stdin.take().expect("Failed to open stdin");
            stdin
                .write_all(script.as_bytes())
                .expect("failed to write to stdin");
            drop(stdin);

            // And capture the output, so we can write it to a file
            let output = python.wait_with_output().expect("failed to read output");
            if !output.status.success() {
                panic!(
                    "template generator returned error:{}",
                    String::from_utf8_lossy(&output.stderr)
                );
            }
            let output = output.stdout;

            fs::create_dir_all(out_path.parent().unwrap())?;
            fs::write(out_path, output)?;
        }

        Ok(())
    }
}
