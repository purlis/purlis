// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;
use std::collections::BTreeMap;
use std::fs;

/// A machine: a home, its harness settings folders named by variables, and a data home.
struct Machine {
    _dir: tempfile::TempDir,
    home: PathBuf,
    env: BTreeMap<String, String>,
    store: Store,
}

impl Machine {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().join("home");
        fs::create_dir_all(&home).unwrap();
        let store = Store::in_data(&dir.path().join("data"));
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
        files(&made.record.config),
        [
            "CLAUDE.md",
            "agents/reviewer.md",
            "settings.json",
            "skills/notes/SKILL.md",
        ]
    );
    assert_eq!(
        fs::read_to_string(made.record.config.join("settings.json")).unwrap(),
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
    assert_eq!(made.record.config, folder.join(CONFIG));
    assert_eq!(made.record.origin, Origin::Made);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        for dir in [machine.store.dir(), folder.as_path(), &made.record.config] {
            let mode = fs::metadata(dir).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o700, "{}", dir.display());
        }
    }
    assert_eq!(machine.store.records().unwrap(), [made.record]);
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
        let file = made.record.config.join("settings.json");
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
    fs::create_dir_all(source.join("agents")).unwrap();
    std::os::unix::fs::symlink(source.join("CLAUDE.md"), source.join("agents/brief.md")).unwrap();
    std::os::unix::fs::symlink(machine.home.join("."), source.join("commands")).unwrap();

    let made = machine.create("work", "claude", 100).unwrap();

    assert_eq!(files(&made.record.config), ["CLAUDE.md", "agents/brief.md"]);
    assert!(
        made.copied
            .skipped
            .iter()
            .any(|s| s.starts_with("settings.json: a link out of")),
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
        files(&made.record.config),
        ["AGENTS.md", "config.toml", "prompts/review.md"]
    );
    assert_eq!(made.record.data, None);
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
        files(&made.record.config),
        [
            "opencode/AGENTS.md",
            "opencode/agent/plan.md",
            "opencode/tui.json"
        ]
    );
    let folder = machine.store.folder_of("work");
    assert_eq!(made.record.data, Some(folder.join(DATA)));
    assert_eq!(files(&folder.join(DATA)), Vec::<String>::new());
}

#[test]
fn a_machine_with_no_settings_makes_a_profile_with_none() {
    let machine = Machine::new();

    let made = machine.create("work", "claude", 100).unwrap();

    assert_eq!(files(&made.record.config), Vec::<String>::new());
    assert_eq!(made.copied, Copied::default());
}

#[test]
fn a_re_import_takes_the_machines_settings_again_and_keeps_the_conversations() {
    let mut machine = Machine::new();
    let source = machine.names("CLAUDE_CONFIG_DIR", "claude-settings");
    claude_settings(&source);
    let made = machine.create("work", "claude", 100).unwrap();
    let conversation = made.record.config.join("projects/-work/kept.jsonl");
    write(&conversation, "kept\n");
    write(&source.join("settings.json"), "{\"theme\": \"light\"}\n");
    fs::remove_file(source.join("agents/reviewer.md")).unwrap();

    let again = machine.import("work", 200).unwrap();

    assert_eq!(again.record.imported.as_ref().unwrap().at, 200);
    assert_eq!(
        fs::read_to_string(made.record.config.join("settings.json")).unwrap(),
        "{\"theme\": \"light\"}\n"
    );
    assert!(!made.record.config.join("agents/reviewer.md").exists());
    assert_eq!(fs::read_to_string(&conversation).unwrap(), "kept\n");
    assert_eq!(machine.store.record("work").unwrap(), Some(again.record));
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
    assert_eq!(machine.store.records().unwrap().len(), 1);
}

#[test]
fn removing_a_profile_names_its_conversations_then_deletes_its_folder() {
    let machine = Machine::new();
    let made = machine.create("work", "claude", 100).unwrap();
    write(&made.record.config.join("projects/-a-b/one.jsonl"), "one\n");
    write(&made.record.config.join("projects/-c/two.jsonl"), "two\n");

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
    assert_eq!(machine.store.records().unwrap(), []);
}

#[test]
fn a_removal_that_changed_since_it_was_shown_removes_nothing() {
    let machine = Machine::new();
    let made = machine.create("work", "claude", 100).unwrap();
    let shown = removal(&machine.store, "work").unwrap();
    write(&made.record.config.join("projects/-a/new.jsonl"), "new\n");

    let refused = remove(&machine.store, "work", &shown).unwrap_err();

    assert!(refused.contains("nothing was"), "{refused}");
    assert!(machine.store.folder_of("work").is_dir());
    assert_eq!(machine.store.records().unwrap().len(), 1);
}

/// Writes the store's records as `json`, for a record purlis would not have made.
fn records(store: &Store, json: &str) {
    write(&store.dir().join(RECORDS), json);
}

#[test]
fn removing_never_deletes_a_folder_outside_the_store() {
    let machine = Machine::new();
    let outside = machine.home.join("someone-elses");
    write(&outside.join("config/settings.json"), "{}\n");
    records(
        &machine.store,
        &format!(
            "{{\"profiles\": [{{\"name\": \"work\", \"kind\": \"claude\", \"origin\": \"made\", \
             \"config\": {:?}}}]}}",
            outside.join("config").display().to_string()
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
            "{{\"profiles\": [{{\"name\": \"work\", \"kind\": \"claude\", \"origin\": \"made\", \
             \"config\": {:?}}}]}}",
            machine
                .store
                .folder_of("work")
                .join("config")
                .display()
                .to_string()
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
        &format!(
            "{{\"profiles\": [{{\"name\": \"claude-this-mac\", \"kind\": \"claude\", \
             \"origin\": \"adopted\", \"config\": {:?}, \"adopted_at\": 5}}]}}",
            own.display().to_string()
        ),
    );

    let shown = removal(&machine.store, "claude-this-mac").unwrap();

    assert!(shown.adopted);
    assert!(shown.says().contains("stays"), "{}", shown.says());
    remove(&machine.store, "claude-this-mac", &shown).unwrap();
    assert!(own.join("projects/-a/one.jsonl").is_file());
    assert_eq!(machine.store.records().unwrap(), []);
}

#[test]
fn an_adopted_profile_is_not_imported_into() {
    let machine = Machine::new();
    let own = machine.home.join(".claude");
    records(
        &machine.store,
        &format!(
            "{{\"profiles\": [{{\"name\": \"mine\", \"kind\": \"claude\", \"origin\": \
             \"adopted\", \"config\": {:?}}}]}}",
            own.display().to_string()
        ),
    );

    assert!(machine.import("mine", 2).is_err());
}

#[test]
fn a_key_a_later_purlis_writes_is_kept_when_the_records_change() {
    let machine = Machine::new();
    records(
        &machine.store,
        "{\"profiles\": [{\"name\": \"old\", \"kind\": \"claude\", \"origin\": \"adopted\", \
         \"config\": \"/x\", \"adopted_at\": 5}], \"version\": 1}",
    );

    machine.create("work", "claude", 1).unwrap();

    let text = fs::read_to_string(machine.store.dir().join(RECORDS)).unwrap();
    assert!(text.contains("\"adopted_at\": 5"), "{text}");
    assert!(text.contains("\"version\": 1"), "{text}");
}

#[test]
fn a_wildcard_matches_a_conversations_file_name() {
    assert!(wildcard("{id}.jsonl", "abc.jsonl"));
    assert!(wildcard("rollout-*-{id}.jsonl", "rollout-2026-abc.jsonl"));
    assert!(!wildcard("rollout-*-{id}.jsonl", "history.jsonl"));
    assert!(wildcard("*", "2026"));
    assert!(!wildcard("{id}.jsonl", "é"));
}
