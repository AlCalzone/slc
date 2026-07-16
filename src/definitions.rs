use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Clone, Copy)]
pub enum Parent {
    Project,
    SDK,
}

pub trait ResolvedWithParent {
    type T;
    fn resolved(&self, parent: Parent) -> Self::T;
}

pub trait WithRootPath {
    fn with_root_path(&self, root_path: &Option<String>) -> Self;
}

#[derive(Debug, Clone, Deserialize)]
pub struct SourceFile {
    pub path: String,
    // Target directory
    pub directory: Option<String>,
    pub condition: Option<Vec<String>>,
    pub unless: Option<Vec<String>>,
}

impl WithRootPath for SourceFile {
    fn with_root_path(&self, root_path: &Option<String>) -> Self {
        if let Some(root) = root_path {
            Self {
                path: Path::new(root)
                    .join(&self.path)
                    .to_string_lossy()
                    .to_string(),
                ..self.clone()
            }
        } else {
            self.clone()
        }
    }
}

#[derive(Debug)]
pub struct ResolvedSourceFile {
    pub path: String,
    pub parent: Parent,
    pub directory: Option<String>,
}

impl ResolvedWithParent for SourceFile {
    type T = ResolvedSourceFile;

    fn resolved(&self, parent: Parent) -> Self::T {
        Self::T {
            path: self.path.clone(),
            parent,
            directory: self.directory.clone(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct ConfigFile {
    pub path: String,
    pub file_id: Option<String>,
    pub directory: Option<String>,
    pub export: Option<bool>,
    pub r#override: Option<ConfigFileOverride>,
    pub condition: Option<Vec<String>>,
    pub unless: Option<Vec<String>>,
}

impl WithRootPath for ConfigFile {
    fn with_root_path(&self, root_path: &Option<String>) -> Self {
        if let Some(root) = root_path {
            Self {
                path: Path::new(root)
                    .join(&self.path)
                    .to_string_lossy()
                    .to_string(),
                ..self.clone()
            }
        } else {
            self.clone()
        }
    }
}

#[derive(Debug, Clone)]
pub struct ResolvedConfigFile {
    // Source path on disk, already instance-prefix-substituted
    pub path: String,
    pub parent: Parent,
    pub file_id: Option<String>,
    pub directory: Option<String>,
    pub export: Option<bool>,
    // Output file name override for instantiable components
    pub output_name: Option<String>,
    // Instance this file was expanded for
    pub instance: Option<String>,
}

impl ResolvedWithParent for ConfigFile {
    type T = ResolvedConfigFile;

    fn resolved(&self, parent: Parent) -> Self::T {
        Self::T {
            path: self.path.clone(),
            parent,
            file_id: self.file_id.clone(),
            directory: self.directory.clone(),
            export: self.export,
            output_name: None,
            instance: None,
        }
    }
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq, Hash)]
pub struct ConfigFileOverride {
    pub file_id: String,
    pub component: String,
    // Targets a single instance of an instantiable component
    pub instance: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct IncludeEntry {
    pub path: String,
    pub directory: Option<String>,
    pub file_list: Option<Vec<HeaderFile>>,
    pub condition: Option<Vec<String>>,
    pub unless: Option<Vec<String>>,
}

impl WithRootPath for IncludeEntry {
    fn with_root_path(&self, root_path: &Option<String>) -> Self {
        if let Some(root) = root_path {
            Self {
                path: Path::new(root)
                    .join(&self.path)
                    .to_string_lossy()
                    .to_string(),
                ..self.clone()
            }
        } else {
            self.clone()
        }
    }
}

#[derive(Debug)]
pub struct ResolvedIncludeEntry {
    pub path: String,
    pub parent: Parent,
    pub directory: Option<String>,
    pub file_list: Option<Vec<ResolvedHeaderFile>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct HeaderFile {
    pub path: String,
    pub condition: Option<Vec<String>>,
    pub unless: Option<Vec<String>>,
}

#[derive(Debug)]
pub struct ResolvedHeaderFile {
    pub path: String,
}

impl From<&HeaderFile> for ResolvedHeaderFile {
    fn from(h: &HeaderFile) -> Self {
        Self {
            path: h.path.clone(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Define {
    pub name: String,
    #[serde(default, deserialize_with = "deserialize_scalar_string")]
    pub value: Option<String>,
    pub condition: Option<Vec<String>>,
    pub unless: Option<Vec<String>>,
}

/// Deserialize a `#define` value written as any YAML scalar into its string
/// form. Real SDK components write integer values unquoted (`value: 4`), which
/// a plain `Option<String>` rejects, failing the whole component's parse.
fn deserialize_scalar_string<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    match serde_yaml::Value::deserialize(deserializer)? {
        serde_yaml::Value::Null => Ok(None),
        serde_yaml::Value::Bool(b) => Ok(Some(b.to_string())),
        serde_yaml::Value::Number(n) => Ok(Some(n.to_string())),
        serde_yaml::Value::String(s) => Ok(Some(s)),
        other => Err(serde::de::Error::custom(format!(
            "define value must be a scalar, got {other:?}"
        ))),
    }
}

#[derive(Debug)]
pub struct ResolvedDefine {
    pub name: String,
    pub value: Option<String>,
}

impl From<&Define> for ResolvedDefine {
    fn from(d: &Define) -> Self {
        Self {
            name: d.name.clone(),
            value: d.value.clone(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Feature {
    pub name: String,
    pub condition: Option<Vec<String>>,
    pub allow_multiple: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Require {
    pub name: String,
    pub condition: Option<Vec<String>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Conflict {
    pub name: String,
    pub condition: Option<Vec<String>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Recommendation {
    pub id: String,
    pub condition: Option<Vec<String>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TemplateContribution {
    pub name: String,
    pub value: minijinja::Value,
    pub priority: Option<i16>,
    pub condition: Option<Vec<String>>,
    pub unless: Option<Vec<String>>,
}

#[derive(Debug, Clone)]
pub struct IntermediateTemplateContribution {
    pub name: String,
    pub value: minijinja::Value,
    pub priority: Option<i16>,
    // Contributing component id, used as tiebreak when priorities match
    pub component_id: String,
}

impl IntermediateTemplateContribution {
    pub fn from_contribution(t: &TemplateContribution, component_id: impl Into<String>) -> Self {
        Self {
            name: t.name.clone(),
            value: t.value.clone(),
            priority: t.priority,
            component_id: component_id.into(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct TemplateFile {
    pub path: String,
    pub condition: Option<Vec<String>>,
    pub unless: Option<Vec<String>>,
    pub export: Option<bool>,
}

impl WithRootPath for TemplateFile {
    fn with_root_path(&self, root_path: &Option<String>) -> Self {
        if let Some(root) = root_path {
            Self {
                path: Path::new(root)
                    .join(&self.path)
                    .to_string_lossy()
                    .to_string(),
                ..self.clone()
            }
        } else {
            self.clone()
        }
    }
}

#[derive(Debug)]
pub struct ResolvedTemplateFile {
    pub path: String,
    pub parent: Parent,
    pub export: Option<bool>,
}

impl ResolvedWithParent for TemplateFile {
    type T = ResolvedTemplateFile;

    fn resolved(&self, parent: Parent) -> Self::T {
        Self::T {
            path: self.path.clone(),
            parent,
            export: self.export,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum Library {
    System(SystemLibrary),
    SDK(SDKLibrary),
}

#[derive(Debug)]
pub enum ResolvedLibrary {
    System(ResolvedSystemLibrary),
    SDK(ResolvedSDKLibrary),
}

impl From<&Library> for ResolvedLibrary {
    fn from(l: &Library) -> Self {
        match l {
            Library::System(s) => ResolvedLibrary::System(s.into()),
            Library::SDK(s) => ResolvedLibrary::SDK(s.into()),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct SystemLibrary {
    pub system: String,
    pub condition: Option<Vec<String>>,
    pub unless: Option<Vec<String>>,
}

#[derive(Debug)]
pub struct ResolvedSystemLibrary {
    pub system: String,
}

impl From<&SystemLibrary> for ResolvedSystemLibrary {
    fn from(s: &SystemLibrary) -> Self {
        Self {
            system: s.system.clone(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct SDKLibrary {
    pub path: String,
    pub condition: Option<Vec<String>>,
    pub unless: Option<Vec<String>>,
}

#[derive(Debug)]
pub struct ResolvedSDKLibrary {
    pub path: String,
}

impl From<&SDKLibrary> for ResolvedSDKLibrary {
    fn from(s: &SDKLibrary) -> Self {
        Self {
            path: s.path.clone(),
        }
    }
}

/// Component/project quality level
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Quality {
    Production,
    Evaluation,
    Experimental,
    Deprecated,
    Internal,
}

impl Quality {
    pub fn from_str_lossy(s: &str) -> Self {
        match s {
            "production" => Quality::Production,
            "evaluation" => Quality::Evaluation,
            "experimental" => Quality::Experimental,
            "deprecated" => Quality::Deprecated,
            "internal" => Quality::Internal,
            // Legacy alias retained for backwards compatibility
            "test" => Quality::Experimental,
            _ => Quality::Evaluation,
        }
    }
}

impl<'de> Deserialize<'de> for Quality {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Ok(Quality::from_str_lossy(&s))
    }
}

/// A component that can be included multiple times under distinct instance names
#[derive(Debug, Clone, Deserialize)]
pub struct Instantiable {
    pub prefix: String,
}

/// A project-level override of a component configuration option
#[derive(Debug, Clone, Deserialize)]
pub struct Configuration {
    pub name: String,
    pub value: String,
    pub condition: Option<Vec<String>>,
    pub unless: Option<Vec<String>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ToolchainSetting {
    pub option: String,
    pub value: String,
    pub condition: Option<Vec<String>>,
    pub unless: Option<Vec<String>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OtherFile {
    pub path: String,
    pub directory: Option<String>,
    pub condition: Option<Vec<String>>,
    pub unless: Option<Vec<String>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PostBuildEntry {
    // Exactly one of profile/path is expected
    pub profile: Option<String>,
    pub path: Option<String>,
    pub condition: Option<Vec<String>>,
    pub unless: Option<Vec<String>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum PostBuild {
    One(PostBuildEntry),
    Many(Vec<PostBuildEntry>),
}

impl PostBuild {
    pub fn entries(&self) -> &[PostBuildEntry] {
        match self {
            PostBuild::One(e) => std::slice::from_ref(e),
            PostBuild::Many(v) => v,
        }
    }
}

/// Replace `{{instance}}` with the given value
pub fn substitute_instance(input: &str, value: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut rest = input;
    while let Some(start) = rest.find("{{") {
        if let Some(end_rel) = rest[start..].find("}}") {
            let inner = &rest[start + 2..start + end_rel];
            if inner.trim() == "instance" {
                out.push_str(&rest[..start]);
                out.push_str(value);
                rest = &rest[start + end_rel + 2..];
                continue;
            }
        }
        // Not an {{instance}} token: keep the "{{" and continue scanning past it
        out.push_str(&rest[..start + 2]);
        rest = &rest[start + 2..];
    }
    out.push_str(rest);
    out
}

/// Transform config-file content for an instance, matching the conventions of the target filenames
pub fn transform_instance_content(path: &Path, instance: &str, content: &str) -> String {
    match path.extension().and_then(|e| e.to_str()) {
        Some("h" | "hh" | "hpp" | "hxx") => content.replace("INSTANCE", &instance.to_uppercase()),
        Some("xml") => substitute_instance(content, instance),
        _ => content.to_string(),
    }
}
