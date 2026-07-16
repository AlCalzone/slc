use crate::{
    substitute_instance, transform_instance_content, Component, ComponentId, ConfigFile,
    ConfigFileOverride, Configuration, Conflict, Define, Feature, IncludeEntry,
    IntermediateTemplateContribution, Library, OtherFile, Parent, PostBuild, Quality, Require,
    ResolvedConfigFile, ResolvedDefine, ResolvedIncludeEntry, ResolvedLibrary, ResolvedSourceFile,
    ResolvedTemplateFile, ResolvedWithParent, SDKId, Satisfied, SourceFile, TemplateContribution,
    TemplateFile, ToolchainSetting, WithRootPath, SDK,
};
use serde::Deserialize;
use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    error::Error,
    fs::{self, File},
    io::{Read, Write},
    path::{Path, PathBuf},
    process::Stdio,
    rc::Rc,
};

#[derive(Debug, Clone, Deserialize)]
pub struct ProjectRaw {
    #[serde(alias = "name")]
    pub project_name: String,
    pub quality: Option<Quality>,
    pub sdk: SDKId,
    pub source: Option<Vec<SourceFile>>,
    pub include: Option<Vec<IncludeEntry>>,
    pub config_file: Option<Vec<ConfigFile>>,
    pub component: Option<Vec<ComponentId>>,
    pub define: Option<Vec<Define>>,
    pub requires: Option<Vec<Require>>,
    pub provides: Option<Vec<Feature>>,
    pub conflicts: Option<Vec<Conflict>>,
    pub library: Option<Vec<Library>>,
    pub template_contribution: Option<Vec<TemplateContribution>>,
    pub configuration: Option<Vec<Configuration>>,
    pub toolchain_settings: Option<Vec<ToolchainSetting>>,
    pub other_file: Option<Vec<OtherFile>>,
    pub post_build: Option<PostBuild>,
}

#[derive(Debug, Clone)]
pub struct Project {
    pub project_name: String,
    pub quality: Option<Quality>,
    pub sdk: SDKId,
    pub root_path: PathBuf,
    pub source: Option<Vec<SourceFile>>,
    pub include: Option<Vec<IncludeEntry>>,
    pub config_file: Option<Vec<ConfigFile>>,
    pub component: Option<Vec<ComponentId>>,
    pub define: Option<Vec<Define>>,
    pub requires: Option<Vec<Require>>,
    pub provides: Option<Vec<Feature>>,
    pub conflicts: Option<Vec<Conflict>>,
    pub library: Option<Vec<Library>>,
    pub template_contribution: Option<Vec<TemplateContribution>>,
    pub configuration: Option<Vec<Configuration>>,
    pub toolchain_settings: Option<Vec<ToolchainSetting>>,
    pub other_file: Option<Vec<OtherFile>>,
    pub post_build: Option<PostBuild>,
}

impl Project {
    pub fn parse(path: impl AsRef<Path>) -> Result<Project, Box<dyn Error>> {
        let path = path.as_ref();
        let mut file = File::open(path)?;
        let mut data = String::new();
        file.read_to_string(&mut data)?;
        let root_path = path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf();
        Self::from_str(&data, root_path)
    }

    pub fn from_str(data: &str, root_path: PathBuf) -> Result<Project, Box<dyn Error>> {
        let raw: ProjectRaw = serde_yaml::from_str(data)?;

        let ret = Self {
            project_name: raw.project_name,
            quality: raw.quality,
            sdk: raw.sdk,
            root_path,
            source: raw.source,
            include: raw.include,
            config_file: raw.config_file,
            component: raw.component,
            define: raw.define,
            requires: raw.requires,
            provides: raw.provides,
            conflicts: raw.conflicts,
            library: raw.library,
            template_contribution: raw.template_contribution,
            configuration: raw.configuration,
            toolchain_settings: raw.toolchain_settings,
            other_file: raw.other_file,
            post_build: raw.post_build,
        };

        Ok(ret)
    }

    pub fn resolve_components(&self, sdk: &SDK) -> Result<ResolveResult, ResolveError> {
        // https://siliconlabs.github.io/slc-specification/1.2/features/

        let mut components: Vec<Rc<Component>> = Vec::new();
        let mut instances: BTreeMap<String, Vec<String>> = BTreeMap::new();

        if let Some(ref comp_ids) = self.component {
            for id in comp_ids {
                if let Some(list) = &id.instance {
                    instances
                        .entry(id.id.clone())
                        .or_default()
                        .extend(list.iter().cloned());
                }
                // A component may be listed more than once (e.g. two instance
                // entries); add it to C only once.
                if components.iter().any(|c| c.id == id.id) {
                    continue;
                }
                let c = sdk
                    .components()
                    .iter()
                    .find(|c| c.id == id.id)
                    .ok_or_else(|| ResolveError::UnknownComponent(id.id.clone()))?;
                components.push(c.clone());
            }
        }

        // The project itself contributes to the required (R) and provided (P)
        // feature sets: its `requires` are requirements, its `provides` are
        // features it satisfies on its own.
        let mut project_requires: BTreeSet<String> = BTreeSet::new();
        let mut project_provides: BTreeSet<String> = BTreeSet::new();

        if let Some(ref requires) = self.requires {
            for req in requires {
                project_requires.insert(req.name.clone());
            }
        }
        if let Some(ref provides) = self.provides {
            for prov in provides {
                project_provides.insert(prov.name.clone());
            }
        }

        loop {
            let mut required_features: BTreeSet<String> =
                BTreeSet::from_iter(project_requires.iter().cloned());
            let mut provided_features: BTreeSet<String> =
                BTreeSet::from_iter(project_provides.iter().cloned());
            let mut conflicts: BTreeSet<String> = BTreeSet::new();

            let mut added_component = false;

            // Fixpoint: a component provides a conditional feature only once the
            // features its condition names are themselves provided. Keep adding
            // until nothing new appears.
            loop {
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
            if let Some(ref pc) = self.conflicts {
                conflicts.extend(
                    pc.iter()
                        .filter(|c| c.satisfied(&provided_features))
                        .map(|c| c.name.clone()),
                );
            }

            let unsatisfied: BTreeSet<String> = required_features
                .difference(&provided_features)
                .cloned()
                .collect();
            if unsatisfied.is_empty() {
                // Success criterion: K disjoint from P.
                let clashing: Vec<String> = conflicts
                    .intersection(&provided_features)
                    .cloned()
                    .collect();
                if !clashing.is_empty() {
                    return Err(ResolveError::ConflictingFeatures(clashing));
                }
                // Success criterion: no feature is provided by more than one
                // component unless every provider marks it `allow_multiple`.
                check_duplicate_provides(&components, &provided_features)?;

                return Ok(ResolveResult {
                    components,
                    provided_features,
                    instances,
                });
            }

            let required_and_provided: BTreeSet<String> = required_features
                .union(&provided_features)
                .cloned()
                .collect();

            // For each open requirement, find SDK components that could satisfy
            // it (condition evaluated against R+P) without introducing a known
            // conflict, excluding components already in C.
            let mut zero_candidate: Option<String> = None;
            let mut multi_candidate: Option<(String, Vec<String>)> = None;

            for req in unsatisfied.iter() {
                let candidates: Vec<_> = sdk
                    .components()
                    .iter()
                    .filter(|c| !components.iter().any(|e| e.id == c.id))
                    .filter(|c| {
                        let Some(provides) = &c.provides else {
                            return false;
                        };
                        provides
                            .iter()
                            .any(|f| &f.name == req && f.satisfied(&required_and_provided))
                    })
                    // Spec: a candidate must not provide any feature currently
                    // in the conflict set K. (A candidate that itself conflicts
                    // with an absent feature is fine; a real clash is caught by
                    // the success criterion once the feature is present.)
                    .filter(|c| {
                        let Some(provides) = &c.provides else {
                            return true;
                        };
                        !provides.iter().any(|f| {
                            f.satisfied(&required_and_provided) && conflicts.contains(&f.name)
                        })
                    })
                    .collect();

                match candidates.len() {
                    0 => {
                        if zero_candidate.is_none() {
                            zero_candidate = Some(req.clone());
                        }
                    }
                    1 => {
                        components.push(candidates[0].clone());
                        added_component = true;
                    }
                    _ => {
                        if multi_candidate.is_none() {
                            multi_candidate = Some((
                                req.clone(),
                                candidates.iter().map(|c| c.id.clone()).collect(),
                            ));
                        }
                    }
                }
            }

            if added_component {
                continue;
            }

            // Nothing auto-added. Try recommendations: a recommended component
            // is considered only if it provides an open requirement, and its
            // provided features are disjoint from every other recommendation.
            // One is added per pass (alphabetically by id), then resolution
            // restarts.
            if multi_candidate.is_some() {
                let mut recommendation_ids: Vec<_> = components
                    .iter()
                    .filter_map(|c| c.recommends.as_ref())
                    .flatten()
                    .filter(|r| r.satisfied(&provided_features))
                    .map(|r| r.id.clone())
                    .collect();
                recommendation_ids.sort();
                recommendation_ids.dedup();

                let recommended_components: Vec<_> = recommendation_ids
                    .into_iter()
                    .filter(|id| !components.iter().any(|c| &c.id == id))
                    .filter_map(|id| sdk.components().iter().find(|c| c.id == id))
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
                    .map(|c| {
                        let feature_set: BTreeSet<String> = c
                            .provides
                            .iter()
                            .flat_map(|p| {
                                p.iter()
                                    .filter(|f| f.satisfied(&provided_features))
                                    .map(|f| f.name.clone())
                            })
                            .collect();
                        (c, feature_set)
                    })
                    .collect();

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

                if let Some(c) = non_conflicting_recommendations.first() {
                    components.push((**c).clone());
                    continue;
                }
            }

            // No progress possible: report the most specific failure.
            if let Some(req) = zero_candidate {
                return Err(ResolveError::UnsatisfiedRequirement(req));
            }
            if let Some((requirement, candidates)) = multi_candidate {
                return Err(ResolveError::AmbiguousRequirement {
                    requirement,
                    candidates,
                });
            }
            // Reachable only if every unsatisfied requirement is neither zero-
            // nor multi-candidate, which cannot happen; guard defensively.
            return Err(ResolveError::UnsatisfiedRequirement(
                unsatisfied.into_iter().next().unwrap_or_default(),
            ));
        }
    }
}

/// Enforce the SLC success criterion that no feature is provided by two
/// components unless `allow_multiple` is set on every provider of it.
fn check_duplicate_provides(
    components: &[Rc<Component>],
    provided_features: &BTreeSet<String>,
) -> Result<(), ResolveError> {
    let mut providers: BTreeMap<String, Vec<(String, bool)>> = BTreeMap::new();
    for c in components {
        if let Some(provides) = &c.provides {
            for f in provides {
                if f.satisfied(provided_features) {
                    providers
                        .entry(f.name.clone())
                        .or_default()
                        .push((c.id.clone(), f.allow_multiple.unwrap_or(false)));
                }
            }
        }
    }
    for (feature, provs) in &providers {
        let distinct: BTreeSet<&String> = provs.iter().map(|(id, _)| id).collect();
        if distinct.len() > 1 && !provs.iter().all(|(_, allow)| *allow) {
            return Err(ResolveError::DuplicateProvide {
                feature: feature.clone(),
                components: distinct.into_iter().cloned().collect(),
            });
        }
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolveError {
    UnknownComponent(String),
    UnsatisfiedRequirement(String),
    AmbiguousRequirement {
        requirement: String,
        candidates: Vec<String>,
    },
    ConflictingFeatures(Vec<String>),
    DuplicateProvide {
        feature: String,
        components: Vec<String>,
    },
}

impl std::fmt::Display for ResolveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ResolveError::UnknownComponent(id) => write!(f, "unknown component: {id}"),
            ResolveError::UnsatisfiedRequirement(req) => {
                write!(f, "no component provides required feature: {req}")
            }
            ResolveError::AmbiguousRequirement {
                requirement,
                candidates,
            } => write!(
                f,
                "requirement {requirement} has multiple providers, none auto-selectable: {}",
                candidates.join(", ")
            ),
            ResolveError::ConflictingFeatures(fs) => {
                write!(f, "conflicting features provided: {}", fs.join(", "))
            }
            ResolveError::DuplicateProvide {
                feature,
                components,
            } => write!(
                f,
                "feature {feature} provided by multiple components without allow_multiple: {}",
                components.join(", ")
            ),
        }
    }
}

impl std::error::Error for ResolveError {}

#[derive(Debug)]
pub struct ResolveResult {
    pub components: Vec<Rc<Component>>,
    pub provided_features: BTreeSet<String>,
    /// Instance names per component id, for instantiable components included
    /// with an explicit `instance` list in the project.
    pub instances: BTreeMap<String, Vec<String>>,
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
    /// Project-level configuration overrides, already filtered by
    /// condition/unless and kept in declaration order (last match wins).
    pub configuration: Vec<Configuration>,
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
            for e in contrib.iter().filter(|e| e.satisfied(all_features)) {
                let t = IntermediateTemplateContribution::from_contribution(e, "");
                template_contribution
                    .entry(t.name.clone())
                    .or_default()
                    .push(t);
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
                let empty = Vec::new();
                let comp_instances = resolved.instances.get(&comp.id).unwrap_or(&empty);
                let is_instantiable = comp.instantiable.is_some() && !comp_instances.is_empty();
                for e in d.iter().filter(|e| e.satisfied(all_features)) {
                    let has_ph =
                        has_instance_ph(&e.name) || e.value.as_deref().is_some_and(has_instance_ph);
                    if is_instantiable && has_ph {
                        for instance in comp_instances {
                            define.push(ResolvedDefine {
                                name: substitute_instance(&e.name, instance),
                                value: e.value.as_deref().map(|v| substitute_instance(v, instance)),
                            });
                        }
                    } else {
                        define.push(e.into());
                    }
                }
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
                for e in contrib.iter().filter(|e| e.satisfied(all_features)) {
                    let t = IntermediateTemplateContribution::from_contribution(e, comp.id.clone());
                    template_contribution
                        .entry(t.name.clone())
                        .or_default()
                        .push(t);
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
            let empty = Vec::new();
            let comp_instances = resolved.instances.get(&comp.id).unwrap_or(&empty);
            let prefix = comp.instantiable.as_ref().map(|i| i.prefix.as_str());

            if let Some(c) = &comp.config_file {
                // Only include non-override config files; for each, look for an
                // override to substitute in.
                for e in c
                    .iter()
                    .filter(|e| e.r#override.is_none() && e.satisfied(all_features))
                {
                    // Expand one entry per instance when the component is
                    // instantiable and the path carries an {{instance}} token;
                    // otherwise a single non-instance rendering.
                    let expand: Vec<Option<&str>> = if prefix.is_some()
                        && !comp_instances.is_empty()
                        && has_instance_ph(&e.path)
                    {
                        comp_instances.iter().map(|i| Some(i.as_str())).collect()
                    } else {
                        vec![None]
                    };

                    for inst in expand {
                        // SDK stage: source path uses the prefix; output name
                        // uses the instance name.
                        let (src_path, output_name) = match (inst, prefix) {
                            (Some(instance), Some(pfx)) => {
                                let src = substitute_instance(&e.path, pfx);
                                let out = Path::new(&substitute_instance(&e.path, instance))
                                    .file_name()
                                    .map(|n| n.to_string_lossy().into_owned());
                                (src, out)
                            }
                            _ => (e.path.clone(), None),
                        };

                        // Locate an override (preferring an instance-specific
                        // one) when the entry declares a file_id.
                        let overr = e.file_id.as_ref().and_then(|fid| {
                            let by_instance = inst.and_then(|instance| {
                                config_file_overrides.get(&ConfigFileOverride {
                                    file_id: fid.clone(),
                                    component: comp.id.clone(),
                                    instance: Some(instance.to_string()),
                                })
                            });
                            by_instance.or_else(|| {
                                config_file_overrides.get(&ConfigFileOverride {
                                    file_id: fid.clone(),
                                    component: comp.id.clone(),
                                    instance: None,
                                })
                            })
                        });

                        let resolved_cf = if let Some(overr) = overr {
                            ResolvedConfigFile {
                                export: e.export,
                                output_name: output_name.clone(),
                                instance: inst.map(str::to_string),
                                ..overr.clone()
                            }
                        } else {
                            ResolvedConfigFile {
                                path: join_root(&comp.root_path, &src_path),
                                parent: Parent::SDK,
                                file_id: e.file_id.clone(),
                                directory: e.directory.clone(),
                                export: e.export,
                                output_name,
                                instance: inst.map(str::to_string),
                            }
                        };
                        config_file.push(resolved_cf);
                    }
                }
            }
        }

        // A contribution list is ordered by priority (lowest/most-negative
        // first), with the contributing component id as a deterministic
        // tiebreak. The stable sort preserves declaration order within a tie.
        let template_contribution: BTreeMap<String, Vec<minijinja::Value>> = template_contribution
            .into_iter()
            .map(|(k, mut v)| {
                v.sort_by(|a, b| {
                    (a.priority.unwrap_or(0), &a.component_id)
                        .cmp(&(b.priority.unwrap_or(0), &b.component_id))
                });
                let values: Vec<_> = v.into_iter().map(|t| t.value).collect();
                (k, values)
            })
            .collect();

        let configuration: Vec<Configuration> = project
            .configuration
            .iter()
            .flatten()
            .filter(|c| c.satisfied(all_features))
            .cloned()
            .collect();

        Self {
            root_path: project.root_path.clone(),
            sdk_root_path: sdk.root_path.clone(),
            source,
            include,
            config_file,
            configuration,
            define,
            library,
            template_file,
            template_contribution,
            provided_features: resolved.provided_features.clone(),
        }
    }

    fn root_for(&self, parent: Parent) -> &PathBuf {
        match parent {
            Parent::Project => &self.root_path,
            Parent::SDK => &self.sdk_root_path,
        }
    }

    /// Generate the full project output tree under `out_dir`: the standard
    /// `autogen/` and `config/` directories (with their `export/` subdirs) are
    /// always created, then config files and templates are emitted.
    pub fn generate(&self, out_dir: impl AsRef<Path>) -> Result<Vec<PathBuf>, Box<dyn Error>> {
        let out_dir = out_dir.as_ref();
        for sub in ["autogen/export", "config/export"] {
            fs::create_dir_all(out_dir.join(sub))?;
        }
        let mut written = self.generate_config_files(out_dir)?;
        written.extend(self.generate_templates(out_dir)?);
        Ok(written)
    }

    pub fn generate_templates(
        &self,
        out_dir: impl AsRef<Path>,
    ) -> Result<Vec<PathBuf>, Box<dyn Error>> {
        let vars = serde_yaml::to_string(&self.template_contribution)?;
        let script = include_str!("render_template.py");

        let autogen = out_dir.as_ref().join("autogen");
        let export = autogen.join("export");
        fs::create_dir_all(&export)?;

        let mut ret = Vec::new();

        for template in &self.template_file {
            let template_path = self.root_for(template.parent).join(&template.path);

            // The source file name (with its .jinja suffix) names the template
            // in the generated banner.
            let source_name = Path::new(&template.path)
                .file_name()
                .and_then(|n| n.to_str())
                .ok_or("template path has no file name")?
                .to_string();

            // Strip a trailing .jinja/.jinja2 for the output name; otherwise
            // keep the file name unchanged.
            let stripped = template
                .path
                .strip_suffix(".jinja")
                .or_else(|| template.path.strip_suffix(".jinja2"))
                .unwrap_or(&template.path);
            let out_filename = Path::new(stripped)
                .file_name()
                .ok_or("template path has no file name")?;

            let dir = if template.export == Some(true) {
                &export
            } else {
                &autogen
            };
            let out_path = dir.join(out_filename);

            let rendered = render_with_python(script, &vars, &template_path, &source_name)?;
            fs::write(&out_path, rendered)?;
            ret.push(out_path);
        }

        Ok(ret)
    }

    pub fn generate_config_files(
        &self,
        out_dir: impl AsRef<Path>,
    ) -> Result<Vec<PathBuf>, Box<dyn Error>> {
        let config_root = out_dir.as_ref().join("config");
        fs::create_dir_all(config_root.join("export"))?;

        let mut ret = Vec::new();

        for config_file in &self.config_file {
            let src = self.root_for(config_file.parent).join(&config_file.path);
            // An instantiable config file emits under its instance-substituted
            // name; otherwise it keeps the source file name.
            let config_filename: String = match &config_file.output_name {
                Some(n) => n.clone(),
                None => src
                    .file_name()
                    .ok_or("config file path has no file name")?
                    .to_string_lossy()
                    .into_owned(),
            };

            // export takes precedence; directory places the file in a config/
            // subdirectory; otherwise the file lands directly in config/.
            let out_path = if config_file.export == Some(true) {
                config_root.join("export").join(&config_filename)
            } else if let Some(dir) = &config_file.directory {
                config_root.join(dir).join(&config_filename)
            } else {
                config_root.join(&config_filename)
            };

            // Spec: a config file already present in config/ is not overwritten,
            // preserving user edits across regeneration.
            if out_path.exists() {
                ret.push(out_path);
                continue;
            }
            if let Some(parent) = out_path.parent() {
                fs::create_dir_all(parent)?;
            }

            let mut contents = fs::read_to_string(&src)?;
            if let Some(instance) = &config_file.instance {
                contents = transform_instance_content(&src, instance, &contents);
            }
            let contents = self.apply_configuration(&contents);
            fs::write(&out_path, contents)?;
            ret.push(out_path);
        }

        Ok(ret)
    }

    /// Rewrite `#define NAME value` lines in a freshly-copied config header
    /// with the project's configuration overrides. Applied only on first copy;
    /// the last matching rule for a given name wins.
    fn apply_configuration(&self, contents: &str) -> String {
        if self.configuration.is_empty() {
            return contents.to_string();
        }
        // Later entries overwrite earlier ones -> last match wins.
        let mut values: HashMap<&str, &str> = HashMap::new();
        for c in &self.configuration {
            values.insert(c.name.as_str(), c.value.as_str());
        }

        let mut out = String::with_capacity(contents.len());
        for line in contents.split_inclusive('\n') {
            out.push_str(&rewrite_define_line(line, &values));
        }
        out
    }

    pub fn get_included_headers(&self) -> Vec<PathBuf> {
        let mut ret = Vec::new();

        for source in &self.source {
            let source_path = self.root_for(source.parent).join(&source.path);
            if is_header(&source_path) {
                ret.push(source_path);
            }
        }

        for include in &self.include {
            if let Some(file_list) = &include.file_list {
                let include_path = self.root_for(include.parent).join(&include.path);
                for file in file_list {
                    let header_path = include_path.join(&file.path);
                    if is_header(&header_path) {
                        ret.push(header_path);
                    }
                }
            }
        }

        ret
    }
}

fn is_header(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|e| e.to_str()),
        Some("h" | "hh" | "hpp" | "hxx")
    )
}

/// Join a component-relative path onto its optional root_path, mirroring
/// [`WithRootPath`] for a bare string path.
fn join_root(root: &Option<String>, path: &str) -> String {
    match root {
        Some(r) => Path::new(r).join(path).to_string_lossy().into_owned(),
        None => path.to_string(),
    }
}

/// Whether a path contains an `{{instance}}` placeholder.
fn has_instance_ph(s: &str) -> bool {
    substitute_instance(s, "\u{1}") != s
}

/// If `line` is an object-like `#define NAME ...` whose NAME has an override,
/// return the line with the value replaced (indentation and trailing newline
/// preserved). Otherwise return the line unchanged.
fn rewrite_define_line(line: &str, values: &HashMap<&str, &str>) -> String {
    let (content, nl) = match line.strip_suffix('\n') {
        Some(c) => (c, "\n"),
        None => (line, ""),
    };
    let trimmed = content.trim_start();
    let indent = &content[..content.len() - trimmed.len()];

    let Some(after) = trimmed.strip_prefix("#define") else {
        return line.to_string();
    };
    // `#define` must be followed by whitespace, else it is not a directive.
    if !after.starts_with(|c: char| c.is_whitespace()) {
        return line.to_string();
    }
    let after = after.trim_start();
    let name: String = after
        .chars()
        .take_while(|c| c.is_alphanumeric() || *c == '_')
        .collect();
    if name.is_empty() {
        return line.to_string();
    }
    // Skip function-like macros: a '(' immediately after the name.
    if after[name.len()..].starts_with('(') {
        return line.to_string();
    }
    match values.get(name.as_str()) {
        Some(value) => format!("{indent}#define {name} {value}{nl}"),
        None => line.to_string(),
    }
}

/// Render one template through the embedded Python/Jinja2 helper. The template
/// file name is passed through so the autogenerated banner can name its source.
fn render_with_python(
    script: &str,
    vars: &str,
    template_path: &Path,
    template_name: &str,
) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut template_content = String::new();
    File::open(template_path)?.read_to_string(&mut template_content)?;

    let mut python = std::process::Command::new("python3")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .arg("-")
        .env("VARS", vars)
        .env("TEMPLATE", template_content)
        .env("TEMPLATE_NAME", template_name)
        .spawn()?;

    {
        let mut stdin = python.stdin.take().ok_or("failed to open python stdin")?;
        stdin.write_all(script.as_bytes())?;
    }

    let output = python.wait_with_output()?;
    if !output.status.success() {
        return Err(format!(
            "template generator failed for {template_name}: {}",
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }
    Ok(output.stdout)
}
