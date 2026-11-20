// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;
use crate::secrets::{Env, registry};

/// A token's shape, never a real one.
const TOKEN: &str = "fabricated-not-a-gitlab-token";

const SELF_HOSTED: &str = "git.acme.internal";

/// A project whose one clone has a remote on the self-hosted GitLab and one on GitHub, with
/// `extra` appended to its settings.
fn project(extra: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    std::fs::write(
        crate::names::manifest(root),
        format!(
            "schema = 1\n[[forge]]\nkind = \"gitlab\"\nhost = \"{SELF_HOSTED}\"\n\n[sandbox]\n\n\
             [sandbox.personas.dev]\n\n[sandbox.personas.ops]\n{extra}"
        ),
    )
    .unwrap();
    for folder in ["personas", "inventory", "workspaces"] {
        std::fs::create_dir_all(root.join(folder)).unwrap();
    }
    let clone = root.join("workspaces/alpha/svc");
    std::fs::create_dir_all(&clone).unwrap();
    let git = |args: &[&str]| {
        let out = crate::forklock::output(
            std::process::Command::new("git")
                .arg("-C")
                .arg(&clone)
                .args(args)
                .env("GIT_CONFIG_GLOBAL", "/dev/null")
                .env("GIT_CONFIG_SYSTEM", "/dev/null"),
        )
        .expect("git runs");
        assert!(out.status.success(), "git {args:?}");
    };
    git(&["init", "-q", "-b", "main", "."]);
    git(&[
        "remote",
        "add",
        "origin",
        &format!("https://{SELF_HOSTED}/acme/svc.git"),
    ]);
    git(&["remote", "add", "mirror", "https://github.com/acme/svc.git"]);
    dir
}

/// Machine state with a plain-file vault for each of `personas`, tagged for it.
fn machine(personas: &[&str]) -> (tempfile::TempDir, Ctx) {
    let dir = tempfile::tempdir().unwrap();
    let ctx = Ctx::new(dir.path(), Env::of(&[]));
    for persona in personas {
        let file = dir.path().join(format!("{persona}.json"));
        std::fs::write(&file, "{}").unwrap();
        registry::add_vault(
            &ctx,
            persona,
            "plain-file",
            serde_json::json!({ "file": file })
                .as_object()
                .unwrap()
                .clone(),
            Some(persona),
            false,
            false,
        )
        .unwrap();
    }
    (dir, ctx)
}

fn stored(ctx: &Ctx, vault: &str, key: &str) -> Option<String> {
    let vault = cmd::provider(ctx, vault).unwrap();
    cmd::get_value(ctx, &vault, key).ok()
}

fn dev() -> Vec<(String, String)> {
    vec![("dev".to_owned(), "dev".to_owned())]
}

#[test]
fn the_hosts_taken_are_the_gitlab_remotes_of_the_projects_clones() {
    let plane = project("");
    assert_eq!(hosts(plane.path()), [SELF_HOSTED]);
    assert_eq!(
        host_in_project(plane.path(), " Git.Acme.Internal "),
        Ok(SELF_HOSTED.to_owned())
    );
    // GitHub is not a GitLab, and gitlab.com is not one of this project's remotes.
    for refused in ["github.com", "gitlab.com", "git.elsewhere.example"] {
        let said = host_in_project(plane.path(), refused).unwrap_err();
        assert!(
            said.contains("isn't a GitLab host the project's clones are on"),
            "{said}"
        );
        assert!(said.contains(SELF_HOSTED), "{said}");
    }
}

#[test]
fn a_token_is_one_word_of_printable_characters_and_never_in_its_refusal() {
    assert!(token_ok(TOKEN).is_ok());
    for bad in [
        "",
        "two words",
        "line\nbreak",
        "tab\there",
        &"x".repeat(MOST_TOKEN + 1),
    ] {
        let said = token_ok(bad).unwrap_err();
        assert!(!said.contains(bad) || bad.is_empty(), "{said}");
    }
}

#[test]
fn the_lines_shown_are_the_lines_written_and_the_rest_of_the_file_is_kept() {
    let before = "# mine\n[sandbox]\n\n[sandbox.personas.dev]\nhosts = [\"a.example\"]\n";
    let after = with_lines(before, SELF_HOSTED, &dev()).unwrap();
    assert!(after.starts_with("# mine\n"), "{after}");
    assert!(after.contains("hosts = [\"a.example\"]"), "{after}");
    let shown = lines_shown(SELF_HOSTED, &dev());
    assert!(
        after.contains(shown.lines().nth(1).unwrap()),
        "{after}\n{shown}"
    );
    assert!(shown.contains("as = \"GITLAB_TOKEN env\""), "{shown}");
    assert!(
        shown.contains(&format!("host = \"{SELF_HOSTED}\"")),
        "{shown}"
    );
    assert!(
        shown.contains(&format!("key = \"gitlab-token-{SELF_HOSTED}\"")),
        "{shown}"
    );
    // Written again, it is the same file.
    assert_eq!(with_lines(&after, SELF_HOSTED, &dev()).unwrap(), after);
}

#[test]
fn setting_up_replaces_no_glab_line_the_person_wrote() {
    let text = "[sandbox.personas.dev.tools]\nglab = { vault = \"v\", as = \"GITLAB_TOKEN env\", \
                key = \"mine\" }\n";
    let said = with_lines(text, SELF_HOSTED, &dev()).unwrap_err();
    assert!(
        said.contains("already has a glab tool of its own"),
        "{said}"
    );
}

#[test]
fn removing_takes_out_only_the_lines_this_flow_wrote_for_that_host() {
    let text = "[sandbox.personas.dev.tools]\nglab = { vault = \"v\", as = \"GITLAB_TOKEN env\", \
                key = \"mine\", host = \"git.acme.internal\" }\n\n\
                [sandbox.personas.ops]\n";
    let ours = with_lines(text, SELF_HOSTED, &[("ops".into(), "ops".into())]).unwrap();
    let gone = without_lines(&ours, SELF_HOSTED).unwrap();
    assert!(
        gone.contains("key = \"mine\""),
        "a hand-written line stays: {gone}"
    );
    assert!(!gone.contains("gitlab-token-"), "{gone}");
}

#[test]
fn held_reads_each_personas_glab_line_and_says_which_are_ours() {
    let plane = project("");
    let ours = rewrite_manifest(plane.path(), |text| with_lines(text, SELF_HOSTED, &dev()));
    ours.unwrap();
    rewrite_manifest(plane.path(), |text| {
        Ok(format!(
            "{text}\n[sandbox.personas.ops.tools]\nglab = {{ vault = \"v\", as = \"GITLAB_TOKEN env\", \
             key = \"k\" }}\n"
        ))
    })
    .unwrap();
    let found = held(plane.path()).unwrap();
    let by = |persona: &str| found.iter().find(|one| one.persona == persona).unwrap();
    assert!(by("dev").ours(), "{:?}", by("dev"));
    assert_eq!(by("dev").host, SELF_HOSTED);
    assert!(!by("ops").ours());
    assert_eq!(
        by("ops").host,
        "gitlab.com",
        "a glab that names no host reaches gitlab.com"
    );
}

#[cfg(not(feature = "forge-wiring"))]
#[test]
fn setting_up_keeps_the_token_in_the_personas_vault_declares_glab_and_allows_what_it_wrote() {
    let plane = project("");
    let (_machine, ctx) = machine(&["dev"]);
    let mut allowed = Vec::new();
    let done = set_up(
        &ctx,
        plane.path(),
        SELF_HOSTED,
        TOKEN,
        &["dev".to_owned()],
        Some(&mut |shown| {
            allowed.push(shown.clone());
            Ok(())
        }),
    )
    .unwrap();
    assert_eq!(done.left, Vec::<String>::new());
    assert_eq!(
        stored(&ctx, "dev", &key_of(SELF_HOSTED)).as_deref(),
        Some(TOKEN)
    );
    // The line is the one a chat's shim is made from: glab, on the clone's host, handed
    // GITLAB_TOKEN from the persona's own vault.
    let [shown] = allowed.as_slice() else {
        panic!("{allowed:?}")
    };
    let [tool] = shown.tools.as_slice() else {
        panic!("{:?}", shown.tools)
    };
    assert_eq!(tool.name, "glab");
    assert_eq!(tool.host.as_deref(), Some(SELF_HOSTED));
    assert_eq!(tool.vault, "dev");
    assert_eq!(tool.values[0].var, "GITLAB_TOKEN");
    assert_eq!(tool.values[0].key, key_of(SELF_HOSTED));
    // And the file says nothing of the token.
    let file = std::fs::read_to_string(crate::names::manifest(plane.path())).unwrap();
    assert!(!file.contains(TOKEN), "{file}");
}

#[test]
fn nothing_is_stored_where_something_would_refuse() {
    let plane = project("");
    let before = std::fs::read_to_string(crate::names::manifest(plane.path())).unwrap();
    let (_machine, ctx) = machine(&["dev"]);
    let personas = ["dev".to_owned()];
    let mut never = |_: &crate::sandbox::persona::Shown| -> Result<(), String> {
        panic!("allowed with nothing set up")
    };
    // A host that is not the clone's, a token that is not one, a persona with no vault.
    for (host, token, who) in [
        ("gitlab.com", TOKEN, "dev"),
        (SELF_HOSTED, "two words", "dev"),
        (SELF_HOSTED, TOKEN, "ops"),
    ] {
        let said = set_up(
            &ctx,
            plane.path(),
            host,
            token,
            &[who.to_owned()],
            Some(&mut never),
        )
        .unwrap_err();
        assert!(!said.contains(TOKEN), "{said}");
    }
    assert!(
        set_up(
            &ctx,
            plane.path(),
            SELF_HOSTED,
            TOKEN,
            &[],
            Some(&mut never)
        )
        .is_err()
    );
    assert_eq!(stored(&ctx, "dev", &key_of(SELF_HOSTED)), None);
    // A persona with a glab of its own: refused before the token is stored.
    rewrite_manifest(plane.path(), |text| {
        Ok(format!(
            "{text}\n[sandbox.personas.dev.tools]\nglab = {{ vault = \"v\", as = \"GITLAB_TOKEN \
             env\", key = \"mine\" }}\n"
        ))
    })
    .unwrap();
    let said = set_up(
        &ctx,
        plane.path(),
        SELF_HOSTED,
        TOKEN,
        &personas,
        Some(&mut never),
    )
    .unwrap_err();
    assert!(said.contains("glab tool of its own"), "{said}");
    assert_eq!(stored(&ctx, "dev", &key_of(SELF_HOSTED)), None);
    assert!(
        !std::fs::read_to_string(crate::names::manifest(plane.path()))
            .unwrap()
            .contains("gitlab-token-")
    );
    let _ = before;
}

#[test]
fn removing_takes_the_token_and_the_line_away_and_refuses_what_it_did_not_write() {
    let plane = project("");
    let (_machine, ctx) = machine(&["dev"]);
    set_up(
        &ctx,
        plane.path(),
        SELF_HOSTED,
        TOKEN,
        &["dev".to_owned()],
        Some(&mut |_| Ok(())),
    )
    .unwrap();
    remove(&ctx, plane.path(), SELF_HOSTED).unwrap();
    assert_eq!(stored(&ctx, "dev", &key_of(SELF_HOSTED)), None);
    assert!(held(plane.path()).unwrap().is_empty());
    // Nothing of ours is left, so a second press is refused and changes nothing.
    assert!(
        remove(&ctx, plane.path(), SELF_HOSTED)
            .unwrap_err()
            .contains("didn't write")
    );
}

#[test]
fn with_no_allow_to_record_every_persona_is_left_for_the_persons_own() {
    let plane = project("");
    let (_machine, ctx) = machine(&["dev"]);
    let done = set_up(
        &ctx,
        plane.path(),
        SELF_HOSTED,
        TOKEN,
        &["dev".to_owned()],
        None,
    )
    .unwrap();
    assert_eq!(done.left, ["dev"]);
    assert_eq!(
        stored(&ctx, "dev", &key_of(SELF_HOSTED)).as_deref(),
        Some(TOKEN)
    );
    assert!(held(plane.path()).unwrap()[0].ours());
}
