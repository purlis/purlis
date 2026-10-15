// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;
use crate::profilestore::list;
use std::collections::BTreeMap;
use std::fs;

/// A machine: a home, its harness settings folders named by variables, and a data home.
struct Machine {
    _dir: tempfile::TempDir,
    home: PathBuf,
    env: BTreeMap<String, String>,
    store: Homes,
}

impl Machine {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().join("home");
        fs::create_dir_all(&home).unwrap();
        let store = Homes::at(&dir.path().join("config-home"), &dir.path().join("data"));
        Self {
            home,
            env: BTreeMap::new(),
            store,
            _dir: dir,
        }
    }

    /// The folder `variable` names, made.
    fn names(&mut self, variable: &str, folder: &str) -> PathBuf {
        let path = self.home.join(folder);
        fs::create_dir_all(&path).unwrap();
        self.env
            .insert(variable.to_owned(), path.display().to_string());
        path
    }

    fn create(&self, name: &str, kind: &str, now: u64) -> Result<Made, String> {
        let env = |name: &str| self.env.get(name).cloned();
        create(
            &self.store,
            name,
            kind,
            &MachineSettings {
                home: &self.home,
                env: &env,
            },
            now,
        )
    }

    fn import(&self, name: &str, now: u64) -> Result<Made, String> {
        let env = |name: &str| self.env.get(name).cloned();
        import(
            &self.store,
            name,
            &MachineSettings {
                home: &self.home,
                env: &env,
            },
            now,
        )
    }
}

fn write(path: &Path, text: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

/// The store's profiles.
fn listed(store: &Homes) -> Vec<MachineProfile> {
    list(store.config_root()).unwrap()
}

/// Every file under `dir`, as paths under it.
fn files(dir: &Path) -> Vec<String> {
    fn walk(base: &Path, dir: &Path, out: &mut Vec<String>) {
        for one in fs::read_dir(dir).unwrap().filter_map(Result::ok) {
            let path = one.path();
            if path.is_dir() {
                walk(base, &path, out);
            } else {
                out.push(path.strip_prefix(base).unwrap().display().to_string());
            }
        }
    }
    let mut out = Vec::new();
    if dir.is_dir() {
        walk(dir, dir, &mut out);
    }
    out.sort();
    out
}

/// A Claude Code settings folder holding settings, and beside them files shaped like a
/// sign-in, a conversation and a history. Their contents are plain words.
fn claude_settings(at: &Path) {
    write(&at.join("settings.json"), "{\"theme\": \"dark\"}\n");
    write(&at.join("CLAUDE.md"), "Be brief.\n");
    write(&at.join(".credentials.json"), "not imported\n");
    write(&at.join(".claude.json"), "not imported\n");
    write(&at.join("credentials.json"), "not imported\n");
    write(&at.join("history.jsonl"), "not imported\n");
    write(&at.join("projects/-work/one.jsonl"), "not imported\n");
    write(&at.join("agents/reviewer.md"), "Reviews.\n");
    write(&at.join("agents/.credentials.json"), "not imported\n");
    write(&at.join("agents/auth.json"), "not imported\n");
    write(&at.join("agents/api_key.txt"), "not imported\n");
    write(&at.join("skills/notes/SKILL.md"), "Notes.\n");
    write(&at.join("skills/notes/.hidden.md"), "not imported\n");
}

#[test]
fn an_import_copies_exactly_the_listed_settings_and_no_credential_file() {
    let mut machine = Machine::new();
    let source = machine.names("CLAUDE_CONFIG_DIR", "claude-settings");
    claude_settings(&source);

    let made = machine.create("work", "claude", 100).unwrap();

    assert_eq!(
        files(&made.record.folders.config),
        [
            "CLAUDE.md",
            "agents/reviewer.md",
            "settings.json",
            "skills/notes/SKILL.md",
        ]
    );
    assert_eq!(
        fs::read_to_string(made.record.folders.config.join("settings.json")).unwrap(),
        "{\"theme\": \"dark\"}\n"
    );
    assert_eq!(
        made.record.imported,
        Some(Imported {
            at: 100,
            from: source
        })
    );
    assert!(
        made.copied
            .skipped
            .iter()
            .any(|s| s.starts_with("agents/auth.json")),
        "{:?}",
        made.copied.skipped
    );
}

#[test]
fn a_new_profile_is_kept_in_a_private_folder_of_its_own_under_the_store() {
    let machine = Machine::new();

    let made = machine.create("work", "claude", 100).unwrap();

    let folder = machine.store.folder_of("work");
    assert_eq!(made.record.folders.config, folder.join(CONFIG));
    assert!(made.record.adopted.is_none());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        for dir in [
            machine.store.dir(),
            folder.as_path(),
            &made.record.folders.config,
        ] {
            let mode = fs::metadata(dir).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o700, "{}", dir.display());
        }
    }
    assert_eq!(listed(&machine.store), [made.record]);
}

#[test]
fn an_imported_file_is_private() {
    let mut machine = Machine::new();
    let source = machine.names("CLAUDE_CONFIG_DIR", "claude-settings");
    claude_settings(&source);

    let made = machine.create("work", "claude", 100).unwrap();

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let file = made.record.folders.config.join("settings.json");
        assert_eq!(
            fs::metadata(file).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
}

#[cfg(unix)]
#[test]
fn an_import_follows_no_link_out_of_the_settings_folder() {
    let mut machine = Machine::new();
    let source = machine.names("CLAUDE_CONFIG_DIR", "claude-settings");
    let outside = machine.home.join("elsewhere.json");
    write(&outside, "elsewhere\n");
    std::os::unix::fs::symlink(&outside, source.join("settings.json")).unwrap();
    write(&source.join("CLAUDE.md"), "Be brief.\n");
    std::os::unix::fs::symlink(machine.home.join("."), source.join("commands")).unwrap();

    let made = machine.create("work", "claude", 100).unwrap();

    assert_eq!(files(&made.record.folders.config), ["CLAUDE.md"]);
    for name in ["settings.json", "commands"] {
        assert!(
            made.copied
                .skipped
                .iter()
                .any(|s| s.starts_with(&format!("{name}: a link"))),
            "{name}: {:?}",
            made.copied.skipped
        );
    }
}

#[cfg(unix)]
#[test]
fn an_import_copies_no_sign_in_through_a_link_inside_the_settings_folder() {
    let mut machine = Machine::new();
    let source = machine.names("CLAUDE_CONFIG_DIR", "claude-settings");
    write(&source.join(".credentials.json"), "not imported\n");
    write(&source.join("auth.json"), "not imported\n");
    std::os::unix::fs::symlink(".credentials.json", source.join("settings.json")).unwrap();
    fs::create_dir_all(source.join("skills")).unwrap();
    std::os::unix::fs::symlink("../auth.json", source.join("skills/notes.md")).unwrap();
    write(&machine.home.join("elsewhere/id_ed25519"), "not imported\n");
    std::os::unix::fs::symlink(machine.home.join("elsewhere"), source.join("skills/keys")).unwrap();
    write(&source.join("CLAUDE.md"), "Be brief.\n");

    let made = machine.create("work", "claude", 100).unwrap();

    assert_eq!(files(&made.record.folders.config), ["CLAUDE.md"]);
}

#[cfg(unix)]
#[test]
fn an_import_copies_no_file_that_has_another_name_on_the_disk() {
    let mut machine = Machine::new();
    let source = machine.names("CLAUDE_CONFIG_DIR", "claude-settings");
    write(&source.join(".credentials.json"), "not imported\n");
    fs::create_dir_all(source.join("skills")).unwrap();
    fs::hard_link(
        source.join(".credentials.json"),
        source.join("skills/notes.md"),
    )
    .unwrap();
    fs::hard_link(source.join(".credentials.json"), source.join("CLAUDE.md")).unwrap();
    write(&source.join("settings.json"), "{}\n");

    let made = machine.create("work", "claude", 100).unwrap();

    assert_eq!(files(&made.record.folders.config), ["settings.json"]);
    assert!(
        made.copied
            .skipped
            .iter()
            .any(|s| s.starts_with("CLAUDE.md: a file with another name")),
        "{:?}",
        made.copied.skipped
    );
}

#[test]
fn a_codex_import_copies_its_settings_and_never_its_sign_in() {
    let mut machine = Machine::new();
    let source = machine.names("CODEX_HOME", "codex-home");
    write(&source.join("config.toml"), "model = \"o9\"\n");
    write(&source.join("AGENTS.md"), "Be brief.\n");
    write(&source.join("prompts/review.md"), "Review.\n");
    write(&source.join("auth.json"), "not imported\n");
    write(&source.join("history.jsonl"), "not imported\n");
    write(
        &source.join("sessions/2026/10/14/rollout-1-abc.jsonl"),
        "not imported\n",
    );

    let made = machine.create("work", "codex", 100).unwrap();

    assert_eq!(
        files(&made.record.folders.config),
        ["AGENTS.md", "config.toml", "prompts/review.md"]
    );
    assert_eq!(made.record.folders.data, None);
}

#[test]
fn an_opencode_profile_has_a_data_folder_and_its_settings_under_its_own_name() {
    let mut machine = Machine::new();
    let config = machine.names("XDG_CONFIG_HOME", "xdg-config");
    let data = machine.names("XDG_DATA_HOME", "xdg-data");
    write(&config.join("opencode/tui.json"), "{}\n");
    write(&config.join("opencode/AGENTS.md"), "Be brief.\n");
    write(&config.join("opencode/agent/plan.md"), "Plans.\n");
    write(&config.join("opencode/auth.json"), "not imported\n");
    write(&data.join("opencode/auth.json"), "not imported\n");

    let made = machine.create("work", "opencode", 100).unwrap();

    assert_eq!(
        files(&made.record.folders.config),
        [
            "opencode/AGENTS.md",
            "opencode/agent/plan.md",
            "opencode/tui.json"
        ]
    );
    let folder = machine.store.folder_of("work");
    assert_eq!(made.record.folders.data, Some(folder.join(DATA)));
    assert_eq!(files(&folder.join(DATA)), Vec::<String>::new());
}

#[test]
fn a_machine_with_no_settings_makes_a_profile_with_none() {
    let machine = Machine::new();

    let made = machine.create("work", "claude", 100).unwrap();

    assert_eq!(files(&made.record.folders.config), Vec::<String>::new());
    assert_eq!(made.copied, Copied::default());
}

#[test]
fn a_re_import_takes_the_machines_settings_again_and_keeps_the_conversations() {
    let mut machine = Machine::new();
    let source = machine.names("CLAUDE_CONFIG_DIR", "claude-settings");
    claude_settings(&source);
    let made = machine.create("work", "claude", 100).unwrap();
    let conversation = made.record.folders.config.join("projects/-work/kept.jsonl");
    write(&conversation, "kept\n");
    write(&source.join("settings.json"), "{\"theme\": \"light\"}\n");
    fs::remove_file(source.join("agents/reviewer.md")).unwrap();

    let again = machine.import("work", 200).unwrap();

    assert_eq!(again.record.imported.as_ref().unwrap().at, 200);
    assert_eq!(
        fs::read_to_string(made.record.folders.config.join("settings.json")).unwrap(),
        "{\"theme\": \"light\"}\n"
    );
    assert!(
        !made
            .record
            .folders
            .config
            .join("agents/reviewer.md")
            .exists()
    );
    assert_eq!(fs::read_to_string(&conversation).unwrap(), "kept\n");
    assert_eq!(
        listed(&machine.store)
            .into_iter()
            .find(|p| p.name == "work"),
        Some(again.record)
    );
}

#[test]
fn a_name_purlis_already_gives_or_cannot_hold_makes_no_profile() {
    let machine = Machine::new();

    for (name, kind) in [
        ("claude", "claude"),
        ("a b", "claude"),
        ("-x", "claude"),
        ("work", "aider"),
    ] {
        assert!(machine.create(name, kind, 1).is_err(), "{name} {kind}");
    }
    machine.create("work", "claude", 1).unwrap();
    let again = machine.create("work", "codex", 2).unwrap_err();
    assert!(
        again.contains("already has a profile named 'work'"),
        "{again}"
    );
    assert_eq!(listed(&machine.store).len(), 1);
}

#[test]
fn removing_a_profile_names_its_conversations_then_deletes_its_folder() {
    let machine = Machine::new();
    let made = machine.create("work", "claude", 100).unwrap();
    write(
        &made.record.folders.config.join("projects/-a-b/one.jsonl"),
        "one\n",
    );
    write(
        &made.record.folders.config.join("projects/-c/two.jsonl"),
        "two\n",
    );

    let removal = removal(&machine.store, "work").unwrap();

    assert_eq!(
        removal.conversations,
        Some(vec![
            "config/projects/-a-b/one.jsonl".to_owned(),
            "config/projects/-c/two.jsonl".to_owned(),
        ])
    );
    assert!(!removal.adopted);
    assert!(
        removal.says().contains("its 2 conversations"),
        "{}",
        removal.says()
    );
    let folder = machine.store.folder_of("work");
    assert!(folder.is_dir(), "nothing goes before it is confirmed");

    remove(&machine.store, "work", &removal).unwrap();

    assert!(!folder.exists());
    assert_eq!(listed(&machine.store), []);
}

#[test]
fn a_removal_that_changed_since_it_was_shown_removes_nothing() {
    let machine = Machine::new();
    let made = machine.create("work", "claude", 100).unwrap();
    let shown = removal(&machine.store, "work").unwrap();
    write(
        &made.record.folders.config.join("projects/-a/new.jsonl"),
        "new\n",
    );

    let refused = remove(&machine.store, "work", &shown).unwrap_err();

    assert!(refused.contains("nothing was"), "{refused}");
    assert!(machine.store.folder_of("work").is_dir());
    assert_eq!(listed(&machine.store).len(), 1);
}

/// Writes the store's records as `json`, for a record purlis would not have made.
fn records(store: &Homes, json: &str) {
    write(&crate::profilestore::path(store.config_root()), json);
}

/// One record as the file holds it: a made one, or one adopted in place as #2033 writes it.
fn record_json(name: &str, config: &Path, adopted: bool) -> String {
    let adopted = if adopted {
        ", \"adopted\": {\"at\": 5, \"variables\": []}"
    } else {
        ""
    };
    format!(
        "{{\"name\": {name:?}, \"harness\": \"claude\", \"title\": \"Claude Code (this Mac)\", \
         \"folders\": {{\"config\": {:?}}}{adopted}}}",
        config.display().to_string()
    )
}

#[test]
fn removing_never_deletes_a_folder_outside_the_store() {
    let machine = Machine::new();
    let outside = machine.home.join("someone-elses");
    write(&outside.join("config/settings.json"), "{}\n");
    records(
        &machine.store,
        &format!(
            "{{\"profiles\": [{}]}}",
            record_json("work", &outside.join("config"), false)
        ),
    );

    let refused = removal(&machine.store, "work").unwrap_err();

    assert!(
        refused.contains("outside purlis's profiles folder"),
        "{refused}"
    );
    let shown = Removal {
        name: "work".into(),
        adopted: false,
        folder: outside.clone(),
        conversations: Some(Vec::new()),
    };
    assert!(remove(&machine.store, "work", &shown).is_err());
    assert!(outside.join("config/settings.json").is_file());
}

#[cfg(unix)]
#[test]
fn removing_never_deletes_through_a_link_in_the_store() {
    let machine = Machine::new();
    let outside = machine.home.join("someone-elses");
    write(&outside.join("config/settings.json"), "{}\n");
    fs::create_dir_all(machine.store.dir()).unwrap();
    std::os::unix::fs::symlink(&outside, machine.store.folder_of("work")).unwrap();
    records(
        &machine.store,
        &format!(
            "{{\"profiles\": [{}]}}",
            record_json(
                "work",
                &machine.store.folder_of("work").join("config"),
                false
            )
        ),
    );

    assert!(removal(&machine.store, "work").is_err());
    assert!(outside.join("config/settings.json").is_file());
}

#[test]
fn removing_an_adopted_profile_forgets_it_and_keeps_its_folder() {
    let machine = Machine::new();
    let own = machine.home.join(".claude");
    write(&own.join("projects/-a/one.jsonl"), "one\n");
    records(
        &machine.store,
        &format!("{{\"profiles\": [{}]}}", record_json("claude", &own, true)),
    );

    let shown = removal(&machine.store, "claude").unwrap();

    assert!(shown.adopted);
    assert!(shown.says().contains("stays"), "{}", shown.says());
    remove(&machine.store, "claude", &shown).unwrap();
    assert!(own.join("projects/-a/one.jsonl").is_file());
    assert_eq!(listed(&machine.store), []);
    let again =
        crate::profilestore::adopt(machine.store.config_root(), &machine.home, &|_: &str| None)
            .unwrap();
    assert_eq!(
        again.adopted,
        [],
        "a folder removed as a profile is not adopted again"
    );
}

#[test]
fn an_adopted_profile_is_not_imported_into() {
    let machine = Machine::new();
    let own = machine.home.join(".claude");
    records(
        &machine.store,
        &format!("{{\"profiles\": [{}]}}", record_json("claude", &own, true)),
    );

    assert!(machine.import("claude", 2).is_err());
}

#[test]
fn an_adopted_record_a_record_this_purlis_cannot_read_and_a_later_key_are_kept() {
    let machine = Machine::new();
    records(
        &machine.store,
        &format!(
            "{{\"version\": 1, \"profiles\": [{}, {{\"name\": \"odd\", \"shape\": 3}}], \
             \"later\": true}}",
            record_json("claude", Path::new("/x"), true)
        ),
    );

    machine.create("work", "claude", 1).unwrap();

    let text = fs::read_to_string(crate::profilestore::path(machine.store.config_root())).unwrap();
    assert!(text.contains("\"variables\": []"), "{text}");
    assert!(text.contains("\"shape\": 3"), "{text}");
    assert!(text.contains("\"later\": true"), "{text}");
    let names: Vec<String> = listed(&machine.store).into_iter().map(|r| r.name).collect();
    assert_eq!(names, ["claude", "work"]);
}

#[test]
fn a_wildcard_matches_a_conversations_file_name() {
    assert!(wildcard("{id}.jsonl", "abc.jsonl"));
    assert!(wildcard("rollout-*-{id}.jsonl", "rollout-2026-abc.jsonl"));
    assert!(!wildcard("rollout-*-{id}.jsonl", "history.jsonl"));
    assert!(wildcard("*", "2026"));
    assert!(!wildcard("{id}.jsonl", "é"));
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[test]
fn an_import_reads_nothing_through_a_folder_swapped_for_a_link_while_it_runs() {
    let mut machine = Machine::new();
    let source = machine.names("CLAUDE_CONFIG_DIR", "claude-settings");
    write(&source.join("skills/notes.md"), "Notes.\n");
    write(&machine.home.join("elsewhere/id_ed25519"), "not imported\n");
    machine.create("work", "claude", 1).unwrap();
    let config = machine.store.folder_of("work").join(CONFIG);
    let root = source.canonicalize().unwrap();

    crate::held::racer::while_swapped(
        &root,
        "skills",
        "../elsewhere",
        || machine.import("work", 2),
        |_| {
            assert!(
                !config.join("skills/id_ed25519").exists(),
                "a file behind a link was copied"
            );
        },
    );
}

#[test]
fn a_sandbox_only_profile_is_made_with_a_folder_of_its_own_and_nothing_imported() {
    // #2037, ruled 2026-10-15: a sandboxed Codex chat runs on a profile purlis makes and uses
    // only inside the sandbox. It is signed in through Codex's own sign-in, never from a copy.
    let mut machine = Machine::new();
    let codex = machine.names("CODEX_HOME", "codex-home");
    write(&codex.join("auth.json"), "the person's own sign-in\n");
    write(&codex.join("config.toml"), "model = \"m\"\n");

    let made = create_sandbox_only(&machine.store, "codex-sandbox", "codex", 100).unwrap();

    assert!(made.record.sandbox_only);
    assert_eq!(made.record.imported, None);
    assert!(made.record.folders.config.is_dir());
    assert_eq!(files(&made.record.folders.config), Vec::<String>::new());
    assert_eq!(listed(&machine.store), std::slice::from_ref(&made.record));
    // The person's own folder is as it was.
    assert_eq!(
        fs::read_to_string(codex.join("auth.json")).unwrap(),
        "the person's own sign-in\n"
    );
    // Its settings are purlis's: there is nothing to import into it.
    let refused = machine.import("codex-sandbox", 200).unwrap_err();
    assert!(refused.contains("sandboxed chats only"), "{refused}");
}

#[test]
fn a_profile_made_for_every_chat_is_not_sandbox_only_and_says_nothing_of_it_on_disk() {
    let machine = Machine::new();
    let made = machine.create("work", "claude", 100).unwrap();
    assert!(!made.record.sandbox_only);
    let text = fs::read_to_string(crate::profilestore::path(machine.store.config_root())).unwrap();
    assert!(!text.contains("sandbox_only"), "{text}");
}

#[test]
fn a_sandbox_only_profiles_home_is_its_own_config_folder_and_nothing_else() {
    // #2037 review: what a wrapped Codex is pointed at is the folder purlis made for that
    // profile, by its name, as the kernel names it.
    let machine = Machine::new();
    let made = create_sandbox_only(&machine.store, "codex-sandbox", "codex", 100).unwrap();
    let other = create_sandbox_only(&machine.store, "other", "codex", 100).unwrap();
    let profiles = machine.store.dir();
    let own = made.record.folders.config.clone();

    assert_eq!(
        sandbox_home(profiles, "codex-sandbox", &own),
        Some(own.canonicalize().unwrap())
    );
    // Another profile's folder, under this profile's name.
    assert_eq!(
        sandbox_home(profiles, "codex-sandbox", &other.record.folders.config),
        None
    );
    // A path that climbs out with `..` and back in.
    let climbing = own.join("../../codex-sandbox/config");
    assert_eq!(sandbox_home(profiles, "codex-sandbox", &climbing), None);
    // A name that is no profile's.
    assert_eq!(sandbox_home(profiles, "../x", &own), None);
    // A link in place of the profile's folder, to the person's own Codex home.
    // The person's own Codex home, under another name here: a sandbox keeps tests from making
    // a `.codex` folder.
    let person = machine.home.join("person-codex");
    fs::create_dir_all(&person).unwrap();
    let linked = profiles.join("linked");
    std::os::unix::fs::symlink(&person, &linked).unwrap();
    assert_eq!(
        sandbox_home(profiles, "linked", &linked.join("config")),
        None
    );
    fs::create_dir_all(person.join("config")).unwrap();
    assert_eq!(
        sandbox_home(profiles, "linked", &linked.join("config")),
        None
    );
    // And a link in place of the config folder itself.
    let inner = profiles.join("inner");
    fs::create_dir_all(&inner).unwrap();
    std::os::unix::fs::symlink(&person, inner.join("config")).unwrap();
    assert_eq!(sandbox_home(profiles, "inner", &inner.join("config")), None);
}

#[test]
fn a_sandbox_only_profiles_folder_another_user_owns_is_no_home() {
    // #2037 review: owned by this user, or nothing.
    let machine = Machine::new();
    let made = create_sandbox_only(&machine.store, "codex-sandbox", "codex", 100).unwrap();
    let own = made.record.folders.config.clone();
    let me = rustix::process::getuid().as_raw();
    let profiles = machine.store.dir();
    assert!(sandbox_home_of(profiles, "codex-sandbox", &own, Some(me)).is_some());
    assert_eq!(
        sandbox_home_of(profiles, "codex-sandbox", &own, Some(me.wrapping_add(1))),
        None
    );
}
