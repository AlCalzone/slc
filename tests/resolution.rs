//! Dependency-resolution conformance against SLC 1.2 `features/`.

use slc::{Component, Project, ResolveError, SDK};
use std::path::PathBuf;
use std::rc::Rc;

fn mk_sdk(components: &[&str]) -> SDK {
    let comps = components
        .iter()
        .map(|y| Rc::new(Component::from_str(y, ".").expect("component parses")))
        .collect();
    SDK::from_components("test_sdk", PathBuf::from("."), comps)
}

fn mk_project(yaml: &str) -> Project {
    Project::from_str(yaml, PathBuf::from(".")).expect("project parses")
}

fn ids(sdk_result: &slc::ResolveResult) -> Vec<String> {
    let mut v: Vec<String> = sdk_result.components.iter().map(|c| c.id.clone()).collect();
    v.sort();
    v
}

#[test]
fn single_provider_is_pulled_in() {
    // Regression canary for the requires/provides swap bug: a project that
    // requires `uart` must pull in the one SDK component that provides it.
    let sdk = mk_sdk(&["id: uart_comp\nprovides:\n- {name: uart}\n"]);
    let project =
        mk_project("project_name: p\nsdk: {id: s, version: 1}\nrequires:\n- {name: uart}\n");

    let r = project.resolve_components(&sdk).expect("resolves");
    assert_eq!(ids(&r), ["uart_comp"]);
    assert!(r.provided_features.contains("uart"));
}

#[test]
fn project_provides_satisfies_component_requirement() {
    // A feature the project itself `provides` must count as provided (P), so a
    // listed component requiring it resolves without pulling an SDK provider.
    let sdk = mk_sdk(&["id: needs_foo\nrequires:\n- {name: foo}\n"]);
    let project = mk_project(
        "project_name: p\nsdk: {id: s, version: 1}\nprovides:\n- {name: foo}\ncomponent:\n- {id: needs_foo}\n",
    );

    let r = project.resolve_components(&sdk).expect("resolves");
    assert_eq!(ids(&r), ["needs_foo"]);
    assert!(r.provided_features.contains("foo"));
}

#[test]
fn conditional_provide_activates_only_when_condition_met() {
    // `feat_b` is provided by A only when `feat_a` is present. With B (which
    // provides feat_a) in the set, the fixpoint must surface feat_b.
    let a = "id: a\nprovides:\n- name: feat_b\n  condition: [feat_a]\n";
    let b = "id: b\nprovides:\n- {name: feat_a}\n";
    let sdk = mk_sdk(&[a, b]);

    let with_b =
        mk_project("project_name: p\nsdk: {id: s, version: 1}\ncomponent:\n- {id: a}\n- {id: b}\n");
    let r = with_b.resolve_components(&sdk).expect("resolves");
    assert!(r.provided_features.contains("feat_a"));
    assert!(r.provided_features.contains("feat_b"));

    // Without B, feat_a is absent so A's conditional provide stays inactive.
    let without_b =
        mk_project("project_name: p\nsdk: {id: s, version: 1}\ncomponent:\n- {id: a}\n");
    let r2 = without_b.resolve_components(&sdk).expect("resolves");
    assert!(!r2.provided_features.contains("feat_b"));
}

#[test]
fn conflicting_features_are_rejected() {
    // A conflicts with feature `y`; B provides `y`. Including both fails.
    let a = "id: a\nprovides:\n- {name: x}\nconflicts:\n- {name: y}\n";
    let b = "id: b\nprovides:\n- {name: y}\n";
    let sdk = mk_sdk(&[a, b]);
    let project =
        mk_project("project_name: p\nsdk: {id: s, version: 1}\ncomponent:\n- {id: a}\n- {id: b}\n");

    match project.resolve_components(&sdk) {
        Err(ResolveError::ConflictingFeatures(fs)) => assert!(fs.contains(&"y".to_string())),
        other => panic!("expected ConflictingFeatures, got {other:?}"),
    }
}

#[test]
fn duplicate_provide_without_allow_multiple_fails() {
    let a = "id: a\nprovides:\n- {name: rtos}\n";
    let b = "id: b\nprovides:\n- {name: rtos}\n";
    let sdk = mk_sdk(&[a, b]);
    let project =
        mk_project("project_name: p\nsdk: {id: s, version: 1}\ncomponent:\n- {id: a}\n- {id: b}\n");

    match project.resolve_components(&sdk) {
        Err(ResolveError::DuplicateProvide {
            feature,
            components,
        }) => {
            assert_eq!(feature, "rtos");
            assert_eq!(components, ["a", "b"]);
        }
        other => panic!("expected DuplicateProvide, got {other:?}"),
    }
}

#[test]
fn duplicate_provide_allowed_when_all_set_allow_multiple() {
    let a = "id: a\nprovides:\n- {name: rtos, allow_multiple: true}\n";
    let b = "id: b\nprovides:\n- {name: rtos, allow_multiple: true}\n";
    let sdk = mk_sdk(&[a, b]);
    let project =
        mk_project("project_name: p\nsdk: {id: s, version: 1}\ncomponent:\n- {id: a}\n- {id: b}\n");
    assert!(project.resolve_components(&sdk).is_ok());
}

#[test]
fn duplicate_provide_fails_if_only_some_allow_multiple() {
    // The flag must be set on *every* provider; a single unmarked one fails.
    let a = "id: a\nprovides:\n- {name: rtos, allow_multiple: true}\n";
    let b = "id: b\nprovides:\n- {name: rtos}\n";
    let sdk = mk_sdk(&[a, b]);
    let project =
        mk_project("project_name: p\nsdk: {id: s, version: 1}\ncomponent:\n- {id: a}\n- {id: b}\n");
    assert!(matches!(
        project.resolve_components(&sdk),
        Err(ResolveError::DuplicateProvide { .. })
    ));
}

#[test]
fn unknown_component_is_reported() {
    let sdk = mk_sdk(&["id: real\n"]);
    let project =
        mk_project("project_name: p\nsdk: {id: s, version: 1}\ncomponent:\n- {id: ghost}\n");
    assert!(matches!(
        project.resolve_components(&sdk),
        Err(ResolveError::UnknownComponent(id)) if id == "ghost"
    ));
}

#[test]
fn unsatisfiable_requirement_is_reported() {
    let sdk = mk_sdk(&["id: real\n"]);
    let project =
        mk_project("project_name: p\nsdk: {id: s, version: 1}\nrequires:\n- {name: nonexistent}\n");
    assert!(matches!(
        project.resolve_components(&sdk),
        Err(ResolveError::UnsatisfiedRequirement(req)) if req == "nonexistent"
    ));
}

#[test]
fn multiple_providers_are_ambiguous_without_a_recommendation() {
    let free = "id: freertos\nprovides:\n- {name: rtos}\n";
    let micrium = "id: micrium\nprovides:\n- {name: rtos}\n";
    let app = "id: app\nprovides:\n- {name: app_feat}\nrequires:\n- {name: rtos}\n";
    let sdk = mk_sdk(&[free, micrium, app]);
    let project =
        mk_project("project_name: p\nsdk: {id: s, version: 1}\ncomponent:\n- {id: app}\n");

    match project.resolve_components(&sdk) {
        Err(ResolveError::AmbiguousRequirement {
            requirement,
            candidates,
        }) => {
            assert_eq!(requirement, "rtos");
            assert_eq!(candidates.len(), 2);
        }
        other => panic!("expected AmbiguousRequirement, got {other:?}"),
    }
}

#[test]
fn recommendation_resolves_ambiguity() {
    // Same ambiguous setup, but `app` recommends freertos: it is selected and
    // micrium is left out.
    let free = "id: freertos\nprovides:\n- {name: rtos}\n";
    let micrium = "id: micrium\nprovides:\n- {name: rtos}\n";
    let app = "id: app\nrequires:\n- {name: rtos}\nrecommends:\n- {id: freertos}\n";
    let sdk = mk_sdk(&[free, micrium, app]);
    let project =
        mk_project("project_name: p\nsdk: {id: s, version: 1}\ncomponent:\n- {id: app}\n");

    let r = project
        .resolve_components(&sdk)
        .expect("resolves via recommendation");
    assert_eq!(ids(&r), ["app", "freertos"]);
}

#[test]
fn shared_dependency_added_once_and_no_duplicates() {
    // Two components require the same feature; its single provider is added
    // exactly once (guards against re-adding an already-present component).
    let a = "id: a\nrequires:\n- {name: x}\n";
    let b = "id: b\nrequires:\n- {name: x}\n";
    let xprov = "id: xprov\nprovides:\n- {name: x}\n";
    let sdk = mk_sdk(&[a, b, xprov]);
    let project =
        mk_project("project_name: p\nsdk: {id: s, version: 1}\ncomponent:\n- {id: a}\n- {id: b}\n");

    let r = project.resolve_components(&sdk).expect("resolves");
    assert_eq!(ids(&r), ["a", "b", "xprov"]);
    let unique: std::collections::BTreeSet<_> = r.components.iter().map(|c| &c.id).collect();
    assert_eq!(unique.len(), r.components.len(), "no duplicate components");
}

#[test]
fn candidate_with_unconditional_conflict_on_absent_feature_is_selectable() {
    // Mirrors the real emlib_cmu case: it provides `emlib_cmu` but declares an
    // unconditional conflict with `device_series_3`. Since that feature is
    // absent, the candidate must still be selectable.
    let cmu = "id: cmu\nprovides:\n- {name: emlib_cmu}\nconflicts:\n- {name: device_series_3}\n";
    let sdk = mk_sdk(&[cmu]);
    let project =
        mk_project("project_name: p\nsdk: {id: s, version: 1}\nrequires:\n- {name: emlib_cmu}\n");
    let r = project.resolve_components(&sdk).expect("resolves");
    assert_eq!(ids(&r), ["cmu"]);
}

#[test]
fn instance_names_are_collected() {
    let sdk = mk_sdk(&["id: rail_util_init\ninstantiable:\n  prefix: inst\n"]);
    let project = mk_project(
        "project_name: p\nsdk: {id: s, version: 1}\ncomponent:\n- id: rail_util_init\n  instance: [inst0, inst1]\n",
    );
    let r = project.resolve_components(&sdk).expect("resolves");
    assert_eq!(ids(&r), ["rail_util_init"]);
    assert_eq!(r.instances["rail_util_init"], ["inst0", "inst1"]);
}
