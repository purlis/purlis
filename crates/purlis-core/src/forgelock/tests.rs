// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use std::path::PathBuf;

use super::*;

#[test]
fn a_chat_is_claimed_where_it_holds_the_credentials_class_or_has_a_mint() {
    assert!(claimed(true, false));
    assert!(claimed(false, true));
    assert!(claimed(true, true));
    assert!(!claimed(false, false));
}

#[test]
fn git_s_ssh_is_told_no_agent_and_keeps_a_route_it_was_given() {
    let none = ssh_command(None);
    assert!(none.contains("-F /dev/null"), "{none}");
    assert!(none.contains("IdentityAgent=none"), "{none}");
    assert!(none.contains("IdentitiesOnly=yes"), "{none}");
    let routed = ssh_command(Some("'/chat/purlis-ssh/bin/ssh'"));
    assert!(
        routed.starts_with("'/chat/purlis-ssh/bin/ssh' "),
        "{routed}"
    );
    assert!(!routed.contains("-F /dev/null"), "{routed}");
    assert!(routed.contains("IdentityAgent=none"), "{routed}");
}

#[test]
fn the_helper_list_is_emptied_after_the_persons_own_config_and_before_purlis_s() {
    let text = git_config(
        Some(Path::new("/home/ada/.gitconfig")),
        Some("!purlis-helper"),
    );
    let included = text.find("/home/ada/.gitconfig").expect("their config");
    let emptied = text.find("helper =\n").expect("emptied");
    let own = text.find("helper = !purlis-helper").expect("purlis's own");
    assert!(included < emptied && emptied < own, "{text}");
    let none = git_config(None, None);
    assert!(!none.contains("[include]"), "{none}");
    assert!(none.contains("helper =\n"), "{none}");
}

#[test]
fn a_locked_chat_gets_an_empty_gh_folder_and_a_git_config_of_its_own() {
    let scratch = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    std::fs::write(home.path().join(".gitconfig"), "[user]\n\tname = Ada\n").unwrap();
    let env = env(Some(scratch.path()), Some(home.path()), None).unwrap();
    let get = |name: &str| {
        env.iter()
            .find(|(set, _)| set == name)
            .map(|(_, value)| value.clone())
            .unwrap_or_else(|| panic!("{name} is not set"))
    };
    let gh = PathBuf::from(get("GH_CONFIG_DIR"));
    assert!(gh.is_dir());
    assert_eq!(std::fs::read_dir(&gh).unwrap().count(), 0, "empty");
    let git = std::fs::read_to_string(get("GIT_CONFIG_GLOBAL")).unwrap();
    assert!(git.contains("helper =\n"), "{git}");
    assert!(git.contains(".gitconfig"), "{git}");
    assert_eq!(get("GIT_CONFIG_SYSTEM"), "/dev/null");
    assert_eq!(get("GIT_TERMINAL_PROMPT"), "0");
    assert_eq!(get("SSH_AUTH_SOCK"), "");
    assert!(get("GIT_SSH_COMMAND").contains("IdentityAgent=none"));
    // The helper list is emptied at the command scope too, past a repository's own config.
    assert_eq!(get("GIT_CONFIG_COUNT"), "1");
    assert_eq!(get("GIT_CONFIG_KEY_0"), "credential.helper");
    assert_eq!(get("GIT_CONFIG_VALUE_0"), "");
}

#[test]
fn the_lock_s_config_entry_goes_after_the_ones_a_chat_already_has() {
    let chat = vec![
        ("GIT_CONFIG_COUNT".to_owned(), "2".to_owned()),
        ("GIT_CONFIG_KEY_0".to_owned(), "a.b".to_owned()),
        ("GIT_CONFIG_VALUE_0".to_owned(), "1".to_owned()),
        ("GIT_CONFIG_KEY_1".to_owned(), "c.d".to_owned()),
        ("GIT_CONFIG_VALUE_1".to_owned(), "2".to_owned()),
    ];
    let locked = apply(chat, None, None).unwrap();
    let get = |name: &str| {
        locked
            .iter()
            .find(|(set, _)| set == name)
            .map(|(_, value)| value.as_str())
    };
    assert_eq!(get("GIT_CONFIG_COUNT"), Some("3"));
    assert_eq!(get("GIT_CONFIG_KEY_0"), Some("a.b"));
    assert_eq!(get("GIT_CONFIG_KEY_1"), Some("c.d"));
    assert_eq!(get("GIT_CONFIG_KEY_2"), Some("credential.helper"));
    assert_eq!(get("GIT_CONFIG_VALUE_2"), Some(""));
}

#[test]
fn claude_code_keeps_no_ssh_command_its_line_did_not_name_and_the_others_keep_theirs() {
    let named = vec![("GIT_SSH_COMMAND".to_owned(), "'/route/ssh'".to_owned())];
    let has = |env: &[(String, String)]| env.iter().any(|(key, _)| key == "GIT_SSH_COMMAND");
    assert!(!has(&apply_to_line(Vec::new(), None, None, true).unwrap()));
    assert!(has(&apply_to_line(named, None, None, true).unwrap()));
    assert!(has(&apply_to_line(Vec::new(), None, None, false).unwrap()));
}

#[test]
fn a_host_is_what_is_left_of_an_origin_with_its_userinfo_port_and_path_off() {
    // `#` stands for the at sign, put back at run time, so no source line reads as an email.
    for (origin, host) in [
        ("https://github.com/login", "github.com"),
        ("https://x#github.com/login", "github.com"),
        ("https://user:pw#github.com:443/a?b", "github.com"),
        ("github.com:8443", "github.com"),
        ("https://[::1]:80/", "::1"),
        ("https://a#b#gitlab.com", "gitlab.com"),
    ] {
        let origin = origin.replace('#', "@");
        let origin = origin.as_str();
        assert_eq!(host_of(origin), host, "{origin}");
        assert_eq!(
            is_a_forge_host(&host_of(origin), &[]),
            host.ends_with("github.com") || host.ends_with("gitlab.com"),
            "{origin}"
        );
    }
}

#[test]
fn a_lock_with_no_folder_to_write_in_reads_as_nothing_at_all() {
    let env = env(None, None, None).unwrap();
    for name in ["GH_CONFIG_DIR", "GIT_CONFIG_GLOBAL", "GIT_CONFIG_SYSTEM"] {
        assert!(
            env.contains(&(name.to_owned(), "/dev/null".to_owned())),
            "{name}"
        );
    }
}

#[test]
fn applying_the_lock_replaces_the_chats_own_and_keeps_its_ssh_route() {
    let scratch = tempfile::tempdir().unwrap();
    let chat = vec![
        ("PATH".to_owned(), "/bin".to_owned()),
        ("SSH_AUTH_SOCK".to_owned(), "/agent".to_owned()),
        (
            "GH_CONFIG_DIR".to_owned(),
            "/home/ada/.config/gh".to_owned(),
        ),
        ("GIT_SSH_COMMAND".to_owned(), "'/route/ssh'".to_owned()),
    ];
    let locked = apply(chat, Some(scratch.path()), None).unwrap();
    let once = |name: &str| {
        let all: Vec<_> = locked.iter().filter(|(set, _)| set == name).collect();
        assert_eq!(all.len(), 1, "{name}: {all:?}");
        all[0].1.clone()
    };
    assert_eq!(once("PATH"), "/bin");
    assert_eq!(once("SSH_AUTH_SOCK"), "");
    assert_ne!(once("GH_CONFIG_DIR"), "/home/ada/.config/gh");
    assert!(once("GIT_SSH_COMMAND").starts_with("'/route/ssh' "));
}

#[test]
fn a_forge_host_is_a_forges_own_name_or_one_of_the_projects_remotes() {
    let known = vec!["git.acme.test".to_owned()];
    for host in [
        "github.com",
        "GitHub.com.",
        "api.github.com",
        "gitlab.com",
        "git.acme.test",
        "sso.git.acme.test",
    ] {
        assert!(is_a_forge_host(host, &known), "{host}");
    }
    for host in ["notgithub.com", "example.com", "github.com.evil.test"] {
        assert!(!is_a_forge_host(host, &known), "{host}");
    }
    assert!(browser_login_refused("github.com").contains("Agent identity"));
}

#[test]
fn every_harness_has_its_forge_connectors_switched_off() {
    let denied = claude_denied();
    for name in CONNECTORS {
        assert!(denied.contains(&format!("mcp__{name}")), "{name}");
    }
    assert!(!denied.iter().any(|rule| rule.contains("purlis")));
    // Codex is told off the connectors its config has, and only those.
    let dir = tempfile::tempdir().unwrap();
    let user = dir.path().join("config.toml");
    std::fs::write(
        &user,
        "[mcp_servers.github]\ncommand = \"x\"\n[mcp_servers.mine]\ncommand = \"y\"\n",
    )
    .unwrap();
    let project = dir.path().join("project.toml");
    std::fs::write(&project, "[mcp_servers.GitLab]\nurl = \"https://z\"\n").unwrap();
    let missing = dir.path().join("none.toml");
    let codex = connectors(Harness::Codex, &[user, project, missing.clone()]);
    assert_eq!(
        codex,
        vec![
            "-c",
            "mcp_servers.github.enabled=false",
            "-c",
            "mcp_servers.GitLab.enabled=false"
        ]
    );
    assert!(connectors(Harness::Codex, &[missing]).is_empty());
    assert!(connectors(Harness::ClaudeCode, &[]).is_empty());
    let config = opencode_config(
        r#"{"plugin":["x"],"mcp":{"purlis":{"type":"local","command":["p"]},"github":{"url":"u"}}}"#,
    );
    let parsed: serde_json::Value = serde_json::from_str(&config).unwrap();
    assert_eq!(
        parsed["mcp"]["purlis"]["command"][0], "p",
        "purlis's own stays"
    );
    assert_eq!(
        parsed["mcp"]["github"]["url"], "u",
        "a name already there is left to it"
    );
    assert_eq!(parsed["mcp"]["gitlab"]["enabled"], false);
    let bare: serde_json::Value =
        serde_json::from_str(&opencode_config(r#"{"plugin":["x"]}"#)).unwrap();
    assert_eq!(bare["mcp"]["github"]["enabled"], false);
}

#[test]
fn an_unprobed_version_is_not_locked() {
    for harness in Harness::ALL {
        for version in [None, Some(""), Some("0.0.1")] {
            assert!(
                matches!(status(harness, version), Status::NotLocked(_)),
                "{harness:?} {version:?}"
            );
        }
    }
}

#[test]
fn a_probed_version_with_a_route_open_is_not_locked_and_says_which() {
    for probed in PROBED {
        let Status::NotLocked(why) = status(probed.harness, Some(probed.version)) else {
            panic!("{probed:?} is locked with a route open");
        };
        assert!(why.contains("gh-keychain"), "{why}");
    }
}

#[test]
fn every_harness_is_probed_once_and_records_its_own_open_routes() {
    for harness in Harness::ALL {
        assert_eq!(
            PROBED
                .iter()
                .filter(|probed| probed.harness == harness)
                .count(),
            1,
            "{harness:?}"
        );
    }
    let said_open: Vec<Route> = Route::ALL
        .into_iter()
        .filter(|route| matches!(route.closed_by(), Closure::Open(_)))
        // A bare ssh is the route's own, in Codex and opencode alone.
        .filter(|route| *route != Route::BareSsh)
        .collect();
    for probed in PROBED {
        // Every route said open is open on each harness, and a harness may add its own.
        for route in &said_open {
            assert!(probed.open.contains(route), "{probed:?} {route:?}");
        }
        for route in probed.open {
            assert!(
                probed.open.iter().filter(|other| *other == route).count() == 1,
                "{probed:?} {route:?} twice"
            );
        }
    }
    let open = |harness: Harness| {
        PROBED
            .iter()
            .find(|probed| probed.harness == harness)
            .unwrap()
            .open
    };
    // Claude Code's line has no `GIT_SSH_COMMAND`, so its ssh is the person's own.
    assert!(open(Harness::ClaudeCode).contains(&Route::SshAgent));
    assert!(!open(Harness::Codex).contains(&Route::SshAgent));
    assert!(!open(Harness::Opencode).contains(&Route::SshAgent));
    // Codex and opencode run a bare ssh through purlis's route, which reads `~/.ssh/config`.
    assert!(open(Harness::Codex).contains(&Route::BareSsh));
    assert!(open(Harness::Opencode).contains(&Route::BareSsh));
    assert!(!open(Harness::ClaudeCode).contains(&Route::BareSsh));
}
