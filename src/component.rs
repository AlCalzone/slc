use std::{
    collections::BTreeMap, error::Error, fs::File, io::Read, path::{Path, PathBuf}
};

use crate::{
    Conflict, Define, Feature, IncludeEntry, Library, Recommendation, Require, SourceFile,
    TemplateContribution, TemplateFile,
};
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct ComponentRaw {
    pub id: String,
    pub root_path: Option<String>,
    pub source: Option<Vec<SourceFile>>,
    pub include: Option<Vec<IncludeEntry>>,
    pub define: Option<Vec<Define>>,
    pub requires: Option<Vec<Require>>,
    pub provides: Option<Vec<Feature>>,
    pub conflicts: Option<Vec<Conflict>>,
    pub recommends: Option<Vec<Recommendation>>,
    pub library: Option<Vec<Library>>,
    pub template_file: Option<Vec<TemplateFile>>,
    pub template_contribution: Option<Vec<TemplateContribution>>,
}

#[derive(Debug, Clone)]
pub struct Component {
    pub id: String,
    pub root_path: Option<String>,
    pub sdk_root: PathBuf,
    pub source: Option<Vec<SourceFile>>,
    pub include: Option<Vec<IncludeEntry>>,
    pub define: Option<Vec<Define>>,
    pub requires: Option<Vec<Require>>,
    pub provides: Option<Vec<Feature>>,
    pub conflicts: Option<Vec<Conflict>>,
    pub recommends: Option<Vec<Recommendation>>,
    pub library: Option<Vec<Library>>,
    pub template_file: Option<Vec<TemplateFile>>,
    pub template_contribution: Option<Vec<TemplateContribution>>,
}

impl Component {
    pub fn parse(
        path: impl AsRef<Path>,
        sdk_root: impl AsRef<Path>,
    ) -> Result<Self, Box<dyn Error>> {
        let mut file = File::open(path)?;
        let mut data = String::new();
        file.read_to_string(&mut data)?;

        // Add support for parsing files with !!omap at the start
        if data.starts_with("!!omap") {
            data = data
                .lines()
                .skip(1)
                .map(|line| &line[2..])
                .collect::<Vec<_>>()
                .join("\n");
        }

        let raw: ComponentRaw = serde_yaml::from_str(&data)?;
        let ret = Self {
            id: raw.id,
            root_path: raw.root_path,
            sdk_root: sdk_root.as_ref().to_path_buf(),
            source: raw.source,
            include: raw.include,
            define: raw.define,
            requires: raw.requires,
            provides: raw.provides,
            conflicts: raw.conflicts,
            recommends: raw.recommends,
            library: raw.library,
            template_file: raw.template_file,
            template_contribution: raw.template_contribution,
        };

        Ok(ret)
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct ComponentId {
    pub id: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ComponentPath {
    pub path: String,
}
