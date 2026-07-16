//! Generation-output conformance: directory layout, config copying, template
//! rendering, and the project `configuration` override mechanism.

mod common;
use common::{write_file, Fixture};

const BASE_PROJECT: &str = "project_name: p\nsdk: {id: test_sdk, version: 1}\n";

#[test]
fn standard_output_dirs_are_always_created() {
    let fx = Fixture::new();
    // No config files and no templates: the tree must still exist.
    fx.generate(BASE_PROJECT);
    for sub in ["autogen", "autogen/export", "config", "config/export"] {
        assert!(fx.out_file(sub).is_dir(), "missing {sub}/");
    }
}

#[test]
fn config_file_is_copied_into_config_dir() {
    let fx = Fixture::new();
    fx.component(
        "cfgcomp",
        "id: cfgcomp\nconfig_file:\n- {path: cfg/foo_config.h}\n",
    );
    fx.sdk_file("cfg/foo_config.h", "#define FOO_COUNT 7\n");

    fx.generate(&format!("{BASE_PROJECT}component:\n- {{id: cfgcomp}}\n"));
    assert!(fx.out_file("config/foo_config.h").is_file());
    assert_eq!(fx.read_out("config/foo_config.h"), "#define FOO_COUNT 7\n");
}

#[test]
fn exported_config_file_lands_in_config_export() {
    let fx = Fixture::new();
    fx.component(
        "cfgcomp",
        "id: cfgcomp\nconfig_file:\n- {path: cfg/exp.h, export: true}\n",
    );
    fx.sdk_file("cfg/exp.h", "#define X 1\n");
    fx.generate(&format!("{BASE_PROJECT}component:\n- {{id: cfgcomp}}\n"));
    assert!(fx.out_file("config/export/exp.h").is_file());
    assert!(!fx.out_file("config/exp.h").exists());
}

#[test]
fn existing_config_file_is_not_overwritten() {
    let fx = Fixture::new();
    fx.component(
        "cfgcomp",
        "id: cfgcomp\nconfig_file:\n- {path: cfg/foo_config.h}\n",
    );
    fx.sdk_file("cfg/foo_config.h", "#define FOO_COUNT 7\n");

    // Simulate a user-edited config already present in the output.
    write_file(
        &fx.out_file("config/foo_config.h"),
        "#define FOO_COUNT 42 // edited\n",
    );
    fx.generate(&format!("{BASE_PROJECT}component:\n- {{id: cfgcomp}}\n"));
    assert_eq!(
        fx.read_out("config/foo_config.h"),
        "#define FOO_COUNT 42 // edited\n"
    );
}

#[test]
fn configuration_rewrites_matching_define() {
    let fx = Fixture::new();
    fx.component(
        "cfgcomp",
        "id: cfgcomp\nconfig_file:\n- {path: cfg/foo_config.h}\n",
    );
    fx.sdk_file(
        "cfg/foo_config.h",
        "// <o FOO_COUNT> Count\n#define FOO_COUNT   7\n#define UNTOUCHED 3\n",
    );

    fx.generate(&format!(
        "{BASE_PROJECT}component:\n- {{id: cfgcomp}}\nconfiguration:\n- {{name: FOO_COUNT, value: '9'}}\n"
    ));
    let out = fx.read_out("config/foo_config.h");
    assert!(out.contains("#define FOO_COUNT 9"), "got: {out:?}");
    assert!(
        out.contains("#define UNTOUCHED 3"),
        "unrelated define changed: {out:?}"
    );
}

#[test]
fn configuration_last_matching_rule_wins() {
    let fx = Fixture::new();
    fx.component(
        "cfgcomp",
        "id: cfgcomp\nconfig_file:\n- {path: cfg/foo_config.h}\n",
    );
    fx.sdk_file("cfg/foo_config.h", "#define FOO 1\n");
    fx.generate(&format!(
        "{BASE_PROJECT}component:\n- {{id: cfgcomp}}\nconfiguration:\n- {{name: FOO, value: '2'}}\n- {{name: FOO, value: '3'}}\n"
    ));
    assert!(fx.read_out("config/foo_config.h").contains("#define FOO 3"));
}

#[test]
fn get_included_headers_recognizes_all_header_extensions() {
    let fx = Fixture::new();
    fx.component(
        "hdr",
        "id: hdr\ninclude:\n- path: inc\n  file_list:\n  - {path: a.h}\n  - {path: b.hpp}\n  - {path: c.hxx}\n  - {path: d.hh}\n  - {path: notheader.txt}\n",
    );
    let parsed = fx.build(&format!("{BASE_PROJECT}component:\n- {{id: hdr}}\n"));
    let names: Vec<String> = parsed
        .get_included_headers()
        .iter()
        .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
        .collect();
    for h in ["a.h", "b.hpp", "c.hxx", "d.hh"] {
        assert!(names.contains(&h.to_string()), "missing {h} in {names:?}");
    }
    assert!(!names.iter().any(|n| n == "notheader.txt"));
}

#[test]
fn template_contributions_order_by_priority_then_component_id() {
    // Priority orders low->high; equal priorities break by contributing
    // component id, independent of the order components are listed.
    let fx = Fixture::new();
    fx.component(
        "zed",
        "id: zed\ntemplate_contribution:\n- {name: x, value: 1, priority: 0}\n",
    );
    fx.component(
        "abc",
        "id: abc\ntemplate_contribution:\n- {name: x, value: 2, priority: 0}\n- {name: x, value: 3, priority: -5}\n",
    );
    // zed listed first to prove the result is sorted, not insertion order.
    let parsed = fx.build(&format!(
        "{BASE_PROJECT}component:\n- {{id: zed}}\n- {{id: abc}}\n"
    ));
    let order: Vec<String> = parsed.template_contribution["x"]
        .iter()
        .map(|v| v.to_string())
        .collect();
    assert_eq!(order, ["3", "2", "1"]);
}

#[test]
fn templates_route_to_autogen_and_export_with_banner() {
    let fx = Fixture::new();
    fx.component(
        "tmpl",
        "id: tmpl\ntemplate_file:\n- {path: t/a.c.jinja}\n- {path: t/b.c.jinja, export: true}\n",
    );
    fx.sdk_file("t/a.c.jinja", "{{ autogenerated_c_file }}\nint a;\n");
    fx.sdk_file("t/b.c.jinja", "int b;\n");

    fx.generate(&format!("{BASE_PROJECT}component:\n- {{id: tmpl}}\n"));

    assert!(fx.out_file("autogen/a.c").is_file());
    assert!(fx.out_file("autogen/export/b.c").is_file());
    assert!(!fx.out_file("autogen/b.c").exists());

    let a = fx.read_out("autogen/a.c");
    assert!(
        a.starts_with("// This file is autogenerated"),
        "banner missing: {a:?}"
    );
    assert!(
        a.contains("Source template file: a.c.jinja"),
        "banner name missing: {a:?}"
    );
}

#[test]
fn template_without_jinja_suffix_still_renders() {
    let fx = Fixture::new();
    fx.component("tmpl", "id: tmpl\ntemplate_file:\n- {path: t/plain.txt}\n");
    fx.sdk_file("t/plain.txt", "hello\n");
    fx.generate(&format!("{BASE_PROJECT}component:\n- {{id: tmpl}}\n"));
    assert_eq!(fx.read_out("autogen/plain.txt"), "hello\n");
}

#[test]
fn template_contribution_values_are_available_and_merged() {
    let fx = Fixture::new();
    fx.component(
        "c",
        "id: c\ntemplate_file:\n- {path: t/list.txt}\ntemplate_contribution:\n- {name: nums, value: 10}\n- {name: nums, value: 20}\n",
    );
    fx.sdk_file("t/list.txt", "{% for n in nums %}{{ n }},{% endfor %}");
    fx.generate(&format!("{BASE_PROJECT}component:\n- {{id: c}}\n"));
    assert_eq!(fx.read_out("autogen/list.txt"), "10,20,");
}

#[test]
fn mutable_list_append_dedup_idiom_renders() {
    // jinja2 dedup idiom: `{% set seen = [] %}` then `seen.append(x)`, which
    // minijinja can't do natively (immutable values) — exercised by core SDK
    // templates like sl_event_handler.c.
    let fx = Fixture::new();
    fx.component(
        "c",
        "id: c\ntemplate_file:\n- {path: t/dedup.txt}\ntemplate_contribution:\n- {name: items, value: a}\n- {name: items, value: b}\n- {name: items, value: a}\n- {name: items, value: c}\n",
    );
    fx.sdk_file(
        "t/dedup.txt",
        "{% set seen = [] %}{% for x in items %}{% if x not in seen %}{% if seen.append(x) %}{% endif %}{{ x }}{% endif %}{% endfor %}",
    );
    fx.generate(&format!("{BASE_PROJECT}component:\n- {{id: c}}\n"));
    assert_eq!(fx.read_out("autogen/dedup.txt"), "abc");
}
