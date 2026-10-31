// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! #1082: the files the operator's commands write in a workspace beside its four stores —
//! `workspace.md` (its `## Sessions` line, its vision, its sections), `workspace.json`, the piece
//! claim log and liveness records in `pieces/`, the work link log in `work/` — and the project
//! root's `sessions/` are written through the workspace's folder held by descriptor. A link a
//! chat leaves at one of them, or at the folder on the way, carries nothing into another
//! workspace or a persona, even when it stays inside the project.
//!
//! Each writer is run against `alpha` with a link planted, and nothing outside `alpha` may
//! change. A racer then swaps the files for links while two writers run.
#![cfg(unix)]

use std::path::{Path, PathBuf};

use purlis_core::active::Place;
use purlis_core::pieces;
use purlis_core::sessionrecord;
use purlis_core::work::TrackerKey;
use purlis_core::work::log as work_log;
use purlis_core::workspaces::{Plane, Workspace};

const DEVICE: &str = "01K6H0Z8Y3V1N3G4QK0A9T5B7C";
const THEIRS: &str = "theirs, never to be written\n";

fn when() -> chrono::DateTime<chrono::Utc> {
    use chrono::TimeZone;
    chrono::Utc.with_ymd_and_hms(2026, 10, 2, 8, 0, 0).unwrap()
}

/// A project with workspaces `alpha` and `beta` and a persona, `beta` and the persona holding
/// a file at every name the writers write.
fn project() -> tempfile::TempDir {
    let p = tempfile::tempdir().unwrap();
    for ws in ["alpha", "beta"] {
        let found = Plane::open(p.path()).workspace(ws).unwrap();
        std::fs::create_dir_all(found.dir()).unwrap();
        found.scaffold_charter().unwrap();
    }
    for target in ["workspaces/beta", "personas/devops"] {
        let dir = p.path().join(target);
        for sub in ["pieces/seen/svc", "work", "sessions"] {
            std::fs::create_dir_all(dir.join(sub)).unwrap();
        }
        for file in [
            "workspace.md",
            "workspace.json",
            "persona.md",
            "pieces/fixture.jsonl",
            "pieces/seen/svc/p1.json",
            &format!("work/{DEVICE}.jsonl"),
        ] {
            std::fs::write(dir.join(file), THEIRS).unwrap();
        }
    }
    p
}

fn alpha(p: &tempfile::TempDir) -> Workspace {
    Plane::open(p.path()).workspace("alpha").unwrap()
}

/// Every file and link under a folder, with its bytes or its target.
type Snapshot = Vec<(PathBuf, Vec<u8>)>;

/// Every file and link under `dir`, with its bytes or its target.
fn snapshot(dir: &Path) -> Snapshot {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return out;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let kind = std::fs::symlink_metadata(&path).unwrap().file_type();
        if kind.is_symlink() {
            let target = std::fs::read_link(&path).unwrap();
            out.push((path, target.to_string_lossy().as_bytes().to_vec()));
        } else if kind.is_dir() {
            out.push((path.clone(), Vec::new()));
            out.extend(snapshot(&path));
        } else {
            out.push((path.clone(), std::fs::read(&path).unwrap()));
        }
    }
    out.sort();
    out
}

/// What is outside `alpha`: `beta` and the persona store.
fn outside(p: &tempfile::TempDir) -> (Snapshot, Snapshot) {
    (
        snapshot(&p.path().join("workspaces/beta")),
        snapshot(&p.path().join("personas")),
    )
}

fn body() -> &'static str {
    "## Goal\n\nG.\n\n## Done\n\n- d\n\n## Decisions\n\n- x\n\n## Open\n\nNothing.\n\n## How to resume\n\nRead it.\n"
}

fn who() -> pieces::Who {
    pieces::Who {
        session: Some("s1".to_owned()),
        persona: None,
        chat: None,
        host: "fixture".to_owned(),
        log: "fixture".to_owned(),
    }
}

/// Every writer of a workspace file, run against `alpha`, with whether it answered Ok.
fn writers(p: &tempfile::TempDir) -> Vec<(&'static str, bool)> {
    let ws = alpha(p);
    let root = p.path();
    let issue = TrackerKey::parse("github:github.com/acme/api#12").unwrap();
    vec![
        ("scaffold_charter", ws.scaffold_charter().is_ok()),
        ("set_vision", ws.set_vision("Ours").is_ok()),
        (
            "add_to_section",
            ws.add_to_section("Glossary", "ours").is_ok(),
        ),
        (
            "seed_sections",
            ws.seed_sections(&[("Glossary", "ours")]).is_ok(),
        ),
        ("point", sessionrecord::point(root, "alpha").is_ok()),
        (
            "write_manifest",
            ws.write_manifest(&serde_json::json!({"name": "alpha"}))
                .is_ok(),
        ),
        (
            "write_manifest_text",
            ws.write_manifest_text("{\"name\": \"alpha\"}\n").is_ok(),
        ),
        (
            "pieces::record",
            pieces::record(
                root,
                "alpha",
                pieces::Event::Claimed,
                "svc",
                "p1",
                None,
                &who(),
                when(),
            )
            .is_some(),
        ),
        (
            "pieces::seen",
            pieces::seen(root, "alpha", "svc", Some("p1"), Some("s1"), None, when()).is_some(),
        ),
        (
            "work::log::append",
            work_log::append(root, "alpha", DEVICE, when(), &work_log::Op::link(issue)).is_ok(),
        ),
    ]
}

/// Plant `link` (relative to the project) pointing at `target`, run every writer, and say
/// nothing outside `alpha` changed.
fn nothing_outside(link: &str, target: &str, clear: bool) {
    let p = project();
    let at = p.path().join(link);
    if clear {
        let _ = std::fs::remove_dir_all(&at);
        let _ = std::fs::remove_file(&at);
    }
    std::os::unix::fs::symlink(target, &at).unwrap();
    let before = outside(&p);
    let answered = writers(&p);
    assert_eq!(
        outside(&p),
        before,
        "{link} -> {target}: something outside alpha changed; writers answered {answered:?}"
    );
}

#[test]
fn a_link_at_workspace_md_to_another_workspace_or_a_persona_carries_no_write() {
    purlis_core::unsteered!();
    for target in ["../beta/workspace.md", "../../personas/devops/persona.md"] {
        nothing_outside("workspaces/alpha/workspace.md", target, true);
    }
}

#[test]
fn a_link_at_workspace_json_to_another_workspace_or_a_persona_carries_no_write() {
    purlis_core::unsteered!();
    for target in [
        "../beta/workspace.json",
        "../../personas/devops/workspace.json",
    ] {
        nothing_outside("workspaces/alpha/workspace.json", target, true);
    }
}

#[test]
fn a_link_at_pieces_or_work_to_another_workspace_or_a_persona_carries_no_line() {
    purlis_core::unsteered!();
    for folder in ["pieces", "work"] {
        for target in [
            format!("../beta/{folder}"),
            format!("../../personas/devops/{folder}"),
        ] {
            nothing_outside(&format!("workspaces/alpha/{folder}"), &target, true);
        }
    }
    // A link at the log itself, inside a real `pieces/` and `work/`.
    let logs = [
        ("pieces/fixture.jsonl", "../../beta/pieces/fixture.jsonl"),
        ("pieces/seen", "../../beta/pieces/seen"),
        (
            &format!("work/{DEVICE}.jsonl") as &str,
            &format!("../../beta/work/{DEVICE}.jsonl") as &str,
        ),
    ];
    for (file, target) in logs {
        let p = project();
        let at = p.path().join("workspaces/alpha").join(file);
        std::fs::create_dir_all(at.parent().unwrap()).unwrap();
        std::os::unix::fs::symlink(target, &at).unwrap();
        let before = outside(&p);
        let answered = writers(&p);
        assert_eq!(outside(&p), before, "{file}: {answered:?}");
    }
}

#[test]
fn a_workspace_folder_that_is_a_link_to_another_workspace_or_a_persona_takes_no_write() {
    purlis_core::unsteered!();
    for target in ["beta", "../personas/devops"] {
        nothing_outside("workspaces/alpha", target, true);
    }
}

#[test]
fn the_project_roots_sessions_linked_to_a_workspaces_or_a_personas_takes_no_record() {
    purlis_core::unsteered!();
    for target in ["workspaces/beta/sessions", "personas/devops/sessions"] {
        let p = project();
        std::os::unix::fs::symlink(target, p.path().join("sessions")).unwrap();
        let before = outside(&p);
        let facts = sessionrecord::Facts {
            place: Place::PlaneRoot,
            at: when().naive_utc(),
            chat: None,
            persona: None,
            pieces: Vec::new(),
        };
        let recorded = sessionrecord::record(
            p.path(),
            &sessionrecord::New {
                title: "Ours",
                body: body(),
                facts: &facts,
            },
        );
        assert!(recorded.is_err(), "{target}: recorded through the link");
        assert_eq!(outside(&p), before, "{target}: something was written");
    }
}

/// Every writer still writes where nothing is linked: the hold changes where it refuses, not
/// what it writes.
#[test]
fn with_no_link_every_writer_writes_in_its_own_workspace() {
    purlis_core::unsteered!();
    let p = project();
    let before = outside(&p);
    let answered = writers(&p);
    assert!(answered.iter().all(|(_, ok)| *ok), "{answered:?}");
    assert_eq!(outside(&p), before);
    let alpha = p.path().join("workspaces/alpha");
    let md = std::fs::read_to_string(alpha.join("workspace.md")).unwrap();
    assert!(md.contains("Ours"), "{md}");
    assert!(md.contains("ours"), "{md}");
    assert!(
        std::fs::read_to_string(alpha.join("workspace.json"))
            .unwrap()
            .contains("alpha")
    );
    assert!(alpha.join("pieces/fixture.jsonl").is_file());
    assert!(alpha.join("pieces/seen/svc/p1.json").is_file());
    assert!(alpha.join(format!("work/{DEVICE}.jsonl")).is_file());
}

/// The race HP-7 measured, against two of these writers: a thread swaps `workspace.md` and
/// `pieces/` with links to another workspace's as fast as it can while the vision and the
/// claim log are written. Nothing outside `alpha` changes.
#[cfg(any(target_os = "linux", target_os = "macos"))]
#[test]
fn a_file_or_folder_swapped_for_a_link_while_a_writer_runs_never_carries_a_write_outside() {
    purlis_core::unsteered!();
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

    const CALLS: usize = 400;
    for (name, target) in [
        ("workspace.md", "../beta/workspace.md"),
        ("pieces", "../beta/pieces"),
    ] {
        let p = project();
        let ws = alpha(&p);
        let alpha_dir = ws.dir().to_path_buf();
        std::fs::create_dir_all(alpha_dir.join("pieces")).unwrap();
        std::os::unix::fs::symlink(target, alpha_dir.join("swap")).unwrap();
        let before = outside(&p);
        let stop = Arc::new(AtomicBool::new(false));
        let swaps = Arc::new(AtomicUsize::new(0));
        let racer = {
            let (stop, swaps) = (stop.clone(), swaps.clone());
            let (from, to) = (alpha_dir.join(name), alpha_dir.join("swap"));
            std::thread::spawn(move || {
                while !stop.load(Ordering::Relaxed) {
                    if rustix::fs::renameat_with(
                        rustix::fs::CWD,
                        &from,
                        rustix::fs::CWD,
                        &to,
                        rustix::fs::RenameFlags::EXCHANGE,
                    )
                    .is_ok()
                    {
                        swaps.fetch_add(1, Ordering::Relaxed);
                    }
                }
            })
        };
        for i in 0..CALLS {
            if name == "workspace.md" {
                let _ = ws.set_vision(&format!("Raced {i}"));
            } else {
                let _ = pieces::record(
                    p.path(),
                    "alpha",
                    pieces::Event::Claimed,
                    "svc",
                    &format!("p{i}"),
                    None,
                    &who(),
                    when(),
                );
            }
        }
        stop.store(true, Ordering::Relaxed);
        racer.join().unwrap();
        assert!(swaps.load(Ordering::Relaxed) > 0, "the racer never swapped");
        assert!(
            outside(&p) == before,
            "{name}: something outside alpha changed"
        );
    }
}
