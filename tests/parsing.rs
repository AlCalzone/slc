//! Parsing conformance for `.slcp` / `.slcc` against the SLC 1.2 spec.

use slc::{substitute_instance, Component, Library, Project, Quality};
use std::path::PathBuf;

fn parse_project(yaml: &str) -> Project {
    Project::from_str(yaml, PathBuf::from(".")).expect("project should parse")
}

fn parse_component(yaml: &str) -> Component {
    Component::from_str(yaml, PathBuf::from(".")).expect("component should parse")
}

#[test]
fn project_parses_from_string_without_touching_disk() {
    // `from_str` accepts the `.slcp` body in memory, so callers need not write
    // a temporary file just to parse a project.
    let root = PathBuf::from("/nonexistent/project/dir");
    let p = Project::from_str("project_name: p\nsdk: {id: s, version: 1}\n", root.clone())
        .expect("project parses from a string");
    assert_eq!(p.project_name, "p");
    assert_eq!(p.root_path, root);
}

#[test]
fn project_name_accepts_name_alias() {
    // Spec: `name` is a backwards-compatible alias for `project_name`.
    let p = parse_project("name: blink\nsdk: {id: s, version: 1}\n");
    assert_eq!(p.project_name, "blink");
}

#[test]
fn component_id_accepts_name_alias() {
    // Spec: `name` is a backwards-compatible alias for a component `id`.
    let c = parse_component("name: my_comp\n");
    assert_eq!(c.id, "my_comp");
}

#[test]
fn quality_legacy_values_are_remapped() {
    // Spec remap: test -> experimental, unknown-ish -> evaluation, production/internal identity.
    assert_eq!(Quality::from_str_lossy("production"), Quality::Production);
    assert_eq!(Quality::from_str_lossy("internal"), Quality::Internal);
    assert_eq!(Quality::from_str_lossy("test"), Quality::Experimental);
    assert_eq!(Quality::from_str_lossy("evaluation"), Quality::Evaluation);
    assert_eq!(Quality::from_str_lossy("deprecated"), Quality::Deprecated);
    assert_eq!(Quality::from_str_lossy("banana"), Quality::Evaluation);
}

#[test]
fn quality_parses_from_component() {
    let c = parse_component("id: c\nquality: test\n");
    assert_eq!(c.quality, Some(Quality::Experimental));
}

#[test]
fn omap_tag_with_blank_and_short_lines_does_not_panic() {
    // Real gecko/simplicity `.slcc` files are emitted as `!!omap` ordered maps.
    // The tag line is dropped and the 2-char list prefix stripped from the body.
    // Blank and 1-char lines must not cause a byte-index panic.
    let yaml = "!!omap\n- id: omap_comp\n\n- provides:\n  - name: feat_a\n";
    let c = parse_component(yaml);
    assert_eq!(c.id, "omap_comp");
    let provides = c.provides.expect("provides parsed from omap body");
    assert_eq!(provides[0].name, "feat_a");
}

#[test]
fn component_instance_and_from_are_parsed() {
    // Spec: a `.slcp` component entry may carry `instance` names and a `from` extension.
    let p = parse_project(
        "project_name: p\nsdk: {id: s, version: 1}\ncomponent:\n- {id: plain}\n- id: rail_util_init\n  instance: [inst0, inst1]\n- {id: ext_comp, from: my_extension}\n",
    );
    let comps = p.component.unwrap();
    assert_eq!(comps[0].id, "plain");
    assert!(comps[0].instance.is_none());
    assert_eq!(comps[1].instance.as_deref().unwrap(), ["inst0", "inst1"]);
    assert_eq!(comps[2].from.as_deref(), Some("my_extension"));
}

#[test]
fn instantiable_prefix_is_parsed() {
    let c = parse_component("id: c\ninstantiable:\n  prefix: inst\n");
    assert_eq!(c.instantiable.unwrap().prefix, "inst");
}

#[test]
fn configuration_section_is_parsed() {
    let p = parse_project(
        "project_name: p\nsdk: {id: s, version: 1}\nconfiguration:\n- {name: SL_STACK_SIZE, value: '2048'}\n- condition: [device_series_2]\n  name: FOO\n  value: '1'\n",
    );
    let cfg = p.configuration.unwrap();
    assert_eq!(cfg[0].name, "SL_STACK_SIZE");
    assert_eq!(cfg[0].value, "2048");
    assert_eq!(cfg[1].condition.as_deref().unwrap(), ["device_series_2"]);
}

#[test]
fn unmodeled_keys_do_not_break_parsing() {
    // A realistic project carries many keys the generator does not act on yet
    // (filter, readme, category, package, toolchain_settings). These must not
    // fail the parse; the modeled keys must still be captured.
    let p = parse_project(
        "project_name: rich\nlabel: Rich\ncategory: Example\npackage: Rail\nquality: production\nfilter:\n- {name: Device Type, value: [SoC]}\nreadme:\n- {path: readme.md}\ntoolchain_settings:\n- {value: debug, option: optimize}\nsdk: {id: simplicity_sdk, version: 2024.12.1}\nrequires:\n- {name: a_radio_config}\n",
    );
    assert_eq!(p.project_name, "rich");
    assert_eq!(p.quality, Some(Quality::Production));
    assert_eq!(p.toolchain_settings.unwrap()[0].option, "optimize");
    assert_eq!(p.requires.unwrap()[0].name, "a_radio_config");
}

#[test]
fn define_value_accepts_any_yaml_scalar() {
    // Real SDK components write integer/bool `#define` values unquoted. Each
    // must parse to its string form rather than failing the whole component.
    let c = parse_component(
        "id: c\ndefine:\n- {name: A_INT, value: 4}\n- {name: A_FLOAT, value: 1.5}\n- {name: A_BOOL, value: true}\n- {name: A_STR, value: '7'}\n- {name: A_NONE}\n",
    );
    let defines = c.define.unwrap();
    assert_eq!(defines[0].value.as_deref(), Some("4"));
    assert_eq!(defines[1].value.as_deref(), Some("1.5"));
    assert_eq!(defines[2].value.as_deref(), Some("true"));
    assert_eq!(defines[3].value.as_deref(), Some("7"));
    assert_eq!(defines[4].value, None);
}

#[test]
fn define_with_integer_value_does_not_skip_component() {
    // The bug: an unquoted integer failed `Option<String>` serde, aborting the
    // component parse so it was dropped with a warning. It must parse instead.
    let c = parse_component("id: with_int\ndefine:\n- name: SL_COUNT\n  value: 12\n");
    assert_eq!(c.id, "with_int");
    assert_eq!(c.define.unwrap()[0].value.as_deref(), Some("12"));
}

#[test]
fn library_system_and_path_variants_parse() {
    let c = parse_component(
        "id: c\nlibrary:\n- system: gcc\n  unless: [device_host]\n- path: lib/libfoo.a\n",
    );
    let libs = c.library.unwrap();
    assert!(matches!(&libs[0], Library::System(s) if s.system == "gcc"));
    assert!(matches!(&libs[1], Library::SDK(s) if s.path == "lib/libfoo.a"));
}

#[test]
fn substitute_instance_replaces_placeholder() {
    assert_eq!(
        substitute_instance("sl_rail_util_init_{{instance}}_config.h", "inst0"),
        "sl_rail_util_init_inst0_config.h"
    );
    // Whitespace inside the braces is tolerated.
    assert_eq!(substitute_instance("a_{{ instance }}_b", "x"), "a_x_b");
    // Unrelated double-brace tokens are left untouched.
    assert_eq!(
        substitute_instance("{{other}}_{{instance}}", "z"),
        "{{other}}_z"
    );
    // No placeholder: unchanged.
    assert_eq!(substitute_instance("plain.h", "z"), "plain.h");
}

#[test]
fn configuration_accepts_any_scalar_value() {
    // Real .slcp files write configuration values unquoted (numbers,
    // booleans); every scalar deserializes into its string form.
    let p = parse_project(
        "project_name: p\nsdk: {id: s, version: 1}\nconfiguration:\n- {name: A, value: 9}\n- {name: B, value: true}\n- {name: C, value: SL_FOO}\n",
    );
    let cfg = p.configuration.unwrap();
    assert_eq!(cfg[0].value, "9");
    assert_eq!(cfg[1].value, "true");
    assert_eq!(cfg[2].value, "SL_FOO");
}
