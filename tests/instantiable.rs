//! Instantiable-component conformance: per-instance config file expansion,
//! the {{instance}} path substitution stages, the INSTANCE content transform,
//! per-instance defines, and instance-targeted config overrides.

mod common;
use common::Fixture;

const BASE: &str = "project_name: p\nsdk: {id: test_sdk, version: 1}\n";

#[test]
fn instantiable_config_file_expands_per_instance() {
    let fx = Fixture::new();
    // Block style: a plain scalar path with an embedded {{instance}} token.
    fx.component(
        "simple_button",
        "id: simple_button\ninstantiable:\n  prefix: btn\nconfig_file:\n- path: cfg/btncfg_{{instance}}.h\n  file_id: btn_cfg\n",
    );
    // Source on disk is named with the prefix (SDK stage) and its content uses
    // the literal INSTANCE token.
    fx.sdk_file("cfg/btncfg_btn.h", "#define BTNCFG_INSTANCE_ENABLE 1\n");

    fx.generate(&format!(
        "{BASE}component:\n- id: simple_button\n  instance: [btn0, btn1]\n"
    ));

    // One config file per instance, named with the instance (project stage).
    assert_eq!(
        fx.read_out("config/btncfg_btn0.h"),
        "#define BTNCFG_BTN0_ENABLE 1\n"
    );
    assert_eq!(
        fx.read_out("config/btncfg_btn1.h"),
        "#define BTNCFG_BTN1_ENABLE 1\n"
    );
    assert!(!fx.out_file("config/btncfg_btn.h").exists());
}

#[test]
fn xml_instance_content_is_substituted_verbatim() {
    let fx = Fixture::new();
    fx.component(
        "radio",
        "id: radio\ninstantiable:\n  prefix: r\nconfig_file:\n- path: cfg/radio_{{instance}}.xml\n  file_id: r_cfg\n",
    );
    // XML content keeps the {{instance}} placeholder, replaced verbatim.
    fx.sdk_file("cfg/radio_r.xml", "<radio name=\"{{instance}}\"/>\n");
    fx.generate(&format!(
        "{BASE}component:\n- id: radio\n  instance: [r0]\n"
    ));
    assert_eq!(fx.read_out("config/radio_r0.xml"), "<radio name=\"r0\"/>\n");
}

#[test]
fn instantiable_defines_expand_per_instance() {
    let fx = Fixture::new();
    fx.component(
        "led",
        "id: led\ninstantiable:\n  prefix: led\ndefine:\n- name: 'SL_{{instance}}_COUNT'\n  value: '4'\n",
    );
    let parsed = fx.build(&format!(
        "{BASE}component:\n- id: led\n  instance: [led0, led1]\n"
    ));
    let names: Vec<&str> = parsed.define.iter().map(|d| d.name.as_str()).collect();
    assert!(names.contains(&"SL_led0_COUNT"), "got {names:?}");
    assert!(names.contains(&"SL_led1_COUNT"), "got {names:?}");
}

#[test]
fn override_targets_a_single_instance() {
    let fx = Fixture::new();
    fx.component(
        "btn",
        "id: btn\ninstantiable:\n  prefix: btn\nconfig_file:\n- path: cfg/b_{{instance}}.h\n  file_id: bcfg\n",
    );
    fx.sdk_file("cfg/b_btn.h", "orig\n");
    // Project supplies a replacement for the btn0 instance only.
    fx.project_file("over_btn0.h", "overridden\n");

    fx.generate(&format!(
        "{BASE}config_file:\n- path: over_btn0.h\n  override:\n    file_id: bcfg\n    component: btn\n    instance: btn0\ncomponent:\n- id: btn\n  instance: [btn0, btn1]\n"
    ));

    assert_eq!(fx.read_out("config/b_btn0.h"), "overridden\n");
    assert_eq!(fx.read_out("config/b_btn1.h"), "orig\n");
}
