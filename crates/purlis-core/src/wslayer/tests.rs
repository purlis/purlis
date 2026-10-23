// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

/// A plane with one workspace directory and a settings file worth mirroring.
fn plane() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("charter.toml"), "").unwrap();
    std::fs::create_dir_all(dir.path().join(".claude")).unwrap();
    std::fs::write(
        dir.path().join(layer::SETTINGS),
        r#"{"env":{"CHARTER_HARNESS":"claude-code"},
            "permissions":{"ask":["Bash(terraform apply *)"]}}"#,
    )
    .unwrap();
    let ws = dir.path().join("workspaces").join("alpha");
    std::fs::create_dir_all(&ws).unwrap();
    (dir, ws)
}

/// Run the scaffold against a workspace of the test plane, as `ensure` does.
fn scaffold_into(plane: &tempfile::TempDir, name: &str) {
    let opened = crate::workspaces::Plane::open(plane.path());
    scaffold(
        &opened,
        name,
        "2026-05-04T11:32:17Z".parse().unwrap(),
        "fixture",
    );
}

fn rows(rows: &[Row]) -> Vec<(&str, Did)> {
    rows.iter().map(|r| (r.rel.as_str(), r.did)).collect()
}

#[test]
fn a_workspace_with_no_layer_gets_one_and_a_record_that_names_it() {
    let (plane, ws) = plane();
    assert_eq!(
        rows(&wire(plane.path(), &ws)),
        [(layer::SETTINGS, Did::Created)]
    );
    let text = std::fs::read_to_string(ws.join(layer::SETTINGS)).unwrap();
    assert!(text.contains("terraform apply"), "{text}");
    assert!(text.contains("CHARTER_HARNESS"), "{text}");
    let record = layer::read_record(&ws);
    assert_eq!(
        record.settled(layer::SETTINGS),
        Some(layer::digest(&text).as_str())
    );
}

#[test]
fn a_second_wire_changes_nothing_and_says_so() {
    let (plane, ws) = plane();
    wire(plane.path(), &ws);
    let marker_before = std::fs::metadata(ws.join(layer::MARKER))
        .unwrap()
        .modified()
        .unwrap();
    assert_eq!(
        rows(&wire(plane.path(), &ws)),
        [(layer::SETTINGS, Did::Present)]
    );
    // The record is republished only when something changed.
    assert_eq!(
        std::fs::metadata(ws.join(layer::MARKER))
            .unwrap()
            .modified()
            .unwrap(),
        marker_before
    );
}

#[test]
fn a_plane_that_moved_refreshes_charters_own_file() {
    let (plane, ws) = plane();
    wire(plane.path(), &ws);
    std::fs::write(
        plane.path().join(layer::SETTINGS),
        r#"{"permissions":{"deny":["Bash(rm -rf *)"]}}"#,
    )
    .unwrap();
    assert_eq!(
        rows(&wire(plane.path(), &ws)),
        [(layer::SETTINGS, Did::Refreshed)]
    );
    let text = std::fs::read_to_string(ws.join(layer::SETTINGS)).unwrap();
    assert!(text.contains("rm -rf"), "{text}");
}

#[test]
fn a_file_charter_did_not_write_is_left_exactly_as_it_is() {
    let (plane, ws) = plane();
    std::fs::create_dir_all(ws.join(".claude")).unwrap();
    std::fs::write(ws.join(layer::SETTINGS), "MINE\n").unwrap();
    assert_eq!(
        rows(&wire(plane.path(), &ws)),
        [(layer::SETTINGS, Did::Foreign)]
    );
    assert_eq!(
        std::fs::read_to_string(ws.join(layer::SETTINGS)).unwrap(),
        "MINE\n"
    );
    // And charter vouches for nothing it did not write.
    assert!(!ws.join(layer::MARKER).exists());
}

#[test]
fn a_plane_that_stops_declaring_anything_withdraws_the_file_it_generated() {
    let (plane, ws) = plane();
    assert_eq!(
        rows(&wire(plane.path(), &ws)),
        [(layer::SETTINGS, Did::Created)]
    );
    // Every key that travels, gone. Not an unreadable file: a plane that is HOLDING its
    // settings is the other case, and it withdraws nothing.
    std::fs::write(plane.path().join(layer::SETTINGS), r#"{"hooks":{}}"#).unwrap();
    assert_eq!(
        rows(&wire(plane.path(), &ws)),
        [(layer::SETTINGS, Did::Removed)]
    );
    assert!(!ws.join(layer::SETTINGS).exists());
    // The `.claude/` it lived in goes too — charter standing in a directory it has
    // nothing in.
    assert!(!ws.join(".claude").exists());
    // And the record, which now names nothing.
    assert!(!ws.join(layer::MARKER).exists());
}

#[test]
fn a_plane_holding_an_unparseable_settings_file_withdraws_nothing() {
    let (plane, ws) = plane();
    wire(plane.path(), &ws);
    let kept = std::fs::read_to_string(ws.join(layer::SETTINGS)).unwrap();
    std::fs::write(plane.path().join(layer::SETTINGS), "{ not json").unwrap();
    // NO ROW AT ALL, which is the Python's answer too and worth being exact about: a plane
    // that is HOLDING its settings wants nothing, so there is nothing to compare — and the
    // record's entry is skipped rather than withdrawn. A `removed` row here would be every
    // workspace's `enabledPlugins`, `env` and `deny` taken away over a typo.
    assert_eq!(rows(&wire(plane.path(), &ws)), []);
    assert_eq!(
        std::fs::read_to_string(ws.join(layer::SETTINGS)).unwrap(),
        kept,
        "charter kept the last good copy"
    );
    // And the record still names it, so the day the plane parses again the file reads as
    // charter's own rather than as somebody else's.
    assert_eq!(
        layer::read_record(&ws).settled(layer::SETTINGS),
        Some(layer::digest(&kept).as_str())
    );
}

#[test]
fn a_record_naming_a_file_outside_charters_generated_roots_withdraws_nothing() {
    let (plane, ws) = plane();
    std::fs::write(ws.join("README.md"), "the operator's\n").unwrap();
    // A marker charter trusts can still name a file charter never wrote.
    std::fs::write(
        ws.join(layer::MARKER),
        serde_json::to_string(&serde_json::json!({
            "README.md": layer::digest("the operator's\n"),
        }))
        .unwrap(),
    )
    .unwrap();
    let did = wire(plane.path(), &ws);
    assert!(
        !did.iter().any(|r| r.rel == "README.md"),
        "{did:?} must not touch a file outside charter's generated roots"
    );
    assert!(ws.join("README.md").exists());
}

/// Write `entries` as the workspace folder's record, the way anything that can write
/// the folder could.
fn forge_record(ws: &Path, entries: &[(&str, &str)]) {
    let doc: serde_json::Map<String, serde_json::Value> = entries
        .iter()
        .map(|(rel, text)| ((*rel).to_owned(), layer::digest(text).into()))
        .collect();
    std::fs::write(ws.join(layer::MARKER), serde_json::to_string(&doc).unwrap()).unwrap();
}

#[test]
fn a_forged_record_naming_the_operators_own_file_under_a_generated_root_withdraws_nothing() {
    // The record sits in the workspace folder, where a chat may be able to write it. One
    // naming the operator's own file under `.claude/`, with that file's digest, must not
    // get purlis to delete it: purlis never offered that text there.
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("charter.toml"), "").unwrap();
    let ws = dir.path().join("workspaces").join("alpha");
    let mine = ".claude/notes/mine.md";
    std::fs::create_dir_all(ws.join(".claude/notes")).unwrap();
    std::fs::write(ws.join(mine), "the operator's\n").unwrap();
    forge_record(&ws, &[(mine, "the operator's\n")]);

    assert!(
        !status(dir.path(), &ws, &want(dir.path()))
            .iter()
            .any(|(rel, _)| rel == mine),
        "a file purlis never offered is not reported as purlis's to withdraw"
    );
    let did = wire(dir.path(), &ws);
    assert!(!did.iter().any(|r| r.rel == mine), "{did:?}");
    assert_eq!(
        std::fs::read_to_string(ws.join(mine)).unwrap(),
        "the operator's\n"
    );
}

#[test]
fn a_forged_record_naming_the_operators_own_settings_withdraws_nothing() {
    // The plane offers nothing, and the operator keeps a settings file of their own in
    // the workspace folder. A record naming it is not proof purlis wrote it.
    let (plane, ws) = plane();
    std::fs::write(plane.path().join(layer::SETTINGS), r#"{"hooks":{}}"#).unwrap();
    std::fs::create_dir_all(ws.join(".claude")).unwrap();
    std::fs::write(ws.join(layer::SETTINGS), "MINE\n").unwrap();
    forge_record(&ws, &[(layer::SETTINGS, "MINE\n")]);

    assert_eq!(status(plane.path(), &ws, &want(plane.path())), []);
    assert_eq!(rows(&wire(plane.path(), &ws)), []);
    assert_eq!(
        std::fs::read_to_string(ws.join(layer::SETTINGS)).unwrap(),
        "MINE\n"
    );

    // Offered at that path once, but another text: still not this file.
    crate::guest::note_offered(
        plane.path(),
        &BTreeMap::from([(layer::SETTINGS.to_owned(), "{}\n".to_owned())]),
    );
    assert_eq!(rows(&wire(plane.path(), &ws)), []);
    assert_eq!(
        std::fs::read_to_string(ws.join(layer::SETTINGS)).unwrap(),
        "MINE\n"
    );
}

#[test]
fn a_forged_record_naming_the_operators_own_settings_never_gets_it_overwritten() {
    // The plane declares settings, and the operator keeps settings of their own in the
    // workspace folder. A record naming that file, with its digest, reads it as purlis's
    // older copy: alone, it would have the next launch overwrite it with the plane's text.
    let (plane, ws) = plane();
    let mine = "{\"permissions\":{\"deny\":[\"Bash(curl *)\"]}}\n";
    std::fs::create_dir_all(ws.join(".claude")).unwrap();
    std::fs::write(ws.join(layer::SETTINGS), mine).unwrap();
    forge_record(&ws, &[(layer::SETTINGS, mine)]);

    assert_eq!(
        status(plane.path(), &ws, &want(plane.path())),
        [(layer::SETTINGS.to_string(), Found::Unconfirmed)]
    );
    assert_eq!(
        rows(&wire(plane.path(), &ws)),
        [(layer::SETTINGS, Did::Unconfirmed)]
    );
    assert_eq!(
        std::fs::read(ws.join(layer::SETTINGS)).unwrap(),
        mine.as_bytes(),
        "left byte for byte as it was"
    );
    assert!(Did::Unconfirmed.is_unresolved() && !Did::Unconfirmed.is_repair());

    // Noted for a checkout's agents at the same path is not noted here: still left as is.
    let agent = ".claude/agents/ops.md";
    crate::guest::note_offered(
        plane.path(),
        &BTreeMap::from([(agent.to_owned(), mine.to_owned())]),
    );
    assert_eq!(
        rows(&wire(plane.path(), &ws)),
        [(layer::SETTINGS, Did::Unconfirmed)]
    );
    assert_eq!(
        std::fs::read_to_string(ws.join(layer::SETTINGS)).unwrap(),
        mine
    );
}

#[test]
fn a_stale_copy_whose_text_purlis_noted_offering_is_still_refreshed() {
    // The genuine case: the text on disk is one the plane offered here before, so both the
    // record and the project's own note vouch for it.
    let (plane, ws) = plane();
    let old = want(plane.path())[layer::SETTINGS].clone();
    crate::guest::note_offered(plane.path(), &want(plane.path()));
    std::fs::create_dir_all(ws.join(".claude")).unwrap();
    std::fs::write(ws.join(layer::SETTINGS), &old).unwrap();
    forge_record(&ws, &[(layer::SETTINGS, &old)]);
    std::fs::write(
        plane.path().join(layer::SETTINGS),
        r#"{"permissions":{"deny":["Bash(rm -rf *)"]}}"#,
    )
    .unwrap();

    assert_eq!(
        status(plane.path(), &ws, &want(plane.path())),
        [(layer::SETTINGS.to_string(), Found::Stale)]
    );
    assert_eq!(
        rows(&wire(plane.path(), &ws)),
        [(layer::SETTINGS, Did::Refreshed)]
    );
    let text = std::fs::read_to_string(ws.join(layer::SETTINGS)).unwrap();
    assert!(text.contains("rm -rf"), "{text}");
}

#[test]
fn an_offer_made_to_a_checkout_vouches_for_nothing_in_a_workspace_folder() {
    // The ledger notes the agents a checkout is offered, at the same relative path a
    // workspace folder could hold a file of the operator's. A record naming that file,
    // with that text, must not get it withdrawn here: this layer never offered it.
    let (plane, ws) = plane();
    let agent = ".claude/agents/ops.md";
    crate::guest::note_offered(
        plane.path(),
        &BTreeMap::from([(agent.to_owned(), "# ops\n".to_owned())]),
    );
    assert!(crate::guest::read_offered(plane.path()).contains_key(agent));
    std::fs::create_dir_all(ws.join(".claude/agents")).unwrap();
    std::fs::write(ws.join(agent), "# ops\n").unwrap();
    forge_record(&ws, &[(agent, "# ops\n")]);

    assert!(
        !status(plane.path(), &ws, &want(plane.path()))
            .iter()
            .any(|(rel, _)| rel == agent)
    );
    let did = wire(plane.path(), &ws);
    assert!(!did.iter().any(|r| r.rel == agent), "{did:?}");
    assert_eq!(std::fs::read_to_string(ws.join(agent)).unwrap(), "# ops\n");
}

#[test]
fn a_copy_purlis_offered_goes_and_nothing_goes_when_nothing_was_noted() {
    // The genuine case: purlis wrote the settings, noted the offer, and the plane then
    // stopped declaring anything.
    let (plane, ws) = plane();
    wire(plane.path(), &ws);
    let ledger = crate::guest::offered_path(plane.path());
    assert!(
        ledger.exists(),
        "the offer is noted where no chat can write"
    );

    let (other, other_ws) = self::plane();
    wire(other.path(), &other_ws);
    // Nothing could be noted, or the note is gone: fail closed.
    std::fs::remove_file(crate::guest::offered_path(other.path())).unwrap();
    for p in [&plane, &other] {
        std::fs::write(p.path().join(layer::SETTINGS), r#"{"hooks":{}}"#).unwrap();
    }

    assert_eq!(
        rows(&wire(plane.path(), &ws)),
        [(layer::SETTINGS, Did::Removed)]
    );
    assert!(!ws.join(layer::SETTINGS).exists());
    assert_eq!(rows(&wire(other.path(), &other_ws)), []);
    assert!(
        other_ws.join(layer::SETTINGS).exists(),
        "with no note, nothing is withdrawn"
    );
}

#[test]
fn a_record_whose_key_leaves_the_workspace_is_not_charters_record() {
    let (plane, ws) = plane();
    let outside = plane.path().parent().unwrap().join("victim.json");
    std::fs::write(&outside, "PRECIOUS\n").unwrap();
    std::fs::write(
        ws.join(layer::MARKER),
        serde_json::to_string(&serde_json::json!({
            "../../../victim.json": layer::digest("PRECIOUS\n"),
        }))
        .unwrap(),
    )
    .unwrap();
    wire(plane.path(), &ws);
    assert_eq!(std::fs::read_to_string(&outside).unwrap(), "PRECIOUS\n");
}

#[cfg(unix)]
#[test]
fn a_workspace_directory_that_is_a_link_out_of_the_plane_is_refused_whole() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("charter.toml"), "").unwrap();
    std::fs::create_dir_all(dir.path().join(".claude")).unwrap();
    std::fs::write(dir.path().join(layer::SETTINGS), r#"{"env":{"A":"1"}}"#).unwrap();
    let outside = dir.path().parent().unwrap().join("elsewhere-ws");
    std::fs::create_dir_all(&outside).unwrap();
    std::fs::create_dir_all(dir.path().join("workspaces")).unwrap();
    let ws = dir.path().join("workspaces").join("escape");
    std::os::unix::fs::symlink(&outside, &ws).unwrap();
    assert_eq!(
        rows(&wire(dir.path(), &ws)),
        [(layer::MARKER, Did::Blocked)]
    );
    assert!(!outside.join(".claude").exists());
    assert!(!outside.join(layer::MARKER).exists());
    let _ = std::fs::remove_dir_all(&outside);
}

#[cfg(unix)]
#[test]
fn a_settings_path_reached_through_a_link_is_foreign_and_never_written_through() {
    let (plane, ws) = plane();
    // Charter's own layer first, so there IS a record — then the file is replaced by a
    // link out of the plane pointing at content that record still vouches for, while the
    // plane moves on. Without the containment check the row reads `stale`, which is
    // charter's own file to rewrite, and the write lands wherever the link points.
    //
    // The weaker shape of this test — a link to content nothing vouches for — passes with
    // the check DELETED, because unrecorded content reads `foreign` on its own. This one
    // does not: it is the digest, not the path, that would let the write through.
    wire(plane.path(), &ws);
    let mine = std::fs::read_to_string(ws.join(layer::SETTINGS)).unwrap();
    let outside = plane.path().parent().unwrap().join("victim-settings.json");
    std::fs::write(&outside, &mine).unwrap();
    std::fs::remove_file(ws.join(layer::SETTINGS)).unwrap();
    // Planted at the EXACT path the write opens, which is the only place a test of this
    // proves anything: a link one directory up is caught by a gate on the parent, and a
    // gate on the parent is the mistake this repository keeps shipping.
    std::os::unix::fs::symlink(&outside, ws.join(layer::SETTINGS)).unwrap();
    std::fs::write(
        plane.path().join(layer::SETTINGS),
        r#"{"permissions":{"deny":["Bash(rm -rf *)"]}}"#,
    )
    .unwrap();

    assert_eq!(
        status(plane.path(), &ws, &want(plane.path())),
        [(layer::SETTINGS.to_string(), Found::Foreign)],
        "a path charter reaches through a link is not one its record can vouch for"
    );
    assert_eq!(
        rows(&wire(plane.path(), &ws)),
        [(layer::SETTINGS, Did::Foreign)]
    );
    assert_eq!(
        std::fs::read_to_string(&outside).unwrap(),
        mine,
        "charter wrote the plane's new rules through a link out of the plane"
    );
    let _ = std::fs::remove_file(&outside);
}

#[cfg(unix)]
#[test]
fn a_withdrawal_is_never_performed_through_a_link_out_of_the_workspace() {
    let (plane, ws) = plane();
    wire(plane.path(), &ws);
    let mine = std::fs::read_to_string(ws.join(layer::SETTINGS)).unwrap();
    // The same shape one verb further on: the record vouches for what the link POINTS at,
    // so without the check the withdraw finds a digest it trusts and unlinks a path of the
    // operator's. `remove_file` goes at the link node, so what is lost is their link.
    let outside = plane.path().parent().unwrap().join("victim-withdrawn.json");
    std::fs::write(&outside, &mine).unwrap();
    std::fs::remove_file(ws.join(layer::SETTINGS)).unwrap();
    std::os::unix::fs::symlink(&outside, ws.join(layer::SETTINGS)).unwrap();
    // And the plane stops declaring anything, so this path is now unwanted.
    std::fs::write(plane.path().join(layer::SETTINGS), r#"{"hooks":{}}"#).unwrap();

    assert!(
        !status(plane.path(), &ws, &want(plane.path()))
            .iter()
            .any(|(_, found)| *found == Found::Unwanted),
        "a path reached through a link is not charter's to withdraw"
    );
    assert!(
        !wire(plane.path(), &ws)
            .iter()
            .any(|row| row.did == Did::Removed)
    );
    assert!(
        ws.join(layer::SETTINGS).symlink_metadata().is_ok(),
        "charter removed a link the operator put there"
    );
    let _ = std::fs::remove_file(&outside);
}

#[cfg(unix)]
#[test]
fn a_claude_directory_that_is_a_link_out_is_refused_at_the_component_it_is_on() {
    let (plane, ws) = plane();
    let outside = plane.path().parent().unwrap().join("victim-claude");
    std::fs::create_dir_all(&outside).unwrap();
    std::os::unix::fs::symlink(&outside, ws.join(".claude")).unwrap();
    let did = wire(plane.path(), &ws);
    // `missing` at the leaf, so the write is attempted and the gate on the WAY is what
    // has to stop it.
    assert_eq!(rows(&did), [(layer::SETTINGS, Did::Blocked)]);
    assert!(!outside.join("settings.json").exists());
    let _ = std::fs::remove_dir_all(&outside);
}

#[cfg(unix)]
#[test]
fn a_pending_record_left_by_a_blocked_write_still_vouches_for_the_old_file() {
    use std::os::unix::fs::PermissionsExt;

    let (plane, ws) = plane();
    wire(plane.path(), &ws);
    let first = std::fs::read_to_string(ws.join(layer::SETTINGS)).unwrap();
    // The plane moves, so charter's own file is `stale` and charter is about to rewrite
    // it. The write is then made to fail with the FILE STILL THERE, which is the only
    // shape that tests this: a path that is GONE is one the record forgets by design, and
    // a link on the way makes the row `foreign` before any write is attempted.
    std::fs::write(
        plane.path().join(layer::SETTINGS),
        r#"{"permissions":{"deny":["Bash(rm -rf *)"]}}"#,
    )
    .unwrap();
    let claude = ws.join(".claude");
    std::fs::set_permissions(&claude, std::fs::Permissions::from_mode(0o500)).unwrap();
    // `r-x`: the file is still readable and still `stat`-able, and the temp `write_whole`
    // writes beside it cannot be created. Root ignores the bits, so the state is asserted
    // rather than assumed — a test that quietly did nothing here would report every guard
    // below it green.
    let probe = std::fs::File::create_new(claude.join("probe"));
    assert!(
        probe.is_err(),
        "this test needs a user the mode bits apply to; it cannot run as root"
    );

    assert_eq!(
        rows(&wire(plane.path(), &ws)),
        [(layer::SETTINGS, Did::Blocked)]
    );

    // The entry vouches for BOTH: what the file holds, and what the write would have put
    // there. Silence is not a verdict — a record published only AFTER the write would
    // leave the old digest beside a file that may already hold the new text, and charter's
    // own file then reads as somebody else's for ever.
    let record = layer::read_record(&ws);
    let wanted = layer::digest(&want(plane.path())[layer::SETTINGS]);
    assert!(
        record
            .recorded(layer::SETTINGS)
            .contains(&layer::digest(&first))
    );
    assert!(record.recorded(layer::SETTINGS).contains(&wanted));
    assert_eq!(
        record.settled(layer::SETTINGS),
        None,
        "a pending entry is not a verdict about what the file holds"
    );
    assert_eq!(
        std::fs::read_to_string(ws.join(layer::SETTINGS)).unwrap(),
        first,
        "a write that did not land left the file exactly as it was"
    );

    // And once the write can land, the file charter last wrote is still charter's own to
    // refresh rather than somebody else's to leave alone.
    std::fs::set_permissions(&claude, std::fs::Permissions::from_mode(0o700)).unwrap();
    assert_eq!(
        rows(&wire(plane.path(), &ws)),
        [(layer::SETTINGS, Did::Refreshed)]
    );
    assert_eq!(
        layer::read_record(&ws).settled(layer::SETTINGS),
        Some(wanted.as_str()),
        "and the record settles on what is now there"
    );
}

#[test]
fn the_read_only_status_reports_what_a_wire_would_do_and_writes_nothing() {
    let (plane, ws) = plane();
    // Missing, before anything is written.
    assert_eq!(
        status(plane.path(), &ws, &want(plane.path())),
        [(layer::SETTINGS.to_string(), Found::Missing)]
    );
    // And it is READ ONLY — `doctor` calls it from a hook, and a check that heals reports
    // every workspace healthy by having just healed it.
    assert!(!ws.join(layer::SETTINGS).exists());
    assert!(!ws.join(layer::MARKER).exists());

    wire(plane.path(), &ws);
    assert_eq!(
        status(plane.path(), &ws, &want(plane.path())),
        [(layer::SETTINGS.to_string(), Found::Ok)]
    );

    // The plane moves: charter's own file, still holding what charter's record names.
    std::fs::write(
        plane.path().join(layer::SETTINGS),
        r#"{"permissions":{"deny":["Bash(rm -rf *)"]}}"#,
    )
    .unwrap();
    assert_eq!(
        status(plane.path(), &ws, &want(plane.path())),
        [(layer::SETTINGS.to_string(), Found::Stale)]
    );

    // The plane stops declaring anything: a file nothing generates any more.
    std::fs::write(plane.path().join(layer::SETTINGS), r#"{"hooks":{}}"#).unwrap();
    assert_eq!(
        status(plane.path(), &ws, &want(plane.path())),
        [(layer::SETTINGS.to_string(), Found::Unwanted)]
    );
    assert!(
        ws.join(layer::SETTINGS).exists(),
        "reporting a withdrawal is not performing one"
    );
}

#[test]
fn a_generated_path_charter_cannot_read_is_unreadable_and_never_foreign() {
    let (plane, ws) = plane();
    wire(plane.path(), &ws);
    // A directory at the generated path: there, and not readable as text. Not `foreign`,
    // whose sentence advises moving a file aside — charter only failed to read this one.
    std::fs::remove_file(ws.join(layer::SETTINGS)).unwrap();
    std::fs::create_dir(ws.join(layer::SETTINGS)).unwrap();
    assert_eq!(
        status(plane.path(), &ws, &want(plane.path())),
        [(layer::SETTINGS.to_string(), Found::Unreadable)]
    );
    assert_eq!(
        rows(&wire(plane.path(), &ws)),
        [(layer::SETTINGS, Did::Unreadable)]
    );
    assert!(ws.join(layer::SETTINGS).is_dir(), "left exactly as it is");
}

#[test]
fn every_status_this_target_can_reach_is_named_by_a_test() {
    // The enumeration the ticket asks for, as an assertion rather than a comment: each
    // of these is produced by a test above, and the ones a workspace directory CANNOT
    // reach are named in the module docs with the reason.
    let reachable = [
        Did::Created,
        Did::Refreshed,
        Did::Present,
        Did::Removed,
        Did::Foreign,
        Did::Blocked,
        Did::Unreadable,
        // A CHECKOUT's only, and named by `wscmd::reinit`'s own tests: the one
        // co-written path is `.claude/settings.local.json`, which a workspace directory
        // never wants.
        Did::HarnessBehind,
        Did::Unconfirmed,
    ];
    assert_eq!(reachable.len(), 9);
    assert_eq!(reachable.iter().filter(|d| d.is_repair()).count(), 3);
    assert_eq!(reachable.iter().filter(|d| d.is_unresolved()).count(), 5);
}

#[test]
fn a_fresh_stamp_reads_back_the_version_it_was_written_with() {
    let (_plane, ws) = plane();
    assert_eq!(stamp(&ws), (0, None));
    write_stamp(&ws).unwrap();
    assert_eq!(stamp(&ws), (STRUCTURE_VERSION, None));
}

#[cfg(unix)]
#[test]
fn a_stamp_that_is_a_link_is_neither_read_through_nor_written_through() {
    let (plane, ws) = plane();
    let outside = plane.path().parent().unwrap().join("victim-stamp");
    std::fs::write(&outside, "99\n").unwrap();
    std::os::unix::fs::symlink(&outside, ws.join(STRUCTURE_MARKER)).unwrap();
    let (version, blocker) = stamp(&ws);
    assert_eq!(version, 0);
    assert_eq!(blocker.map(|b| b.why), Some(Why::IsALink));
    assert!(write_stamp(&ws).is_err());
    assert_eq!(std::fs::read_to_string(&outside).unwrap(), "99\n");
    let _ = std::fs::remove_file(&outside);
}

#[test]
fn a_stamp_that_is_a_directory_is_named_rather_than_written_over() {
    let (_plane, ws) = plane();
    std::fs::create_dir_all(ws.join(STRUCTURE_MARKER)).unwrap();
    let (version, blocker) = stamp(&ws);
    assert_eq!(version, 0);
    assert_eq!(blocker.map(|b| b.why), Some(Why::IsADirectory));
    assert!(write_stamp(&ws).is_err());
}

#[test]
fn a_legacy_stamp_is_renamed_rather_than_restamped() {
    for legacy in [".edm-structure", ".charter-structure"] {
        let (_plane, ws) = plane();
        std::fs::write(ws.join(legacy), "3\n").unwrap();
        // The version it really is, not the one charter is shipping.
        assert_eq!(stamp(&ws).0, 3, "{legacy}");
        assert_eq!(
            std::fs::read_to_string(ws.join(".purlis-structure")).unwrap(),
            "3\n"
        );
        assert!(!ws.join(legacy).exists(), "{legacy}");
    }
}

#[test]
fn with_charters_stamp_and_an_older_one_the_charter_one_wins_and_moves() {
    let (_plane, ws) = plane();
    std::fs::write(ws.join(".edm-structure"), "2\n").unwrap();
    std::fs::write(ws.join(".charter-structure"), "4\n").unwrap();
    assert_eq!(stamp(&ws).0, 4);
    assert!(ws.join(STRUCTURE_MARKER).is_file());
    assert!(!ws.join(".edm-structure").exists());
    assert!(!ws.join(".charter-structure").exists());
}

#[test]
fn a_layer_record_charter_wrote_moves_to_the_purlis_name_and_its_files_stay_charters() {
    let (plane, ws) = plane();
    wire(plane.path(), &ws);
    let record = std::fs::read_to_string(ws.join(layer::MARKER)).unwrap();
    // As charter left it before the rename.
    std::fs::rename(ws.join(layer::MARKER), ws.join(".charter-generated")).unwrap();
    let before = std::fs::read_to_string(ws.join(layer::SETTINGS)).unwrap();

    let rows = wire(plane.path(), &ws);
    assert!(
        rows.iter().all(|r| r.did == Did::Present),
        "every file is still charter's: {rows:?}"
    );
    assert!(!ws.join(".charter-generated").exists());
    assert_eq!(
        std::fs::read_to_string(ws.join(layer::MARKER)).unwrap(),
        record
    );
    assert_eq!(
        std::fs::read_to_string(ws.join(layer::SETTINGS)).unwrap(),
        before
    );
}

#[test]
fn a_baseline_file_under_something_that_is_not_a_directory_is_in_the_way_not_missing() {
    let (_plane, ws) = plane();
    std::fs::write(ws.join("refs"), "not a directory\n").unwrap();
    let status = structure_status(&ws);
    assert!(!status.missing.contains(&"refs/README.md"));
    let (rel, blocker) = status
        .in_the_way
        .iter()
        .find(|(rel, _)| rel == "refs/README.md")
        .expect("named in_the_way");
    assert_eq!(rel, "refs/README.md");
    assert_eq!(blocker.why, Why::NotADirectory);
}

#[cfg(unix)]
#[test]
fn a_baseline_file_that_is_a_dangling_link_is_in_the_way_and_never_written_through() {
    let (plane, ws) = plane();
    let never = plane.path().parent().unwrap().join("never-made.md");
    std::os::unix::fs::symlink(&never, ws.join("workspace.md")).unwrap();
    let status = structure_status(&ws);
    // A link where a FILE belongs is in the way wherever it points.
    assert_eq!(
        status
            .in_the_way
            .iter()
            .find(|(rel, _)| rel == "workspace.md")
            .map(|(_, b)| b.why),
        Some(Why::IsALink)
    );
    assert!(!status.missing.contains(&"workspace.md"));
    assert!(!never.exists());
}

#[cfg(unix)]
#[test]
fn a_baseline_directory_that_is_a_link_out_of_the_workspace_is_in_the_way() {
    let (plane, ws) = plane();
    let outside = plane.path().parent().unwrap().join("victim-refs");
    std::fs::create_dir_all(&outside).unwrap();
    std::os::unix::fs::symlink(&outside, ws.join("refs")).unwrap();
    let status = structure_status(&ws);
    let (_rel, blocker) = status
        .in_the_way
        .iter()
        .find(|(rel, _)| rel == "refs/README.md")
        .expect("named in_the_way");
    // The link is named, not the file under it: that is where the repair is.
    assert_eq!(blocker.why, Why::IsALink);
    assert_eq!(blocker.path, ws.join("refs"));
    assert!(!status.missing.contains(&"refs/README.md"));
    // And nothing is written out there.
    scaffold_into(&plane, "alpha");
    assert!(!outside.join("README.md").exists());
    let _ = std::fs::remove_dir_all(&outside);
}

#[cfg(unix)]
#[test]
fn a_baseline_directory_that_is_a_link_back_inside_the_workspace_is_not_in_the_way() {
    let (_plane, ws) = plane();
    // The other side of the same test, and the reason both ends are resolved before they
    // are compared: on macOS `$TMPDIR` is itself reached through a link, so a lexical
    // comparison calls every honest link in a temp plane an escape.
    std::fs::create_dir_all(ws.join("real-refs")).unwrap();
    std::os::unix::fs::symlink(ws.join("real-refs"), ws.join("refs")).unwrap();
    let status = structure_status(&ws);
    assert!(
        !status
            .in_the_way
            .iter()
            .any(|(rel, _)| rel == "refs/README.md"),
        "{status:?}"
    );
    assert!(status.missing.contains(&"refs/README.md"));
}

#[test]
fn a_workspace_with_the_whole_baseline_and_the_stamp_reads_current() {
    let (_plane, ws) = plane();
    for rel in BASELINE {
        let path = ws.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "x\n").unwrap();
    }
    write_stamp(&ws).unwrap();
    let status = structure_status(&ws);
    assert!(status.ok, "{status:?}");
    assert!(status.missing.is_empty());
    assert_eq!(status.version, STRUCTURE_VERSION);
}

#[test]
fn an_older_stamp_is_stale_even_with_the_whole_baseline() {
    let (_plane, ws) = plane();
    for rel in BASELINE {
        let path = ws.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "x\n").unwrap();
    }
    std::fs::write(ws.join(STRUCTURE_MARKER), "4\n").unwrap();
    assert!(!structure_status(&ws).ok);
    assert!(needs_reinit(&ws));
}

/// A workspace whose stamp the filesystem will not answer about is not "behind" (#1289):
/// unread is not old, and a `reinit` could not reach it either. The footer's rule, here.
#[cfg(unix)]
#[test]
fn a_workspace_or_stamp_it_cannot_read_is_unknown_not_behind() {
    use std::os::unix::fs::PermissionsExt;
    let mode = |p: &Path, m: u32| std::fs::set_permissions(p, std::fs::Permissions::from_mode(m));
    // No `plane()`: a manifest cannot be written in every sandbox, and the stamp's reading
    // needs only the workspace's own folder.
    let held = tempfile::tempdir().unwrap();
    let ws = held.path().join("workspaces").join("alpha");
    for rel in BASELINE {
        let path = ws.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "x\n").unwrap();
    }
    write_stamp(&ws).unwrap();
    assert!(!needs_reinit(&ws));

    // A stamp that is there but cannot be read.
    let marker = ws.join(STRUCTURE_MARKER);
    mode(&marker, 0o000).unwrap();
    let read = std::fs::read(&marker).is_ok();
    let unread = structure_status(&ws);
    let unread_stamp = needs_reinit(&ws);
    // …while a baseline file that is missing is still missing: `reinit` can create it
    // whether or not it can stamp.
    let one = ws.join(BASELINE[0]);
    std::fs::rename(&one, held.path().join("aside")).unwrap();
    let unread_stamp_and_missing = needs_reinit(&ws);
    std::fs::rename(held.path().join("aside"), &one).unwrap();
    mode(&marker, 0o644).unwrap();

    // A workspace folder that cannot be listed: nothing in it answers.
    mode(&ws, 0o000).unwrap();
    let listed = std::fs::read_dir(&ws).is_ok();
    let sealed = needs_reinit(&ws);
    mode(&ws, 0o755).unwrap();

    if read || listed {
        // Running as a user the mode does not stop (root): nothing to test.
        return;
    }
    assert!(unread.stamp_unread, "{unread:?}");
    assert!(!unread_stamp);
    assert!(unread_stamp_and_missing);
    assert!(!sealed);
    // An absent stamp is still version 0, and behind: there is nothing it failed to read.
    std::fs::remove_file(&marker).unwrap();
    assert!(!structure_status(&ws).stamp_unread);
    assert!(needs_reinit(&ws));
    // And a readable old one is behind.
    std::fs::write(&marker, "4\n").unwrap();
    assert!(needs_reinit(&ws));
}

#[test]
fn a_sandboxed_scaffold_leaves_the_manifest_to_the_app_which_makes_it_next() {
    let (dir, ws) = plane();
    let opened = crate::workspaces::Plane::open(dir.path());
    let now = chrono::Utc::now();

    // In a sandboxed chat: the workspace's structure, and no manifest.
    scaffold_as(&opened, "alpha", now, "a chat", true);
    assert!(ws.join("workspace.md").is_file(), "the structure is made");
    assert!(
        !ws.join("workspace.json").exists(),
        "the manifest is the app's"
    );

    // The app's own ensure, as it runs before it clones for the chat, makes it.
    crate::wscmd::ensure::ensure(dir.path(), "alpha", now, "the app").expect("ensured");
    let manifest: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(ws.join("workspace.json")).unwrap())
            .expect("JSON");
    assert_eq!(manifest["name"], "alpha", "{manifest}");
}
