use crate::{
    ConfigFile, Conflict, Define, Feature, IncludeEntry, Instantiable, Library, OtherFile, Quality,
    Recommendation, Require, SourceFile, TemplateContribution, TemplateFile, ToolchainSetting,
};
use serde::Deserialize;
use std::{
    error::Error,
    fs::File,
    io::Read,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, Deserialize)]
pub struct ComponentRaw {
    #[serde(alias = "name")]
    pub id: String,
    pub quality: Option<Quality>,
    pub root_path: Option<String>,
    pub component_root_path: Option<String>,
    pub instantiable: Option<Instantiable>,
    pub source: Option<Vec<SourceFile>>,
    pub include: Option<Vec<IncludeEntry>>,
    pub config_file: Option<Vec<ConfigFile>>,
    pub define: Option<Vec<Define>>,
    pub requires: Option<Vec<Require>>,
    pub provides: Option<Vec<Feature>>,
    pub conflicts: Option<Vec<Conflict>>,
    pub recommends: Option<Vec<Recommendation>>,
    pub library: Option<Vec<Library>>,
    pub template_file: Option<Vec<TemplateFile>>,
    pub template_contribution: Option<Vec<TemplateContribution>>,
    pub toolchain_settings: Option<Vec<ToolchainSetting>>,
    pub other_file: Option<Vec<OtherFile>>,
}

#[derive(Debug, Clone)]
pub struct Component {
    pub id: String,
    pub quality: Option<Quality>,
    pub root_path: Option<String>,
    pub sdk_root: PathBuf,
    pub instantiable: Option<Instantiable>,
    pub source: Option<Vec<SourceFile>>,
    pub include: Option<Vec<IncludeEntry>>,
    pub config_file: Option<Vec<ConfigFile>>,
    pub define: Option<Vec<Define>>,
    pub requires: Option<Vec<Require>>,
    pub provides: Option<Vec<Feature>>,
    pub conflicts: Option<Vec<Conflict>>,
    pub recommends: Option<Vec<Recommendation>>,
    pub library: Option<Vec<Library>>,
    pub template_file: Option<Vec<TemplateFile>>,
    pub template_contribution: Option<Vec<TemplateContribution>>,
    pub toolchain_settings: Option<Vec<ToolchainSetting>>,
    pub other_file: Option<Vec<OtherFile>>,
}

impl Component {
    pub fn parse(
        path: impl AsRef<Path>,
        sdk_root: impl AsRef<Path>,
    ) -> Result<Self, Box<dyn Error>> {
        let mut file = File::open(path)?;
        let mut data = String::new();
        file.read_to_string(&mut data)?;

        Self::from_str(&data, sdk_root)
    }

    pub fn from_str(data: &str, sdk_root: impl AsRef<Path>) -> Result<Self, Box<dyn Error>> {
        // Strip a leading `!!omap` YAML tag. serde_yaml cannot deserialize a
        // struct from a tagged ordered-map, so drop the tag line and remove the
        // 2-char list prefix (`- ` or two-space continuation) from each body
        // line, leaving a plain mapping. Char-safe so blank/short/multibyte
        // lines don't panic.
        let cleaned;
        let data = if data.starts_with("!!omap") {
            cleaned = data
                .lines()
                .skip(1)
                .map(|line| {
                    let cut = line.char_indices().nth(2).map_or(line.len(), |(i, _)| i);
                    &line[cut..]
                })
                .collect::<Vec<_>>()
                .join("\n");
            cleaned.as_str()
        } else {
            data
        };

        let raw: ComponentRaw = serde_yaml::from_str(data)?;
        let ret = Self {
            id: raw.id,
            quality: raw.quality,
            root_path: raw.component_root_path.or(raw.root_path),
            sdk_root: sdk_root.as_ref().to_path_buf(),
            instantiable: raw.instantiable,
            source: raw.source,
            include: raw.include,
            config_file: raw.config_file,
            define: raw.define,
            requires: raw.requires,
            provides: raw.provides,
            conflicts: raw.conflicts,
            recommends: raw.recommends,
            library: raw.library,
            template_file: raw.template_file,
            template_contribution: raw.template_contribution,
            toolchain_settings: raw.toolchain_settings,
            other_file: raw.other_file,
        };

        Ok(ret)
    }
}

/// A `component:` entry in a `.slcp`. `instance` names apply to instantiable
/// components; `from` selects a component supplied by a named SDK extension.
#[derive(Debug, Clone, Deserialize)]
pub struct ComponentId {
    pub id: String,
    pub instance: Option<Vec<String>>,
    pub from: Option<String>,
    pub condition: Option<Vec<String>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ComponentPath {
    pub path: String,
}
