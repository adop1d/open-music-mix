//! Every `#### Scenario:` in the change specs must have a same-name test (task 8.1).

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

const SPECS: &[&str] = &[
    "playback-queue",
    "local-library",
    "stream-playback",
    "spotify-import",
    "tui-shell",
    "album-art",
    "session-persistence",
];

fn slug(scenario: &str) -> String {
    let mut slug = String::new();
    let mut underscore = false;
    for character in scenario.chars() {
        if character.is_ascii_alphanumeric() {
            slug.push(character.to_ascii_lowercase());
            underscore = false;
        } else if !underscore && !slug.is_empty() {
            slug.push('_');
            underscore = true;
        }
    }
    while slug.ends_with('_') {
        slug.pop();
    }
    slug
}

fn scenario_names() -> BTreeMap<String, String> {
    let mut scenarios = BTreeMap::new();
    for spec in SPECS {
        let path = format!(
            "{}/openspec/changes/add-music-tui/specs/{spec}/spec.md",
            env!("CARGO_MANIFEST_DIR")
        );
        let text = std::fs::read_to_string(&path).unwrap_or_else(|error| {
            panic!("cannot read {path}: {error}");
        });
        for line in text.lines() {
            if let Some(name) = line.trim().strip_prefix("#### Scenario: ") {
                let name = name.trim().to_string();
                let slug = slug(&name);
                if let Some(previous) = scenarios.insert(slug.clone(), spec.to_string()) {
                    panic!("scenario {name:?} appears in both {previous} and {spec}");
                }
            }
        }
    }
    assert_eq!(scenarios.len(), 86, "expected 86 scenarios");
    scenarios
}

fn test_names() -> BTreeSet<String> {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests");
    let mut names = BTreeSet::new();
    for entry in std::fs::read_dir(directory).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().and_then(|value| value.to_str()) != Some("rs") {
            continue;
        }
        let text = std::fs::read_to_string(&path).unwrap();
        for line in text.lines() {
            let line = line.trim();
            let Some((_, rest)) = line.split_once("fn ") else {
                continue;
            };
            let name: String = rest
                .chars()
                .take_while(|character| character.is_ascii_alphanumeric() || *character == '_')
                .collect();
            if !name.is_empty() {
                names.insert(name);
            }
        }
    }
    names
}

#[test]
fn every_scenario_has_a_matching_test() {
    let scenarios = scenario_names();
    let tests = test_names();
    let missing: Vec<_> = scenarios
        .iter()
        .filter(|(slug, _)| !tests.contains(*slug))
        .map(|(slug, spec)| format!("{spec}: {slug}"))
        .collect();
    assert!(
        missing.is_empty(),
        "scenarios without a same-name test:\n{}",
        missing.join("\n")
    );
}
