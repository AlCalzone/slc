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
    pub path: String,
    pub parent: Parent,
    pub file_id: Option<String>,
    pub directory: Option<String>,
    pub export: Option<bool>,
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
        }
    }
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq, Hash)]
pub struct ConfigFileOverride {
    pub file_id: String,
    pub component: String,
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
    pub value: Option<String>,
    pub condition: Option<Vec<String>>,
    pub unless: Option<Vec<String>>,
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
}

impl From<&TemplateContribution> for IntermediateTemplateContribution {
    fn from(t: &TemplateContribution) -> Self {
        Self {
            name: t.name.clone(),
            value: t.value.clone(),
            priority: t.priority,
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
