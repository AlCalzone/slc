use crate::{
    Component, ComponentId, ConfigFile, ConfigFileOverride, Define, Feature, IncludeEntry,
    IntermediateTemplateContribution, Library, Parent, Require, ResolvedConfigFile, ResolvedDefine,
    ResolvedIncludeEntry, ResolvedLibrary, ResolvedSourceFile, ResolvedTemplateFile,
    ResolvedWithParent, SDKId, Satisfied, SourceFile, TemplateContribution, TemplateFile,
    WithRootPath, SDK,
};
use core::panic;
use serde::Deserialize;
use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    error::Error,
    fs::{self, File},
    io::{Read, Write},
    path::{Path, PathBuf},
    process::Stdio,
    rc::Rc,
    vec,
};

#[derive(Debug, Clone, Deserialize)]
pub struct ProjectRaw {
    pub project_name: String,
    pub sdk: SDKId,
    pub source: Option<Vec<SourceFile>>,
    pub include: Option<Vec<IncludeEntry>>,
    pub config_file: Option<Vec<ConfigFile>>,
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
    pub config_file: Option<Vec<ConfigFile>>,
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
            config_file: raw.config_file,
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
                    .unwrap_or_else(|| panic!("unknown component {}", id.id));

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
                provided_features.extend(additional_provides);
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
                    recommendation_ids.sort();

                    let recommended_components: Vec<_> = recommendation_ids
                        .into_iter()
                        .map(|id| {
                            sdk.components()
                                .iter()
                                .find(|c| c.id == id)
                                .unwrap_or_else(|| panic!("unknown component {}", id))
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
                    if !non_conflicting_recommendations.is_empty() {
                        let c = **non_conflicting_recommendations.first().unwrap();
                        components.push(c.clone());
                        continue;
                    }
                }

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
    pub config_file: Vec<ResolvedConfigFile>,
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

        // Config files are collected in two steps: First the overrides, then the resulting files
        let mut config_file_overrides: HashMap<ConfigFileOverride, ResolvedConfigFile> =
            HashMap::new();
        let mut config_file: Vec<ResolvedConfigFile> = Vec::new();

        let all_features = &resolved.provided_features;

        if let Some(s) = &project.source {
            source.extend(s.iter().filter_map(|e| {
                if e.satisfied(all_features) {
                    Some(e.resolved(Parent::Project))
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

        if let Some(c) = &project.config_file {
            let overrides: HashMap<ConfigFileOverride, ResolvedConfigFile> = c
                .iter()
                .filter_map(|e| {
                    if !e.satisfied(all_features) {
                        return None;
                    }
                    e.r#override
                        .as_ref()
                        .map(|o| (o.clone(), e.resolved(Parent::Project)))
                })
                .collect();
            config_file_overrides.extend(overrides);
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

            if let Some(c) = &comp.config_file {
                let overrides: HashMap<ConfigFileOverride, ResolvedConfigFile> = c
                    .iter()
                    .filter_map(|e| {
                        if !e.satisfied(all_features) {
                            return None;
                        }
                        e.r#override.as_ref().map(|o| {
                            (
                                o.clone(),
                                e.with_root_path(&comp.root_path).resolved(Parent::SDK),
                            )
                        })
                    })
                    .collect();
                config_file_overrides.extend(overrides);
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

        if let Some(c) = &project.config_file {
            // Only include non-override config files. Config files directly in the project cannot be overridden
            config_file.extend(c.iter().filter_map(|e| {
                if e.r#override.is_none() && e.satisfied(all_features) {
                    Some(e.resolved(Parent::Project))
                } else {
                    None
                }
            }));
        }

        for comp in &resolved.components {
            if let Some(c) = &comp.config_file {
                // Only include non-override config files. For each, check if there exists an override we should use instead
                for e in c
                    .iter()
                    .filter(|e| e.r#override.is_none() && e.satisfied(all_features))
                {
                    // Overriding a configuration file that doesn't have a file_id in the original component is not supported.
                    if e.file_id.is_none() {
                        config_file.push(e.with_root_path(&comp.root_path).resolved(Parent::SDK));
                        continue;
                    }

                    // Try to find an override for this config
                    let override_key = ConfigFileOverride {
                        file_id: e.file_id.as_ref().unwrap().clone(),
                        component: comp.id.clone(),
                    };
                    let overr = config_file_overrides.get(&override_key);

                    if let Some(overr) = overr {
                        // If there is an override, use it instead of the original, but preserve some of the fields
                        config_file.push(ResolvedConfigFile {
                            export: e.export,
                            ..overr.clone()
                        });
                    } else {
                        // If not, use the original
                        config_file.push(e.with_root_path(&comp.root_path).resolved(Parent::SDK));
                    }
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
            config_file,
            define,
            library,
            template_file,
            template_contribution,
            provided_features: resolved.provided_features.clone(),
        }
    }

    pub fn generate_templates(
        &self,
        out_dir: impl AsRef<Path>,
    ) -> Result<Vec<PathBuf>, Box<dyn Error>> {
        let vars = serde_yaml::to_string(&self.template_contribution)?;
        let script = include_str!("render_template.py");

        let out_dir = out_dir.as_ref().join("autogen");
        fs::create_dir_all(&out_dir)?;

        let mut ret = Vec::new();

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
            let out_filename = Path::new(out_filename).file_name().unwrap();
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

            fs::write(&out_path, output)?;
            ret.push(out_path);
        }

        Ok(ret)
    }

    pub fn generate_config_files(
        &self,
        out_dir: impl AsRef<Path>,
    ) -> Result<Vec<PathBuf>, Box<dyn Error>> {
        if self.config_file.is_empty() {
            return Ok(Vec::new());
        }

        let out_dir = out_dir.as_ref().join("config");
        fs::create_dir_all(&out_dir)?;

        let mut ret = Vec::new();

        for config_file in &self.config_file {
            let root_path = match config_file.parent {
                Parent::Project => &self.root_path,
                Parent::SDK => &self.sdk_root_path,
            };
            let config_path = root_path.join(&config_file.path);
            let config_filename = config_path.file_name().unwrap();

            let out_path_suffix = match (&config_file.directory, &config_file.export) {
                (None, Some(true)) => Some("export".to_string()),
                (Some(_), Some(true)) => {
                    panic!("config file cannot have both directory and export")
                }
                (Some(ref d), _) => Some(d.clone()),
                (None, _) => None,
            };

            let out_path = if let Some(suffix) = out_path_suffix {
                out_dir.join(suffix).join(config_filename)
            } else {
                out_dir.join(config_filename)
            };

            fs::copy(&config_path, &out_path)?;

            ret.push(out_path);
        }

        Ok(ret)
    }

    pub fn get_included_headers(&self) -> Vec<PathBuf> {
        let mut ret = Vec::new();

        for source in &self.source {
            let root_path = match source.parent {
                Parent::Project => &self.root_path,
                Parent::SDK => &self.sdk_root_path,
            };

            let source_path = root_path.join(&source.path);
            if source_path.extension().map_or(false, |e| e == "h") {
                ret.push(source_path);
            }
        }

        for include in &self.include {
            if let Some(file_list) = &include.file_list {
                let root_path = match include.parent {
                    Parent::Project => &self.root_path,
                    Parent::SDK => &self.sdk_root_path,
                };
                let include_path = root_path.join(&include.path);

                for file in file_list {
                    let header_path = include_path.join(&file.path);
                    if header_path.extension().map_or(false, |e| e == "h") {
                        ret.push(header_path);
                    }
                }
            }
        }

        ret
    }
}
