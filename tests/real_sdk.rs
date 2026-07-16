//! Opt-in parse test against a real on-disk SDK. Set `SLC_TEST_SDK` to the
//! directory of a Simplicity/Gecko SDK checkout to exercise the parser over
//! every `.slcc` it ships. Ignored by default because it needs local data.

use slc::Component;
use std::path::{Path, PathBuf};

fn find_slcc(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            find_slcc(&path, out);
        } else if path.extension().is_some_and(|e| e == "slcc") {
            out.push(path);
        }
    }
}

#[test]
#[ignore = "requires a real SDK; set SLC_TEST_SDK"]
fn every_component_in_real_sdk_parses() {
    let Ok(root) = std::env::var("SLC_TEST_SDK") else {
        eprintln!("SLC_TEST_SDK not set; skipping");
        return;
    };
    let root = PathBuf::from(root);

    let mut files = Vec::new();
    find_slcc(&root, &mut files);
    assert!(!files.is_empty(), "no .slcc files found under {root:?}");

    let mut failures = Vec::new();
    for f in &files {
        if let Err(e) = Component::parse(f, &root) {
            failures.push(format!("{}: {e}", f.display()));
        }
    }

    let total = files.len();
    let failed = failures.len();
    for msg in failures.iter().take(25) {
        eprintln!("PARSE FAIL {msg}");
    }
    eprintln!("parsed {} / {total} components", total - failed);
    assert_eq!(failed, 0, "{failed}/{total} components failed to parse");
}
