//! End-to-end tests driving the built `slc` binary.

mod common;
use common::Fixture;
use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_slc");

#[test]
fn generate_writes_config_and_autogen_trees() {
    let fx = Fixture::new();
    fx.component(
        "cfgcomp",
        "id: cfgcomp\nconfig_file:\n- {path: cfg/foo.h}\n",
    );
    fx.sdk_file("cfg/foo.h", "#define A 1\n");
    let (slcs, slcp) = fx
        .prepare("project_name: p\nsdk: {id: test_sdk, version: 1}\ncomponent:\n- {id: cfgcomp}\n");

    let status = Command::new(BIN)
        .arg("generate")
        .arg("--sdk")
        .arg(&slcs)
        .arg("--output")
        .arg(&fx.out_dir)
        .arg(&slcp)
        .status()
        .expect("run slc");

    assert!(status.success(), "expected success, got {status}");
    assert!(fx.out_file("config/foo.h").is_file());
    assert!(fx.out_file("autogen").is_dir());
}

#[test]
fn sdk_argument_accepts_a_directory() {
    let fx = Fixture::new();
    fx.component("c", "id: c\n");
    let (_slcs, slcp) =
        fx.prepare("project_name: p\nsdk: {id: test_sdk, version: 1}\ncomponent:\n- {id: c}\n");

    // Pass the SDK directory rather than the .slcs file path.
    let status = Command::new(BIN)
        .arg("generate")
        .arg("--sdk")
        .arg(&fx.sdk_root)
        .arg("--output")
        .arg(&fx.out_dir)
        .arg(&slcp)
        .status()
        .expect("run slc");
    assert!(status.success());
}

#[test]
fn resolution_failure_exits_nonzero() {
    let fx = Fixture::new();
    fx.component("real", "id: real\n");
    let (slcs, slcp) =
        fx.prepare("project_name: p\nsdk: {id: test_sdk, version: 1}\ncomponent:\n- {id: ghost}\n");

    let output = Command::new(BIN)
        .arg("generate")
        .arg("--sdk")
        .arg(&slcs)
        .arg(&slcp)
        .output()
        .expect("run slc");

    assert!(!output.status.success(), "expected failure exit code");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("ghost"),
        "error should name the component: {stderr}"
    );
}

#[test]
fn missing_arguments_exit_nonzero() {
    let output = Command::new(BIN).output().expect("run slc");
    assert!(!output.status.success());
}
