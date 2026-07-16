//! Shared harness for building a throwaway on-disk SDK + project and running
//! the full parse -> resolve -> generate pipeline against a temp directory.

#![allow(dead_code)]

use slc::{ParsedProject, Project, SDK};
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

pub struct Fixture {
    pub dir: TempDir,
    pub sdk_root: PathBuf,
    pub project_dir: PathBuf,
    pub out_dir: PathBuf,
}

impl Fixture {
    pub fn new() -> Self {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path();
        let sdk_root = root.join("sdk");
        let project_dir = root.join("project");
        let out_dir = root.join("out");
        fs::create_dir_all(sdk_root.join("components")).unwrap();
        fs::create_dir_all(&project_dir).unwrap();
        fs::create_dir_all(&out_dir).unwrap();
        Fixture {
            dir,
            sdk_root,
            project_dir,
            out_dir,
        }
    }

    /// Write a component `.slcc` under `sdk/components/<name>.slcc`.
    pub fn component(&self, name: &str, yaml: &str) -> &Self {
        write_file(
            &self
                .sdk_root
                .join("components")
                .join(format!("{name}.slcc")),
            yaml,
        );
        self
    }

    /// Write an arbitrary file relative to the SDK root (where component
    /// source/config/template paths resolve).
    pub fn sdk_file(&self, rel: &str, contents: &str) -> &Self {
        write_file(&self.sdk_root.join(rel), contents);
        self
    }

    /// Write an arbitrary file relative to the project root.
    pub fn project_file(&self, rel: &str, contents: &str) -> &Self {
        write_file(&self.project_dir.join(rel), contents);
        self
    }

    pub fn out_file(&self, rel: &str) -> PathBuf {
        self.out_dir.join(rel)
    }

    pub fn read_out(&self, rel: &str) -> String {
        fs::read_to_string(self.out_file(rel)).unwrap_or_else(|e| panic!("reading out/{rel}: {e}"))
    }

    fn write_slcs(&self) {
        write_file(
            &self.sdk_root.join("test.slcs"),
            "id: test_sdk\nsdk_version: 1.0.0\ncomponent_path:\n- path: components\n",
        );
    }

    /// Parse SDK + the given project YAML, resolve, and build a ParsedProject.
    pub fn build(&self, project_yaml: &str) -> ParsedProject {
        self.write_slcs();
        write_file(&self.project_dir.join("project.slcp"), project_yaml);
        let sdk = SDK::parse(self.sdk_root.join("test.slcs")).expect("sdk parses");
        let project =
            Project::parse(self.project_dir.join("project.slcp")).expect("project parses");
        let resolved = project.resolve_components(&sdk).expect("resolves");
        ParsedProject::new(&sdk, &project, &resolved)
    }

    /// Build and run generation into the out directory.
    pub fn generate(&self, project_yaml: &str) -> ParsedProject {
        let parsed = self.build(project_yaml);
        parsed.generate(&self.out_dir).expect("generation succeeds");
        parsed
    }

    /// Write the SDK `.slcs` and the project `.slcp` to disk and return their
    /// paths, without running the pipeline (for driving the CLI binary).
    pub fn prepare(&self, project_yaml: &str) -> (PathBuf, PathBuf) {
        self.write_slcs();
        let slcp = self.project_dir.join("project.slcp");
        write_file(&slcp, project_yaml);
        (self.sdk_root.join("test.slcs"), slcp)
    }
}

pub fn write_file(path: &Path, contents: &str) {
    if let Some(p) = path.parent() {
        fs::create_dir_all(p).unwrap();
    }
    fs::write(path, contents).unwrap();
}
