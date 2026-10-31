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

fn host(name: &str) -> Host {
    Host::parse(name).expect("a host")
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
    let hosts: Vec<String> = read
        .studio
        .expect("studio")
        .hosts
        .iter()
        .map(ToString::to_string)
        .collect();
    assert_eq!(hosts, ["registry.example.com", "cdn.example.com"]);
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
    assert!(model_host(&host("api.anthropic.com")));
    assert!(model_host(&host("API.Anthropic.com.")));
    assert!(model_host(&host("x.openai.azure.com")));
    assert!(model_host(&host("bedrock-runtime.eu-west-1.amazonaws.com")));
    assert!(!model_host(&host("s3.eu-west-1.amazonaws.com")));
    assert!(!model_host(&host("notanthropic.com")));
    assert!(!model_host(&host("anthropic.com.example.org")));
    assert!(!model_host(&host("registry.example.com")));
}

#[test]
fn the_proxy_leaves_out_a_model_host_whatever_the_manifest_said() {
    let declared = vec![
        host("registry.example.com"),
        host("api.anthropic.com"),
        host("openrouter.ai"),
    ];
    assert_eq!(reachable(&declared), [host("registry.example.com")]);
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
    assert!(confined_said(&[host("a.example.com")]).contains("only a.example.com"));
    let two = confined_said(&[host("a.example.com"), host("b.example.com")]);
    assert!(
        two.contains("only a.example.com and b.example.com"),
        "{two}"
    );
    assert!(two.contains("never to a host that serves a model"), "{two}");
    assert!(!two.contains("runs as you do"), "{two}");
}

// ---------------------------------------------------------------------------------------
// The profile a confined program runs in, as purlis's Linux sandbox reads it
// ---------------------------------------------------------------------------------------

/// The profile for an extension at `ext` writing `state` and `tmp`, read as the Linux sandbox
/// reads it: the same profile Seatbelt is handed on macOS, compiled once.
fn held(ext: &Path, state: &Path, tmp: &Path) -> purlis_linux_sandbox::profile::Profile {
    let text = crate::sandbox::confined::profile(
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
fn a_confined_program_starts_programs_from_what_it_reads_on_linux() {
    let profile = held(Path::new(EXT), Path::new(STATE), Path::new(TMP));
    let roots = profile.starts_from();
    for root in [EXT, STATE, TMP, "/usr/bin", "/usr/lib"] {
        assert!(roots.contains(&PathBuf::from(root)), "{root}: {roots:?}");
    }
    assert!(!roots.contains(&PathBuf::from("/")), "{roots:?}");
    // Every root is one it reads, and nothing a person keeps is beneath one.
    for root in &roots {
        assert!(
            reads(&profile, &root.join("program").display().to_string())
                || reads(&profile, &root.display().to_string()),
            "{}",
            root.display()
        );
    }
    for kept in [
        "/home",
        "/root",
        "/opt/ext/other",
        "/opt/ext/studio-x-twin",
        "/etc/shadow",
    ] {
        assert!(
            !roots.iter().any(|root| Path::new(kept).starts_with(root)),
            "{kept}: {roots:?}"
        );
    }
}

#[test]
fn a_confined_program_starts_programs_from_what_it_reads_on_macos() {
    let text = crate::sandbox::confined::profile(
        Os::MacOs,
        &[PathBuf::from("/Users/ada/ext/studio-x")],
        &[PathBuf::from("/private/var/folders/x/T/purlis-extension-1")],
        &[40_001],
    )
    .expect("a profile");
    assert!(!text.contains("(allow process-exec)"), "{text}");
    assert!(
        text.contains("(allow file-read* file-map-executable process-exec\n"),
        "{text}"
    );
    assert!(
        text.contains(
            "(deny file-read* file-map-executable process-exec (subpath \"/System/Volumes/Data\"))"
        ),
        "{text}"
    );
    assert!(text.contains("(deny process-exec (regex "), "{text}");
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
    // The two loopback addresses the proxy holds, and no other on Linux's 127/8.
    assert!(reaches(&profile, "::1", 40_001));
    assert!(reaches(&profile, "::ffff:127.0.0.1", 40_001));
    assert!(
        !reaches(&profile, "127.0.0.2", 40_001),
        "another loopback address"
    );
    assert!(
        !reaches(&profile, "127.1.2.3", 40_002),
        "another loopback address"
    );
    assert!(
        !reaches(&profile, "160.79.104.10", 443),
        "a model provider, directly"
    );
    assert!(!reaches(&profile, "192.0.2.1", 443), "any host, directly");
    let none = crate::sandbox::confined::profile(
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
    let text = crate::sandbox::confined::profile(
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
    assert!(
        text.contains("(allow file-read* file-map-executable"),
        "{text}"
    );
    // Of the person's home, only the extension's own folder is a subpath, and the home itself
    // only a literal its path resolves through.
    let homes: Vec<&str> = text
        .lines()
        .filter(|line| line.contains("/Users/ada") && line.contains("(subpath"))
        .collect();
    assert!(
        homes
            .iter()
            .all(|line| line.contains("/Users/ada/ext/studio-x")),
        "{homes:?}"
    );
    assert!(text.contains("(literal \"/Users/ada\")"), "{text}");
    // Read as Linux's reader reads it, the one operation it has no word for set aside: the data
    // volume, which holds every home under a second name, is not the system's.
    let read =
        purlis_linux_sandbox::profile::Profile::parse(&text.replace(" file-map-executable", ""))
            .expect("read");
    assert!(reads(
        &read,
        "/System/Library/Frameworks/CoreFoundation.framework/CoreFoundation"
    ));
    assert!(reads(&read, "/Users/ada/ext/studio-x/bin/x"));
    for kept in [
        "/System/Volumes/Data/Users/ada/.ssh/id_ed25519",
        "/System/Volumes/Data/Users/ada/work/plane/charter.toml",
        "/Users/ada/.ssh/id_ed25519",
        "/Users/ada/Library/Keychains/login.keychain-db",
    ] {
        assert!(!reads(&read, kept), "{kept} is read");
    }
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
    let file = |path: String, bytes: Vec<u8>| SkillFile {
        path,
        bytes,
        executable: false,
    };
    let mut files: Vec<SkillFile> = (0..100)
        .map(|n| file(format!("f{n:03}.md"), b"words\n".to_vec()))
        .collect();
    files.push(file("binary.png".to_owned(), vec![0xff, 0xfe, 0x00]));
    files.push(file(
        "z-big.md".to_owned(),
        vec![b'x'; MOST_HANDED_BYTES + 1],
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
    assert_eq!(
        offered.skills,
        [OfferedSkill {
            name: "deploy".to_owned(),
            description: "D".to_owned(),
        }]
    );
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

// ---------------------------------------------------------------------------------------
// What the review asked to be held (#3088)
// ---------------------------------------------------------------------------------------

/// The macOS profile for a program in `ext`, read as Linux's reader reads it, the one operation
/// it has no word for set aside.
fn held_on_macos(ext: &str) -> purlis_linux_sandbox::profile::Profile {
    let text = crate::sandbox::confined::profile(
        Os::MacOs,
        &[PathBuf::from(ext)],
        &[PathBuf::from("/opt/tmp/purlis-extension-1")],
        &[40_001],
    )
    .expect("a profile");
    purlis_linux_sandbox::profile::Profile::parse(&text.replace(" file-map-executable", ""))
        .expect("read")
}

#[test]
fn a_package_managers_programs_are_read_and_its_configuration_and_state_are_not() {
    let macos = held_on_macos("/Users/ada/ext/studio-x");
    let linux = held(Path::new(EXT), Path::new(STATE), Path::new(TMP));
    // Homebrew on Apple silicon and on Intel, and the system's own prefix.
    for (profile, programs, kept) in [
        (
            &macos,
            [
                "/opt/homebrew/bin/python3",
                "/opt/homebrew/Cellar/python@3.13/3.13.1/bin/python3",
                "/opt/homebrew/opt/openssl@3/lib/libssl.3.dylib",
                "/usr/local/bin/python3",
                "/usr/local/Cellar/git/2.47.0/bin/git",
                "/usr/lib/libSystem.B.dylib",
            ],
            [
                "/opt/homebrew/etc/openssl@3/private/key.pem",
                "/opt/homebrew/var/postgresql@16/postgresql.conf",
                "/usr/local/etc/my.cnf",
                "/usr/local/var/mysql/ibdata1",
                "/usr/local/secrets.txt",
            ],
        ),
        (
            &linux,
            [
                "/usr/bin/python3",
                "/usr/lib/x86_64-linux-gnu/libssl.so.3",
                "/usr/local/bin/tool",
                "/usr/local/lib/libtool.so",
                "/usr/share/zoneinfo/UTC",
                "/usr/libexec/tool",
            ],
            [
                "/usr/local/etc/tool.conf",
                "/usr/local/var/db/state",
                "/usr/local/secrets.txt",
                "/usr/local/var/log/x",
                "/usr/local/etc/ssl/private.pem",
            ],
        ),
    ] {
        for read in programs {
            assert!(reads(profile, read), "{read} is not read");
        }
        for kept in kept {
            assert!(!reads(profile, kept), "{kept} is read");
        }
    }
}

#[test]
fn no_other_process_is_read_through_the_systems_settings() {
    let text = crate::sandbox::confined::profile(Os::MacOs, &[PathBuf::from(EXT)], &[], &[])
        .expect("a profile");
    assert!(
        text.contains("(deny sysctl-read (sysctl-name-prefix \"kern.proc\"))"),
        "{text}"
    );
}

#[test]
fn a_private_or_special_use_address_is_never_reached() {
    use crate::sandbox::hosts::special_use;
    let ip = |text: &str| text.parse::<std::net::IpAddr>().expect("an address");
    for private in [
        "10.1.2.3",
        "172.16.0.1",
        "172.31.255.254",
        "192.168.1.1",
        "100.64.0.1",
        "100.127.255.254",
        "127.0.0.1",
        "169.254.169.254",
        "0.0.0.0",
        "192.0.2.10",
        "198.18.0.1",
        "198.51.100.1",
        "203.0.113.1",
        "224.0.0.1",
        "240.0.0.1",
        "255.255.255.255",
        "::1",
        "fc00::1",
        "fd12:3456::1",
        "fe80::1",
        "2001:db8::1",
        "100::1",
        "::ffff:10.0.0.1",
        "64:ff9b::a00:1",
        "2002:a00:1::1",
    ] {
        assert!(special_use(ip(private)), "{private} is reached");
    }
    for public in [
        "160.79.104.10",
        "93.184.216.34",
        "2606:4700::6810:84e5",
        "100.128.0.1",
    ] {
        assert!(!special_use(ip(public)), "{public} is refused");
    }
}

/// A TLS record holding a ClientHello naming `name`, as a client writes one.
fn client_hello(name: &str) -> Vec<u8> {
    client_hello_with(name, false)
}

/// [`client_hello`], carrying an encrypted ClientHello (ECH) after its server name where `ech`.
fn client_hello_with(name: &str, ech: bool) -> Vec<u8> {
    let mut extra = Vec::new();
    if ech {
        extra.extend(0xfe0du16.to_be_bytes());
        extra.extend(4u16.to_be_bytes());
        extra.extend([0, 0, 1, 0]);
    }
    client_hello_carrying(name, &extra)
}

/// A TLS record holding a TLS 1.3 ClientHello naming `name`, with the extensions `extra` after
/// its server name.
fn client_hello_carrying(name: &str, extra: &[u8]) -> Vec<u8> {
    let name = name.as_bytes();
    let mut list = Vec::new();
    list.extend(u16::try_from(3 + name.len()).expect("short").to_be_bytes());
    list.push(0);
    list.extend(u16::try_from(name.len()).expect("short").to_be_bytes());
    list.extend(name);
    let mut extensions = Vec::new();
    extensions.extend(43u16.to_be_bytes());
    extensions.extend(3u16.to_be_bytes());
    extensions.extend([2, 3, 4]);
    extensions.extend(0u16.to_be_bytes());
    extensions.extend(u16::try_from(list.len()).expect("short").to_be_bytes());
    extensions.extend(list);
    extensions.extend(extra);
    let mut hello = vec![3, 3];
    hello.extend([7u8; 32]);
    hello.push(0);
    hello.extend(2u16.to_be_bytes());
    hello.extend([0x13, 0x01]);
    hello.extend([1, 0]);
    hello.extend(
        u16::try_from(extensions.len())
            .expect("short")
            .to_be_bytes(),
    );
    hello.extend(extensions);
    let mut handshake = vec![1, 0];
    handshake.extend(u16::try_from(hello.len()).expect("short").to_be_bytes());
    handshake.extend(hello);
    let mut record = vec![0x16, 3, 1];
    record.extend(u16::try_from(handshake.len()).expect("short").to_be_bytes());
    record.extend(handshake);
    record
}

#[test]
fn a_tunnels_client_hello_is_read_for_the_one_host_it_names() {
    use crate::sandbox::egress::hello_names;
    assert_eq!(
        hello_names(&client_hello("registry.example.com")).as_deref(),
        Some("registry.example.com")
    );
    assert_eq!(
        hello_names(&client_hello("api.anthropic.com.")).as_deref(),
        Some("api.anthropic.com")
    );
    // Not a handshake, cut short, or lengths that run past what was read.
    let whole = client_hello("registry.example.com");
    assert_eq!(hello_names(b"GET / HTTP/1.1\r\n\r\n"), None);
    assert_eq!(hello_names(&whole[..whole.len() - 3]), None);
    let mut lying = whole.clone();
    lying[3] = 0xff;
    assert_eq!(hello_names(&lying), None);
    for cut in 0..whole.len() {
        let _ = hello_names(&whole[..cut]);
    }
    // An encrypted ClientHello names its real server inside: the outer name is not believed.
    assert_eq!(
        hello_names(&client_hello_with("registry.example.com", true)),
        None
    );
}

#[test]
fn each_confined_extension_is_given_one_pair_of_ports_for_the_whole_run() {
    let hosts = vec!["registry.example.com".to_owned()];
    let first = proxy_for("one-pair-a", hosts.clone()).expect("a proxy");
    let again = proxy_for("one-pair-a", hosts.clone()).expect("the same proxy");
    assert_eq!(
        (first.port(), first.socks_port()),
        (again.port(), again.socks_port()),
        "a second question is asked through the same ports"
    );
    let before = proxies_held();
    for _ in 0..20 {
        let _ = proxy_for("one-pair-a", hosts.clone()).expect("the same proxy");
    }
    assert_eq!(proxies_held(), before, "asking again holds no more ports");
    let other = proxy_for("one-pair-b", hosts).expect("another extension's proxy");
    assert_ne!(other.port(), first.port());
    assert_eq!(proxies_held(), before + 1);
}

#[test]
fn an_answer_with_a_key_its_question_does_not_have_is_refused() {
    let refused = findings_of(
        "x",
        &doc(serde_json::json!({ "findings": [], "notes": "more" })),
    )
    .expect_err("an unknown key");
    assert!(refused.contains("cannot read"), "{refused}");
    assert!(template_of("x", "made", &doc(serde_json::json!({}))).is_err());
}

/// What a client connecting to `at` hears from a confined proxy asked for a tunnel to a declared
/// host that never resolves: the proxy's status line, or nothing.
fn heard_at(at: std::net::SocketAddr) -> String {
    use std::io::{Read, Write};
    let Ok(mut stream) =
        std::net::TcpStream::connect_timeout(&at, std::time::Duration::from_secs(2))
    else {
        return String::new();
    };
    let _ = stream.set_read_timeout(Some(std::time::Duration::from_secs(5)));
    let _ =
        stream.write_all(b"CONNECT probe.invalid:443 HTTP/1.1\r\nHost: probe.invalid:443\r\n\r\n");
    // The whole status line, however the bytes arrive.
    let mut head: Vec<u8> = Vec::new();
    let mut byte = [0u8; 1];
    while head.len() < 64 && !head.ends_with(b"\r\n") {
        match stream.read(&mut byte) {
            Ok(1) => head.push(byte[0]),
            _ => break,
        }
    }
    String::from_utf8_lossy(&head).into_owned()
}

#[test]
fn a_confined_proxy_carries_only_while_a_question_is_answered_and_never_once_turned_off() {
    let proxy = proxy_for("gate-a", vec!["probe.invalid".to_owned()]).expect("a proxy");
    let at = std::net::SocketAddr::from(([127, 0, 0, 1], proxy.port()));
    assert!(
        !heard_at(at).starts_with("HTTP/1.1"),
        "between questions the proxy carried a connection"
    );
    let asking = Asking::of(std::sync::Arc::clone(&proxy));
    let during = heard_at(at);
    assert!(
        during.starts_with("HTTP/1.1 502"),
        "during a question the proxy did not carry: {during:?}"
    );
    // On the IPv6 loopback too, which a sandbox rule for the port lets the program reach.
    if std::net::TcpListener::bind((std::net::Ipv6Addr::LOCALHOST, 0)).is_ok() {
        let v6 = std::net::SocketAddr::from((std::net::Ipv6Addr::LOCALHOST, proxy.port()));
        let heard = heard_at(v6);
        assert!(
            heard.starts_with("HTTP/1.1 502"),
            "[::1] on the proxy's port was not the proxy's: {heard:?}"
        );
    }
    drop(asking);
    assert!(
        !heard_at(at).starts_with("HTTP/1.1"),
        "after the question it carried"
    );
    // Turned off: its proxy is closed for good, its ports still held by nothing that answers.
    let port = proxy.port();
    drop(proxy);
    close_proxy("gate-a");
    let again = std::net::SocketAddr::from(([127, 0, 0, 1], port));
    assert!(
        !heard_at(again).starts_with("HTTP/1.1"),
        "after turn-off it carried"
    );
    assert!(
        std::net::TcpListener::bind(again).is_err(),
        "a closed proxy's port was given to another listener"
    );
}

#[test]
fn a_name_that_resolves_to_a_private_network_is_not_connected_to() {
    use crate::sandbox::egress::{Connected, carried_when_confined, connect_held_resolving};
    let reach = crate::sandbox::reach::Reach::open(vec!["registry.example.com".to_owned()]);
    for private in ["[fd00::1]:443", "10.0.0.1:443", "100.64.0.1:443"] {
        let at: std::net::SocketAddr = private.parse().expect("an address");
        let connected = connect_held_resolving(
            "registry.example.com",
            443,
            &reach,
            &[],
            &carried_when_confined,
            &|_, _| vec![at],
        );
        assert!(
            matches!(connected, Connected::OnlyLocal),
            "{private} was connected to"
        );
    }
}

/// A tunnel through a confined proxy to `host`, its ClientHello sent and the start of the
/// server's answer read, so the tunnel is carried and open; or `None` where `host` cannot be
/// reached from here (never on CI).
fn open_tunnel(proxy: &crate::sandbox::egress::Proxy, host: &str) -> Option<std::net::TcpStream> {
    use std::io::{Read, Write};
    let mut stream = std::net::TcpStream::connect(("127.0.0.1", proxy.port())).ok()?;
    stream
        .set_read_timeout(Some(std::time::Duration::from_secs(10)))
        .ok()?;
    write!(
        stream,
        "CONNECT {host}:443 HTTP/1.1\r\nHost: {host}:443\r\n\r\n"
    )
    .ok()?;
    let mut head = Vec::new();
    let mut byte = [0u8; 1];
    while !head.ends_with(b"\r\n\r\n") && head.len() < 4096 {
        match stream.read(&mut byte) {
            Ok(1) => head.push(byte[0]),
            _ => break,
        }
    }
    if !String::from_utf8_lossy(&head).starts_with("HTTP/1.1 200") {
        assert!(
            std::env::var_os("CI").is_none(),
            "on CI, the tunnel to {host} was not carried: {}",
            String::from_utf8_lossy(&head)
        );
        return None;
    }
    stream.write_all(&answered_client_hello(host)).ok()?;
    let mut answered = [0u8; 1];
    let carried = matches!(stream.read(&mut answered), Ok(1));
    assert!(
        carried || std::env::var_os("CI").is_none(),
        "on CI, {host} did not answer through the tunnel"
    );
    carried.then_some(stream)
}

/// **A ClientHello naming `name` that a real server goes on with**: [`client_hello`] lacks the
/// key share, groups and signature algorithms TLS 1.3 needs, so a server answers it with an
/// alert and hangs up, which ends a tunnel whatever the proxy does. Given these, it sends its
/// side of the handshake and waits for the client's.
fn answered_client_hello(name: &str) -> Vec<u8> {
    let mut extra = Vec::new();
    // supported_groups: x25519.
    extra.extend(10u16.to_be_bytes());
    extra.extend(4u16.to_be_bytes());
    extra.extend([0, 2, 0, 0x1d]);
    // signature_algorithms: rsa_pss_rsae_sha256, ecdsa_secp256r1_sha256, rsa_pkcs1_sha256.
    extra.extend(13u16.to_be_bytes());
    extra.extend(8u16.to_be_bytes());
    extra.extend([0, 6, 8, 4, 4, 3, 4, 1]);
    // key_share: one x25519 public key, the curve's base point.
    extra.extend(51u16.to_be_bytes());
    extra.extend(38u16.to_be_bytes());
    extra.extend(36u16.to_be_bytes());
    extra.extend(0x1du16.to_be_bytes());
    extra.extend(32u16.to_be_bytes());
    extra.push(9);
    extra.extend([0u8; 31]);
    client_hello_carrying(name, &extra)
}

/// Whether `stream` is still open after a quiet spell, whatever it had left to read: the far
/// side has not hung up on its own, so only the proxy can end it now.
fn stays_open(stream: &std::net::TcpStream) -> bool {
    use std::io::Read;
    let mut stream = stream;
    let _ = stream.set_read_timeout(Some(std::time::Duration::from_secs(2)));
    let mut chunk = [0u8; 4096];
    loop {
        match stream.read(&mut chunk) {
            Ok(0) => return false,
            Ok(_) => continue,
            Err(why) => {
                return why.kind() == std::io::ErrorKind::WouldBlock
                    || why.kind() == std::io::ErrorKind::TimedOut;
            }
        }
    }
}

/// Whether `stream` reaches its end within a few seconds, whatever it still had to read.
fn ends_promptly(mut stream: std::net::TcpStream) -> bool {
    use std::io::Read;
    let begun = std::time::Instant::now();
    let _ = stream.set_read_timeout(Some(std::time::Duration::from_secs(8)));
    let mut chunk = [0u8; 4096];
    loop {
        match stream.read(&mut chunk) {
            Ok(0) => return begun.elapsed() < std::time::Duration::from_secs(5),
            Ok(_) => continue,
            Err(why) => {
                return why.kind() != std::io::ErrorKind::WouldBlock
                    && why.kind() != std::io::ErrorKind::TimedOut;
            }
        }
    }
}

#[test]
fn a_tunnel_carried_during_a_question_ends_with_it_and_when_its_proxy_closes() {
    for (id, close) in [("cut-a", false), ("cut-b", true)] {
        let proxy = proxy_for(id, vec!["example.com".to_owned()]).expect("a proxy");
        let asking = Asking::of(std::sync::Arc::clone(&proxy));
        let Some(tunnel) = open_tunnel(&proxy, "example.com") else {
            // Off CI, where `open_tunnel` holds it to, a machine with no network says nothing here.
            return;
        };
        assert!(
            stays_open(&tunnel),
            "example.com hung up on the tunnel itself, so its end would prove nothing"
        );
        if close {
            // The question is still being answered: only `close_proxy` can end the tunnel here.
            close_proxy(id);
            assert!(
                ends_promptly(tunnel),
                "the tunnel outlived its proxy closing for good, mid-question"
            );
            drop(asking);
        } else {
            drop(asking);
            assert!(ends_promptly(tunnel), "the tunnel outlived its question");
        }
        drop(proxy);
    }
}

// ---------------------------------------------------------------------------------------
// A skill router (SK-15b, #3230; ADR 0102 §5, §6)
// ---------------------------------------------------------------------------------------

#[test]
fn a_router_declares_its_title_and_whether_it_reads_prompts() {
    let reads = |router: serde_json::Value| {
        let text = manifest(
            &["skill-router"],
            serde_json::json!({}),
            serde_json::json!({ "skill-router": router }),
        );
        read(&text, true).map(|manifest| manifest.studio.and_then(|studio| studio.router))
    };
    assert_eq!(
        reads(serde_json::json!({ "title": "Relevance" })),
        Ok(Some(DeclaredRouter {
            title: "Relevance".to_owned(),
            reads_prompts: false,
        }))
    );
    assert_eq!(
        reads(serde_json::json!({ "title": "Relevance", "reads-prompts": true }))
            .map(|router| router.map(|it| it.reads_prompts)),
        Ok(Some(true))
    );
    for bad in [
        serde_json::json!({ "title": "R", "reads-prompts": "yes" }),
        serde_json::json!({ "title": "R", "prompts": true }),
        serde_json::json!("R"),
    ] {
        assert!(reads(bad.clone()).is_err(), "{bad} was taken");
    }
}

#[test]
fn the_approval_says_a_router_that_reads_prompts_reads_every_prompt() {
    let said = |reads_prompts: bool| {
        declares(&Declared {
            router: Some(DeclaredRouter {
                title: "Relevance".to_owned(),
                reads_prompts,
            }),
            ..Declared::default()
        })
        .join("\n")
    };
    let reading = said(true);
    assert!(
        reading.contains("“Relevance”") && reading.contains("reads every prompt"),
        "{reading}"
    );
    assert!(reading.contains("100 ms"), "{reading}");
    let not = said(false);
    assert!(not.contains("never reads a prompt"), "{not}");
}

#[test]
fn a_router_is_asked_the_skills_it_may_hide_the_budget_and_nothing_of_the_chat() {
    let skills = [RouteSkill {
        name: "deploy".to_owned(),
        description: "Deploys.".to_owned(),
        pinned: false,
    }];
    let ask = Ask::Route {
        skills: &skills,
        budget: RouteBudget {
            skills: 10,
            chars: 2000,
        },
        besides: RouteBudget {
            skills: 1,
            chars: 40,
        },
    };
    assert_eq!(ask.capability(), Capability::SkillRouter);
    assert_eq!(
        ask.line(),
        serde_json::json!({
            "ask": "route",
            "skills": [{ "name": "deploy", "description": "Deploys.", "pinned": false }],
            "budget": { "skills": 10, "chars": 2000 },
            "besides": { "skills": 1, "chars": 40 },
        })
    );
}

#[test]
fn a_routers_answer_is_read_failing_closed() {
    assert_eq!(
        routed_of(
            "r",
            2,
            &doc(serde_json::json!({ "hidden": [{ "skill": "deploy", "why": " rarely used " }] }))
        ),
        Ok(vec![(
            "deploy".to_owned(),
            purlis_skills::Line::new("rarely used").expect("a line")
        )])
    );
    assert_eq!(
        routed_of("r", 2, &doc(serde_json::json!({ "hidden": [] }))),
        Ok(Vec::new())
    );
    for bad in [
        serde_json::json!({}),
        serde_json::json!({ "hidden": [{ "skill": "deploy" }] }),
        serde_json::json!({ "hidden": [{ "skill": "deploy", "why": "x", "more": 1 }] }),
        serde_json::json!({ "hidden": [{ "skill": "Not A Name", "why": "x" }] }),
        serde_json::json!({ "hidden": [{ "skill": "deploy", "why": "two\nlines" }] }),
        serde_json::json!({ "hidden": [{ "skill": "deploy", "why": "" }] }),
        serde_json::json!({ "hidden": [], "shown": [] }),
    ] {
        assert!(
            routed_of("r", 2, &doc(bad.clone())).is_err(),
            "{bad} was taken"
        );
    }
    let too_many = serde_json::json!({ "hidden": [
        { "skill": "a", "why": "x" }, { "skill": "b", "why": "x" }, { "skill": "c", "why": "x" }
    ] });
    let refused = routed_of("r", 2, &doc(too_many)).expect_err("more than it was asked");
    assert!(
        refused.contains("more skills than it was asked about"),
        "{refused}"
    );
}

// ---- a lint that answers many skills at once (#4195) ----------------------------------------

#[test]
fn a_lint_says_whether_it_answers_many_skills_at_once() {
    let reads = |lint: serde_json::Value| {
        let text = manifest(
            &["skill-lint"],
            serde_json::json!({}),
            serde_json::json!({ "skill-lint": lint }),
        );
        read(&text, true).map(|manifest| {
            let studio = manifest.studio.expect("a studio extension");
            (studio.lint, studio.lint_many_at_once)
        })
    };
    assert_eq!(
        reads(serde_json::json!({ "title": "House style" })),
        Ok((Some("House style".to_owned()), false)),
        "a lint that says nothing is asked one skill at a time, as before"
    );
    assert_eq!(
        reads(serde_json::json!({ "title": "House style", "many-at-once": true })),
        Ok((Some("House style".to_owned()), true))
    );
    for bad in [
        serde_json::json!({ "title": "H", "many-at-once": "yes" }),
        serde_json::json!({ "title": "H", "batch": true }),
    ] {
        assert!(reads(bad.clone()).is_err(), "{bad} was taken");
    }
}

#[test]
fn a_question_of_many_hands_each_skill_by_name_with_its_files() {
    let skills = [LintedSkill {
        skill: "deploy".to_owned(),
        files: vec![HandedFile {
            path: "SKILL.md".to_owned(),
            text: "---\n".to_owned(),
            executable: false,
        }],
    }];
    let ask = Ask::FindingsOfEach { skills: &skills };
    assert_eq!(
        ask.line(),
        serde_json::json!({
            "ask": "findings-of-each",
            "skills": [{
                "skill": "deploy",
                "files": [{ "path": "SKILL.md", "text": "---\n", "executable": false }],
            }],
        })
    );
    assert_eq!(ask.capability(), Capability::SkillLint);
    let one_at_a_time = Declared {
        lint: Some("House style".to_owned()),
        ..Declared::default()
    };
    let refused = ask.unoffered("x", &one_at_a_time).expect("refused");
    assert!(refused.contains("one skill at a time"), "{refused}");
    let many = Declared {
        lint_many_at_once: true,
        ..one_at_a_time
    };
    assert_eq!(ask.unoffered("x", &many), None);
}

#[test]
fn findings_of_many_are_one_per_skill_asked_in_order_failing_closed() {
    let said = findings_of_each(
        "x",
        &["deploy", "review", "deploy"],
        &doc(serde_json::json!({ "skills": [
            { "skill": "deploy", "findings": [{ "says": " too long " }] },
            { "skill": "review", "findings": [] },
            { "skill": "deploy", "findings": [] },
        ] })),
    )
    .expect("read");
    assert_eq!(
        said,
        [
            Ok(vec!["too long".to_owned()]),
            Ok(Vec::new()),
            Ok(Vec::new())
        ]
    );
    // Refused whole: one left out, one more, another skill, the wrong order, a key no answer
    // has, another shape.
    for bad in [
        serde_json::json!({ "skills": [{ "skill": "deploy", "findings": [] }] }),
        serde_json::json!({ "skills": [
            { "skill": "deploy", "findings": [] },
            { "skill": "review", "findings": [] },
            { "skill": "other", "findings": [] },
        ] }),
        serde_json::json!({ "skills": [
            { "skill": "review", "findings": [] },
            { "skill": "deploy", "findings": [] },
        ] }),
        serde_json::json!({ "skills": [
            { "skill": "deploy", "findings": [], "level": "blocks" },
            { "skill": "review", "findings": [] },
        ] }),
        serde_json::json!({ "findings": [] }),
    ] {
        assert!(
            findings_of_each("x", &["deploy", "review"], &doc(bad.clone())).is_err(),
            "{bad} was taken"
        );
    }
    // Refused for one skill alone: findings no single answer may hold.
    let too_many: Vec<serde_json::Value> = (0..=MOST_FINDINGS)
        .map(|_| serde_json::json!({ "says": "ok" }))
        .collect();
    for bad in [
        serde_json::json!([{ "says": "a\u{202e}b" }]),
        serde_json::json!([{ "says": "" }]),
        serde_json::json!(too_many),
    ] {
        let said = findings_of_each(
            "x",
            &["deploy", "review"],
            &doc(serde_json::json!({ "skills": [
                { "skill": "deploy", "findings": bad },
                { "skill": "review", "findings": [{ "says": "fine" }] },
            ] })),
        )
        .expect("read");
        assert!(said[0].is_err(), "{bad} was taken");
        assert_eq!(said[1], Ok(vec!["fine".to_owned()]));
    }
}
