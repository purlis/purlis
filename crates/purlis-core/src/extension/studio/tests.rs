// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use std::path::{Path, PathBuf};

use super::*;
use crate::extension::capability::Capability;

/// A manifest asking for `capabilities`, with `extra` top-level keys and `contributes` keys.
fn manifest(
    capabilities: &[&str],
    extra: serde_json::Value,
    contributes: serde_json::Value,
) -> String {
    let mut doc = serde_json::json!({
        "version": 3,
        "id": "studio-x",
        "name": "Studio X",
        "capabilities": capabilities,
        "contributes": { "runs": "bin/x" },
    });
    for (key, value) in extra.as_object().cloned().unwrap_or_default() {
        doc[key] = value;
    }
    for (key, value) in contributes.as_object().cloned().unwrap_or_default() {
        doc["contributes"][key] = value;
    }
    doc.to_string()
}

/// `text` read as purlis reads a manifest where the `skill-studio` flag is `on`.
fn read(text: &str, on: bool) -> Result<crate::extension::Manifest, String> {
    crate::extension::parse_knowing(text, &Capability::known_where(on))
}

fn lint_only() -> serde_json::Value {
    serde_json::json!({ "skill-lint": { "title": "House style" } })
}

#[test]
fn the_six_words_refuse_the_whole_manifest_while_the_flag_is_off() {
    for word in Capability::STUDIO {
        let text = manifest(
            &[word.as_str()],
            serde_json::json!({}),
            serde_json::json!({ word.as_str(): { "title": "T" } }),
        );
        let refused = read(&text, false).expect_err("unknown with the flag off");
        assert!(
            refused.contains("which this purlis does not know"),
            "{refused}"
        );
    }
}

#[test]
fn a_studio_capability_needs_protocol_three() {
    let text = manifest(
        &["skill-lint"],
        serde_json::json!({ "version": 2 }),
        lint_only(),
    );
    let refused = read(&text, true).expect_err("protocol 2 carries no studio question");
    assert!(refused.contains("needs protocol 3"), "{refused}");
}

#[test]
fn an_extension_that_declares_one_is_confined_and_one_that_does_not_is_not() {
    let confined = read(
        &manifest(&["skill-lint"], serde_json::json!({}), lint_only()),
        true,
    )
    .expect("it reads");
    assert!(confined.confined());
    assert_eq!(
        confined.studio.as_ref().and_then(|it| it.lint.as_deref()),
        Some("House style")
    );
    let watched = read(
        &manifest(
            &["cli"],
            serde_json::json!({}),
            serde_json::json!({ "cli": [{ "name": "go", "title": "Go", "writes": false }] }),
        ),
        true,
    )
    .expect("it reads");
    assert!(!watched.confined());
}

#[test]
fn on_a_system_purlis_cannot_confine_on_none_of_the_extension_is_read() {
    let doc = serde_json::Map::new();
    let contributes = lint_only().as_object().cloned().expect("an object");
    for os in [Os::Windows, Os::Other] {
        let refused = declared_of(
            &doc,
            &contributes,
            &[Capability::SkillLint],
            Some("bin/x"),
            os,
        )
        .expect_err("refused whole");
        assert!(
            refused.contains("it loads none of this extension"),
            "{refused}"
        );
    }
    for os in [Os::MacOs, Os::Linux] {
        assert!(
            declared_of(
                &doc,
                &contributes,
                &[Capability::SkillLint],
                Some("bin/x"),
                os
            )
            .expect("read")
            .is_some()
        );
    }
}

#[test]
fn a_studio_capability_needs_a_program() {
    let doc = serde_json::Map::new();
    let contributes = lint_only().as_object().cloned().expect("an object");
    let refused = declared_of(
        &doc,
        &contributes,
        &[Capability::SkillLint],
        None,
        Os::MacOs,
    )
    .expect_err("nothing to ask");
    assert!(refused.contains("no program"), "{refused}");
}

#[test]
fn hosts_without_a_studio_capability_are_a_claim_nothing_checks_and_are_refused() {
    let text = manifest(
        &["cli"],
        serde_json::json!({ "hosts": ["example.com"] }),
        serde_json::json!({ "cli": [{ "name": "go", "title": "Go", "writes": false }] }),
    );
    let refused = read(&text, true).expect_err("refused");
    assert!(refused.contains("nothing checks"), "{refused}");
}

#[test]
fn a_declared_host_is_one_name_never_a_wildcard_an_address_or_a_port() {
    for host in [
        "*.example.com",
        "10.0.0.5",
        "example.com:8443",
        "[::1]",
        "https://example.com",
    ] {
        let text = manifest(
            &["skill-lint"],
            serde_json::json!({ "hosts": [host] }),
            lint_only(),
        );
        assert!(read(&text, true).is_err(), "{host} was taken");
    }
    let text = manifest(
        &["skill-lint"],
        serde_json::json!({ "hosts": ["Registry.Example.com.", "cdn.example.com"] }),
        lint_only(),
    );
    let read = read(&text, true).expect("names");
    assert_eq!(
        read.studio.expect("studio").hosts,
        ["registry.example.com", "cdn.example.com"]
    );
}

#[test]
fn a_host_declared_twice_or_too_many_hosts_are_refused() {
    let twice = manifest(
        &["skill-lint"],
        serde_json::json!({ "hosts": ["a.example.com", "A.example.com"] }),
        lint_only(),
    );
    assert!(read(&twice, true).expect_err("twice").contains("twice"));
    let many: Vec<String> = (0..=MOST_HOSTS)
        .map(|n| format!("h{n}.example.com"))
        .collect();
    let many = manifest(
        &["skill-lint"],
        serde_json::json!({ "hosts": many }),
        lint_only(),
    );
    assert!(read(&many, true).is_err());
}

#[test]
fn every_host_that_serves_a_model_is_refused_with_its_name() {
    for host in crate::sandbox::Preset::ModelProviders
        .own_hosts()
        .iter()
        .chain(MODEL_DOMAINS.iter())
        .chain(
            [
                "eu.api.anthropic.com",
                "bedrock-runtime.us-east-1.amazonaws.com",
            ]
            .iter(),
        )
    {
        let text = manifest(
            &["skill-lint"],
            serde_json::json!({ "hosts": [host] }),
            lint_only(),
        );
        let refused = read(&text, true).expect_err("a model host");
        assert!(refused.contains("serves a model"), "{host}: {refused}");
    }
}

#[test]
fn a_model_host_is_the_domain_or_a_name_under_it_and_no_look_alike() {
    assert!(model_host("api.anthropic.com"));
    assert!(model_host("API.Anthropic.com."));
    assert!(model_host("x.openai.azure.com"));
    assert!(model_host("bedrock-runtime.eu-west-1.amazonaws.com"));
    assert!(!model_host("s3.eu-west-1.amazonaws.com"));
    assert!(!model_host("notanthropic.com"));
    assert!(!model_host("anthropic.com.example.org"));
    assert!(!model_host("registry.example.com"));
}

#[test]
fn the_proxy_leaves_out_a_model_host_whatever_the_manifest_said() {
    let declared = vec![
        "registry.example.com".to_owned(),
        "api.anthropic.com".to_owned(),
        "openrouter.ai".to_owned(),
    ];
    assert_eq!(reachable(&declared), ["registry.example.com"]);
}

#[test]
fn a_confined_extension_writes_nothing_of_the_project() {
    let writes = manifest(
        &["skill-lint", "writes"],
        serde_json::json!({}),
        serde_json::json!({ "skill-lint": { "title": "T" }, "writes": ["notes/"] }),
    );
    let refused = read(&writes, true).expect_err("plane writes");
    assert!(
        refused.contains("writes nothing of the project"),
        "{refused}"
    );
    let folder = manifest(
        &["skill-lint", "events"],
        serde_json::json!({}),
        serde_json::json!({
            "skill-lint": { "title": "T" },
            "events": { "hears": ["plane-saved"], "workspace_folder": "probe" },
        }),
    );
    let refused = read(&folder, true).expect_err("a folder in each workspace");
    assert!(
        refused.contains("writes nothing of the project"),
        "{refused}"
    );
}

#[test]
fn templates_and_panels_are_read_against_an_allow_list() {
    let good = manifest(
        &["skill-template", "studio-panel"],
        serde_json::json!({}),
        serde_json::json!({
            "skill-template": [{ "id": "house", "title": "House", "about": "Our shape." }],
            "studio-panel": [{ "id": "checks", "title": "Checks" }],
        }),
    );
    let read_ok = read(&good, true).expect("reads");
    let studio = read_ok.studio.expect("studio");
    assert_eq!(studio.templates[0].id, "house");
    assert_eq!(studio.panels[0].title, "Checks");
    for bad in [
        serde_json::json!({ "skill-template": [{ "id": "house", "title": "H", "about": "A", "file": "x" }], "studio-panel": [{ "id": "c", "title": "C" }] }),
        serde_json::json!({ "skill-template": [{ "id": "../up", "title": "H", "about": "A" }], "studio-panel": [{ "id": "c", "title": "C" }] }),
        serde_json::json!({ "skill-template": [], "studio-panel": [{ "id": "c", "title": "C" }] }),
        serde_json::json!({ "skill-template": [{ "id": "h", "title": "H", "about": "A" }], "studio-panel": [{ "id": "c", "title": "C\u{202e}" }] }),
        serde_json::json!({ "skill-template": [{ "id": "h", "title": "H", "about": "A" }, { "id": "h", "title": "H", "about": "A" }], "studio-panel": [{ "id": "c", "title": "C" }] }),
    ] {
        let text = manifest(
            &["skill-template", "studio-panel"],
            serde_json::json!({}),
            bad.clone(),
        );
        assert!(read(&text, true).is_err(), "{bad} was taken");
    }
}

#[test]
fn the_approval_says_what_holds_it_from_the_core() {
    assert!(confined_said(&[]).contains("It reaches no host on the network"));
    assert!(confined_said(&["a.example.com".to_owned()]).contains("only a.example.com"));
    let two = confined_said(&["a.example.com".to_owned(), "b.example.com".to_owned()]);
    assert!(
        two.contains("only a.example.com and b.example.com"),
        "{two}"
    );
    assert!(two.contains("never a host that serves a model"), "{two}");
    assert!(!two.contains("runs as you do"), "{two}");
}

// ---------------------------------------------------------------------------------------
// The profile a confined program runs in, as purlis's Linux sandbox reads it
// ---------------------------------------------------------------------------------------

/// The profile for an extension at `ext` writing `state` and `tmp`, read as the Linux sandbox
/// reads it: the same profile Seatbelt is handed on macOS, compiled once.
fn held(ext: &Path, state: &Path, tmp: &Path) -> purlis_linux_sandbox::profile::Profile {
    let text = crate::sandbox::seatbelt::confined_profile(
        Os::Linux,
        &[ext.to_path_buf()],
        &[state.to_path_buf(), tmp.to_path_buf()],
        &[40_001, 40_002],
    )
    .expect("a profile");
    purlis_linux_sandbox::profile::Profile::parse(&text).expect("the Linux sandbox reads it")
}

fn reads(profile: &purlis_linux_sandbox::profile::Profile, path: &str) -> bool {
    use purlis_linux_sandbox::profile::{FileOp, Kind, Op};
    profile.allows(&Op::File {
        what: FileOp::ReadData,
        path: Path::new(path),
        kind: Kind::Regular,
    })
}

fn writes(profile: &purlis_linux_sandbox::profile::Profile, path: &str) -> bool {
    use purlis_linux_sandbox::profile::{FileOp, Kind, Op};
    profile.allows(&Op::File {
        what: FileOp::WriteData,
        path: Path::new(path),
        kind: Kind::Regular,
    })
}

fn reaches(profile: &purlis_linux_sandbox::profile::Profile, ip: &str, port: u16) -> bool {
    use purlis_linux_sandbox::profile::{Op, Remote};
    profile.allows(&Op::Connect(Remote::Ip {
        ip: ip.parse().expect("an address"),
        port,
    }))
}

const EXT: &str = "/opt/ext/studio-x";
const STATE: &str = "/opt/ext/studio-x/state";
/// Paths that resolve the same on every system the tests run on: no `/var` or `/etc`, which macOS
/// keeps under `/private`.
const TMP: &str = "/opt/tmp/purlis-extension-1";

#[test]
fn a_confined_program_reads_its_own_folder_and_the_system_and_nothing_a_person_keeps() {
    let profile = held(Path::new(EXT), Path::new(STATE), Path::new(TMP));
    for read in [
        "/opt/ext/studio-x/purlis-extension.json",
        "/opt/ext/studio-x/bin/x",
        "/usr/lib/x86_64-linux-gnu/libc.so.6",
        "/dev/urandom",
    ] {
        assert!(reads(&profile, read), "{read} is not read");
    }
    for kept in [
        "/home/ada/.ssh/id_ed25519",
        "/home/ada/work/plane/charter.toml",
        "/opt/ext/other/purlis-extension.json",
        "/opt/ext/studio-x-twin/bin/x",
        "/etc/shadow",
        "/etc/passwd",
        "/root/.bashrc",
        "/proc/1/environ",
    ] {
        assert!(!reads(&profile, kept), "{kept} is read");
    }
}

#[test]
fn a_confined_program_lists_no_folder_above_its_own() {
    use purlis_linux_sandbox::profile::{FileOp, Kind, Op};
    let profile = held(Path::new(EXT), Path::new(STATE), Path::new(TMP));
    let listed = |what: FileOp, path: &str| {
        profile.allows(&Op::File {
            what,
            path: Path::new(path),
            kind: Kind::Directory,
        })
    };
    // Each folder above its own resolves as a folder, and lists nothing.
    assert!(listed(FileOp::ReadMetadata, "/opt/ext"));
    assert!(listed(FileOp::ReadMetadata, "/"));
    assert!(!listed(FileOp::ReadData, "/opt/ext"));
    assert!(!listed(FileOp::ReadData, "/"));
    assert!(!listed(FileOp::ReadMetadata, "/opt/ext/other"));
}

#[test]
fn a_confined_program_writes_its_state_and_its_temporary_folder_and_nothing_else() {
    let profile = held(Path::new(EXT), Path::new(STATE), Path::new(TMP));
    assert!(writes(&profile, "/opt/ext/studio-x/state/cache.json"));
    assert!(writes(&profile, "/opt/tmp/purlis-extension-1/scratch"));
    for kept in [
        "/opt/ext/studio-x/bin/x",
        "/opt/ext/studio-x/purlis-extension.json",
        "/home/ada/work/plane/skills/x/SKILL.md",
        "/opt/tmp/purlis-extension-2/scratch",
        "/usr/bin/env",
    ] {
        assert!(!writes(&profile, kept), "{kept} is written");
    }
}

#[test]
fn a_confined_program_reaches_its_proxy_and_no_other_address() {
    let profile = held(Path::new(EXT), Path::new(STATE), Path::new(TMP));
    assert!(reaches(&profile, "127.0.0.1", 40_001));
    assert!(reaches(&profile, "127.0.0.1", 40_002));
    assert!(
        !reaches(&profile, "127.0.0.1", 40_003),
        "another port of this machine"
    );
    assert!(
        !reaches(&profile, "160.79.104.10", 443),
        "a model provider, directly"
    );
    assert!(!reaches(&profile, "192.0.2.1", 443), "any host, directly");
    let none = crate::sandbox::seatbelt::confined_profile(
        Os::Linux,
        &[PathBuf::from(EXT)],
        &[PathBuf::from(TMP)],
        &[],
    )
    .expect("a profile");
    let none = purlis_linux_sandbox::profile::Profile::parse(&none).expect("read");
    assert!(
        !reaches(&none, "127.0.0.1", 40_001),
        "no host declared, no proxy"
    );
}

#[test]
fn the_denial_classes_hold_inside_its_own_folder() {
    // A vault under its own folder, and a later program's name in its state folder.
    let profile = held(Path::new(EXT), Path::new(STATE), Path::new(TMP));
    assert!(!reads(
        &profile,
        "/opt/ext/studio-x/.purlis/vaults/ops.json"
    ));
    assert!(!writes(
        &profile,
        "/opt/ext/studio-x/state/.git/hooks/pre-commit"
    ));
}

#[test]
fn the_macos_profile_reads_the_systems_programs_and_no_home() {
    let text = crate::sandbox::seatbelt::confined_profile(
        Os::MacOs,
        &[PathBuf::from("/Users/ada/ext/studio-x")],
        &[PathBuf::from("/private/var/folders/x/T/purlis-extension-1")],
        &[40_001],
    )
    .expect("a profile");
    assert!(text.starts_with("(version 1)\n(deny default)\n"), "{text}");
    assert!(text.contains("(subpath \"/System\")"), "{text}");
    assert!(!text.contains("(subpath \"/Users\")"), "{text}");
    assert!(!text.contains("user-preference-read"), "{text}");
    assert!(!text.contains("com.apple.FSEvents"), "{text}");
    assert!(!text.contains("opendirectoryd"), "{text}");
    let profile = purlis_linux_sandbox::profile::Profile::parse(&text).expect("read");
    assert!(reads(&profile, "/Users/ada/ext/studio-x/bin/x"));
    assert!(!reads(
        &profile,
        "/Users/ada/Library/Keychains/login.keychain-db"
    ));
    assert!(!reads(&profile, "/Users/ada/.ssh/id_ed25519"));
}

// ---------------------------------------------------------------------------------------
// The questions and their answers
// ---------------------------------------------------------------------------------------

fn doc(value: serde_json::Value) -> serde_json::Map<String, serde_json::Value> {
    value.as_object().cloned().expect("an object")
}

#[test]
fn each_question_says_what_it_asks_and_hands_a_skill_as_its_files() {
    let files = [HandedFile {
        path: "SKILL.md".to_owned(),
        text: "---\n".to_owned(),
        executable: false,
    }];
    assert_eq!(Ask::Skills.line(), serde_json::json!({ "ask": "skills" }));
    assert_eq!(
        Ask::Findings {
            skill: "deploy",
            files: &files
        }
        .line(),
        serde_json::json!({
            "ask": "findings",
            "skill": "deploy",
            "files": [{ "path": "SKILL.md", "text": "---\n", "executable": false }],
        })
    );
    assert_eq!(Ask::Skills.capability(), Capability::SkillSource);
    assert_eq!(
        Ask::Template {
            template: "t",
            name: "n",
            description: "d"
        }
        .capability(),
        Capability::SkillTemplate
    );
}

#[test]
fn a_question_hands_text_files_within_its_bounds() {
    let mut files: Vec<(String, Vec<u8>, bool)> = (0..100)
        .map(|n| (format!("f{n:03}.md"), b"words\n".to_vec(), false))
        .collect();
    files.push(("binary.png".to_owned(), vec![0xff, 0xfe, 0x00], false));
    files.push((
        "z-big.md".to_owned(),
        vec![b'x'; MOST_HANDED_BYTES + 1],
        false,
    ));
    let handed = handed(&files);
    assert_eq!(handed.len(), MOST_HANDED_FILES);
    assert!(handed.iter().all(|it| it.path != "binary.png"));
}

#[test]
fn a_sources_listing_is_read_failing_closed() {
    let offered = offered_of(
        "x",
        &doc(serde_json::json!({ "revision": "r1", "skills": [{ "name": "deploy", "description": "D" }] })),
    )
    .expect("read");
    assert_eq!(offered.skills, [("deploy".to_owned(), "D".to_owned())]);
    for bad in [
        serde_json::json!({ "skills": [] }),
        serde_json::json!({ "revision": "../r", "skills": [] }),
        serde_json::json!({ "revision": "r1", "skills": [{ "name": "../up" }] }),
        serde_json::json!({ "revision": "r1", "skills": [{ "name": "a" }, { "name": "a" }] }),
        serde_json::json!({ "revision": "r1", "skills": [{ "name": "a", "path": "/etc" }] }),
        serde_json::json!({ "revision": "r1", "skills": [{ "name": "a", "description": "a\u{200b}b" }] }),
    ] {
        assert!(
            offered_of("x", &doc(bad.clone())).is_err(),
            "{bad} was taken"
        );
    }
    let many: Vec<serde_json::Value> = (0..=MOST_OFFERED)
        .map(|n| serde_json::json!({ "name": format!("s{n}") }))
        .collect();
    assert!(
        offered_of(
            "x",
            &doc(serde_json::json!({ "revision": "r", "skills": many }))
        )
        .is_err()
    );
}

#[test]
fn a_skills_files_are_plain_paths_inside_its_folder_at_the_revision_asked() {
    let good = doc(serde_json::json!({
        "revision": "r1",
        "files": [
            { "path": "SKILL.md", "text": "---\n" },
            { "path": "scripts/run.sh", "text": "#!/bin/sh\n", "executable": true },
        ],
    }));
    let files = files_of("x", "r1", &good).expect("read");
    assert_eq!(files.len(), 2);
    assert!(files[1].executable);
    assert!(files_of("x", "r2", &good).is_err(), "another revision");
    for path in [
        "../SKILL.md",
        "/etc/passwd",
        "a//b",
        "a/./b",
        "",
        "a\\b/../../c",
    ] {
        let bad = doc(serde_json::json!({
            "revision": "r1",
            "files": [{ "path": "SKILL.md", "text": "" }, { "path": path, "text": "x" }],
        }));
        assert!(files_of("x", "r1", &bad).is_err(), "{path:?} was taken");
    }
    let no_skill_md =
        doc(serde_json::json!({ "revision": "r1", "files": [{ "path": "a.md", "text": "" }] }));
    assert!(files_of("x", "r1", &no_skill_md).is_err());
    let twice = doc(serde_json::json!({
        "revision": "r1",
        "files": [{ "path": "SKILL.md", "text": "" }, { "path": "SKILL.md", "text": "" }],
    }));
    assert!(files_of("x", "r1", &twice).is_err());
}

#[test]
fn a_lints_findings_are_lines_of_words_within_bounds() {
    let said = findings_of(
        "x",
        &doc(serde_json::json!({ "findings": [{ "says": " too long " }] })),
    )
    .expect("read");
    assert_eq!(said, ["too long"]);
    for bad in [
        serde_json::json!({ "findings": [{ "says": "" }] }),
        serde_json::json!({ "findings": [{ "says": "a\u{202e}b" }] }),
        serde_json::json!({ "findings": [{ "says": "x".repeat(MOST_FINDING_BYTES + 1) }] }),
        serde_json::json!({ "findings": [{ "says": "ok", "level": "blocks" }] }),
        serde_json::json!({ "findings": "none" }),
    ] {
        assert!(
            findings_of("x", &doc(bad.clone())).is_err(),
            "{bad} was taken"
        );
    }
}

#[test]
fn a_templates_text_is_a_whole_skill_that_keeps_its_name() {
    let text = "---\nname: made\ndescription: Made from a template, for a test.\n---\n\n# made\n";
    assert_eq!(
        template_of("x", "made", &doc(serde_json::json!({ "text": text }))).expect("read"),
        text
    );
    let renamed = text.replace("name: made", "name: other");
    assert!(template_of("x", "made", &doc(serde_json::json!({ "text": renamed }))).is_err());
    assert!(
        template_of(
            "x",
            "made",
            &doc(serde_json::json!({ "text": "no frontmatter" }))
        )
        .is_err()
    );
}

#[test]
fn a_panels_blocks_are_read_against_the_panel_vocabulary() {
    let blocks = blocks_of(
        "x",
        &doc(serde_json::json!({ "blocks": [{
            "kind": "table",
            "caption": "Checks",
            "columns": ["Check", "Result"],
            "rows": [{ "key": "a", "cells": ["lint", ""] }],
        }] })),
    )
    .expect("read");
    let crate::panel::Block::Table(table) = &blocks[0] else {
        panic!("{blocks:?}");
    };
    assert_eq!(table.rows[0].cells, ["lint", ""]);
    for bad in [
        serde_json::json!({ "blocks": [{ "kind": "table", "caption": "C", "columns": ["A"], "rows": [{ "key": "a", "cells": ["1", "2"] }] }] }),
        serde_json::json!({ "blocks": [{ "kind": "table", "caption": "C", "columns": [], "rows": [] }] }),
        serde_json::json!({ "blocks": [{ "kind": "table", "caption": "C", "columns": ["A"], "rows": [{ "key": "a", "cells": ["1"] }, { "key": "a", "cells": ["2"] }] }] }),
        serde_json::json!({ "blocks": [{ "kind": "html", "text": "<b>" }] }),
    ] {
        assert!(
            blocks_of("x", &doc(bad.clone())).is_err(),
            "{bad} was taken"
        );
    }
}

#[test]
fn a_grade_is_four_messages_two_each_way() {
    let from_purlis: Vec<GradeMessage> = GradeMessage::ALL
        .into_iter()
        .filter(|it| it.from_purlis())
        .collect();
    assert_eq!(
        from_purlis,
        [GradeMessage::GradeRequest, GradeMessage::RunChatResult]
    );
    assert_eq!(
        serde_json::to_value([Arm::With, Arm::Without, Arm::Previous]).expect("words"),
        serde_json::json!(["with", "without", "previous"])
    );
}
