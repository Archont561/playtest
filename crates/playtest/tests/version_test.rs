use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fs;
use std::path::Path;

use playtest::VERSION;
use rstest::rstest;

fn parse_cargo_lock_graph(lock_text: &str) -> BTreeMap<String, BTreeSet<String>> {
    let mut graph: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for block in lock_text.split("[[package]]").skip(1) {
        let mut pkg_name: Option<String> = None;
        let mut in_deps = false;
        let mut deps = BTreeSet::new();
        for raw_line in block.lines() {
            let line = raw_line.trim();
            if !in_deps {
                if let Some(rest) = line.strip_prefix("name = ") {
                    pkg_name = Some(rest.trim_matches('"').to_owned());
                } else if line.starts_with("dependencies = [") {
                    in_deps = !line.ends_with(']');
                }
            } else if line.starts_with(']') {
                in_deps = false;
            } else {
                let entry = line.trim_end_matches(',').trim().trim_matches('"');
                if let Some(dep_name) = entry.split_whitespace().next()
                    && !dep_name.is_empty()
                {
                    deps.insert(dep_name.to_owned());
                }
            }
        }
        if let Some(name) = pkg_name {
            graph.entry(name).or_default().extend(deps);
        }
    }
    graph
}

fn reachable_from(graph: &BTreeMap<String, BTreeSet<String>>, start: &str) -> BTreeSet<String> {
    let mut visited = BTreeSet::new();
    let mut queue = VecDeque::new();
    if let Some(direct) = graph.get(start) {
        for dep in direct {
            queue.push_back(dep.clone());
        }
    }
    while let Some(node) = queue.pop_front() {
        if !visited.insert(node.clone()) {
            continue;
        }
        if let Some(children) = graph.get(&node) {
            for child in children {
                if !visited.contains(child) {
                    queue.push_back(child.clone());
                }
            }
        }
    }
    visited
}

#[rstest]
fn test_version_matches_workspace() {
    assert_eq!(VERSION, env!("CARGO_PKG_VERSION"));
    assert_eq!(VERSION, playtest_core::VERSION);
}

#[rstest]
fn test_workspace_dependency_boundaries() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let Some(lock_path) = manifest_dir
        .parent()
        .and_then(Path::parent)
        .map(|root| root.join("Cargo.lock"))
        .filter(|path| path.is_file())
    else {
        return;
    };
    let lock_text = fs::read_to_string(&lock_path)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", lock_path.display()));
    let graph = parse_cargo_lock_graph(&lock_text);

    for forbidden in ["playtest-assets", "playtest-cli", "clifacet"] {
        for lib_crate in ["playtest", "playtest-core", "playtest-macros"] {
            let closure = reachable_from(&graph, lib_crate);
            assert!(
                !closure.contains(forbidden),
                "Spec 0012 §1 violation: `{lib_crate}` must never depend on `{forbidden}`"
            );
        }
    }

    for workspace_crate in [
        "playtest",
        "playtest-core",
        "playtest-macros",
        "playtest-assets",
    ] {
        let direct = graph.get(workspace_crate).cloned().unwrap_or_default();
        assert!(
            !direct.contains("playtest-assets") && !direct.contains("clifacet"),
            "Spec 0012 §1 violation: only `playtest-cli` may depend on `playtest-assets` or `clifacet` (found on `{workspace_crate}`)"
        );
    }
}
