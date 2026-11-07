// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The generated project: the same seed gives the same project, and purlis-core reads it as
//! a project of that size.

use std::collections::BTreeMap;
use std::path::Path;

use scale_project::{Plan, Size, generate};

fn plan(seed: u64) -> Plan {
    Plan {
        seed,
        size: Size::small(),
        at: chrono::NaiveDate::from_ymd_opt(2026, 11, 6)
            .and_then(|day| day.and_hms_opt(12, 0, 0))
            .expect("a moment"),
    }
}

/// Every file under `root`, by its path relative to it, with its bytes.
fn tree(root: &Path) -> BTreeMap<String, Vec<u8>> {
    fn walk(root: &Path, dir: &Path, into: &mut BTreeMap<String, Vec<u8>>) {
        for entry in std::fs::read_dir(dir).expect("a readable directory") {
            let path = entry.expect("an entry").path();
            if path.is_dir() {
                walk(root, &path, into);
            } else {
                let name = path.strip_prefix(root).expect("under the root");
                into.insert(
                    name.to_string_lossy().into_owned(),
                    std::fs::read(&path).expect("a readable file"),
                );
            }
        }
    }
    let mut into = BTreeMap::new();
    walk(root, root, &mut into);
    into
}

fn generated(seed: u64) -> (tempfile::TempDir, BTreeMap<String, Vec<u8>>) {
    let dir = tempfile::tempdir().expect("a temporary directory");
    generate(&plan(seed), dir.path()).expect("the project is written");
    let files = tree(dir.path());
    (dir, files)
}

#[test]
fn the_same_seed_writes_the_same_project_byte_for_byte() {
    let (_one, first) = generated(5497);
    let (_two, second) = generated(5497);
    assert!(
        first.len() > 50,
        "a project of a few files: {:?}",
        first.keys()
    );
    assert_eq!(
        first.keys().collect::<Vec<_>>(),
        second.keys().collect::<Vec<_>>()
    );
    for (name, bytes) in &first {
        assert_eq!(Some(bytes), second.get(name), "{name} differs");
    }
}

#[test]
fn a_different_seed_writes_a_different_project() {
    let (_one, first) = generated(5497);
    let (_two, second) = generated(5498);
    assert_ne!(first, second);
}

#[test]
fn purlis_core_reads_the_project_at_the_size_planned() {
    use purlis_core::workspaces::Plane;

    let dir = tempfile::tempdir().expect("a temporary directory");
    let root = dir.path();
    let plan = plan(5497);
    generate(&plan, root).expect("the project is written");
    let size = plan.size;

    let model = purlis_core::planemodel::Model::read(root);
    assert_eq!(
        model.rows().expect("workspaces listed").count(),
        size.workspaces
    );
    let personas = model.personas().expect("personas listed");
    assert_eq!(
        personas
            .iter()
            .filter(|name| !name.starts_with('_'))
            .count(),
        size.personas,
        "{personas:?}"
    );

    let plane = Plane::open(root);
    let (mut memories, mut todos, mut sessions) = (0, 0, 0);
    for name in plane.workspaces().expect("workspaces") {
        assert!(plane.is_live(&name), "{name} is LIVE");
        let workspace = plane.workspace(&name).expect("a workspace");
        memories += workspace.memories().expect("memories").len();
        todos += workspace.todos().expect("todos").len();
        sessions += purlis_core::sessionrecord::list(
            root,
            &purlis_core::active::Place::Workspace(name.clone()),
        )
        .len();
        let (missing, unlisted) =
            purlis_core::memstore::index_drift(root, &workspace.dir().join("memory"))
                .expect("its memory is readable");
        assert!(
            missing.is_empty() && unlisted.is_empty(),
            "{name}'s index drifts"
        );
    }
    assert_eq!(memories, size.workspace_memories);
    assert_eq!(todos, size.todos_per_workspace * size.workspaces);
    assert_eq!(sessions, size.sessions_per_workspace * size.workspaces);

    let in_personas: usize = personas
        .iter()
        .filter(|name| !name.starts_with('_'))
        .map(|name| purlis_core::personas::memory_count(root, name))
        .sum();
    assert_eq!(in_personas, size.persona_memories);
    assert_eq!(
        purlis_core::personas::memory_count(root, "_shared"),
        size.shared_memories
    );

    let records = purlis_core::dispatchrecord::list(root);
    assert_eq!(records.len(), size.dispatches);
    assert_eq!(
        records.iter().filter(|record| record.running()).count(),
        size.running_dispatches
    );
}
