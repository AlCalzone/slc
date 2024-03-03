use std::{
    error::Error,
    fs::File,
    io::Read,
    path::{Path, PathBuf},
    rc::Rc,
};

use crate::{Component, ComponentPath};
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct SDKRaw {
    pub id: String,
    pub sdk_version: String,
    pub component_path: Vec<ComponentPath>,
}

#[derive(Debug, Clone)]
pub struct SDK {
    pub id: String,
    pub sdk_version: String,
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
            sdk_version: raw.sdk_version,
            root_path,
            components,
        };

        Ok(ret)
    }

    pub fn components(&self) -> &[Rc<Component>] {
        &self.components
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
            let Ok(read_dir) = std::fs::read_dir(&comp_dir) else {
                return vec![];
            };

            let files = read_dir
                .into_iter()
                .filter_map(|e| e.ok())
                .filter(|f| f.file_type().unwrap().is_file())
                .filter(|f| matches!(f.path().extension(), Some(ext) if ext == "slcc"));

            let components: Vec<_> = files
                .into_iter()
                .map(move |f| {
                    let comp = Component::parse(f.path(), sdk_root).unwrap();
                    Rc::new(comp)
                })
                .collect();
            components
        })
        .collect()
}
