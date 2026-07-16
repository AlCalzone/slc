use crate::{Component, ComponentPath};
use serde::Deserialize;
use std::{
    error::Error,
    fs::File,
    io::Read,
    path::{Path, PathBuf},
    rc::Rc,
};

#[derive(Debug, Clone, Deserialize)]
pub struct SDKRaw {
    pub id: String,
    pub label: Option<String>,
    pub description: Option<String>,
    pub sdk_version: String,
    pub specification_version: Option<u32>,
    pub supplier: Option<String>,
    pub component_path: Vec<ComponentPath>,
}

#[derive(Debug, Clone)]
pub struct SDK {
    pub id: String,
    pub label: Option<String>,
    pub sdk_version: String,
    pub specification_version: Option<u32>,
    pub root_path: PathBuf,
    components: Vec<Rc<Component>>,
}

impl SDK {
    pub fn parse(path: impl AsRef<Path>) -> Result<Self, Box<dyn Error>> {
        let path = path.as_ref();
        let mut file = File::open(path)?;
        let mut data = String::new();
        file.read_to_string(&mut data)?;

        let raw: SDKRaw = serde_yaml::from_str(&data)?;
        let root_path = path.parent().unwrap().to_path_buf();
        let components = load_components(&root_path, raw.component_path);

        let ret = Self {
            id: raw.id,
            label: raw.label,
            sdk_version: raw.sdk_version,
            specification_version: raw.specification_version,
            root_path,
            components,
        };

        Ok(ret)
    }

    pub fn components(&self) -> &[Rc<Component>] {
        &self.components
    }

    /// Build an SDK directly from already-parsed components, bypassing disk
    /// discovery. Intended for tests.
    #[doc(hidden)]
    pub fn from_components(
        id: impl Into<String>,
        root_path: PathBuf,
        components: Vec<Rc<Component>>,
    ) -> Self {
        Self {
            id: id.into(),
            label: None,
            sdk_version: String::new(),
            specification_version: None,
            root_path,
            components,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct SDKId {
    pub id: String,
    pub version: String,
}

pub fn load_components(
    sdk_root: &PathBuf,
    component_paths: Vec<ComponentPath>,
) -> Vec<Rc<Component>> {
    component_paths
        .iter()
        .flat_map(|comp_dir| {
            let comp_dir = sdk_root.join(&comp_dir.path);
            let Ok(read_dir) = std::fs::read_dir(comp_dir) else {
                return vec![];
            };

            let files = read_dir
                .into_iter()
                .filter_map(|e| e.ok())
                .filter(|f| f.file_type().map(|t| t.is_file()).unwrap_or(false))
                .filter(|f| matches!(f.path().extension(), Some(ext) if ext == "slcc"));

            // Skip unparseable components rather than aborting the SDK load
            let components: Vec<_> = files
                .into_iter()
                .filter_map(move |f| match Component::parse(f.path(), sdk_root) {
                    Ok(comp) => Some(Rc::new(comp)),
                    Err(e) => {
                        eprintln!("slc: warning: skipping {}: {e}", f.path().display());
                        None
                    }
                })
                .collect();
            components
        })
        .collect()
}
