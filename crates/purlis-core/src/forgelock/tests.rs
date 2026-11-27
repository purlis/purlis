// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use std::path::PathBuf;

use super::*;

#[cfg(not(feature = "forge-wiring"))]
#[test]
fn a_chat_is_claimed_where_it_holds_the_credentials_class_or_has_a_mint() {
    assert!(claimed(true, false));
    assert!(claimed(false, true));
    assert!(claimed(true, true));
    assert!(!claimed(false, false));
}

#[cfg(feature = "forge-wiring")]
#[test]
fn every_chat_is_claimed_whatever_it_holds_and_whether_or_not_it_mints() {
    assert!(claimed(false, false));
    assert!(claimed(true, false));
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
    // The command-scope pair is added where the chat is composed ([`arm`]), not here.
    assert_eq!(get(MARKER), "1");
}

#[test]
fn the_lock_s_config_pair_goes_after_a_profile_s_a_passed_one_and_the_hooks() {
    // Production's order: what a chat inherits and a profile sets, then purlis's hooks, then the
    // lock, composed by `chatenv::compose` with the line's variables as its `set`.
    let app = vec![
        ("GIT_CONFIG_COUNT".into(), "1".into()),
        ("GIT_CONFIG_KEY_0".into(), "passed.one".into()),
        ("GIT_CONFIG_VALUE_0".into(), "p".into()),
    ];
    let line = apply(
        vec![
            ("GIT_CONFIG_COUNT".to_owned(), "1".to_owned()),
            ("GIT_CONFIG_KEY_0".to_owned(), "profile.one".to_owned()),
            ("GIT_CONFIG_VALUE_0".to_owned(), "q".to_owned()),
        ],
        None,
        None,
    )
    .unwrap();
    let hooks = crate::githooks::GitHooks::at("/app/git-hooks");
    let composed = crate::chatenv::compose(
        app,
        &crate::chatenv::Starting {
            harness: None,
            operator: &["GIT_CONFIG_COUNT".to_owned()],
            strip: &[],
            set: &line,
            git_hooks: Some(&hooks),
            declared: &[],
        },
    );
    let get = |name: &str| {
        composed
            .iter()
            .rev()
            .find(|(set, _)| set == name)
            .map(|(_, value)| value.to_string_lossy().into_owned())
    };
    // The profile's pair replaced the passed one by name, as before; the no-gc pairs (#7531),
    // hooks, then the lock follow, so the lock's is still the last.
    assert_eq!(get("GIT_CONFIG_KEY_0").as_deref(), Some("profile.one"));
    assert_eq!(get("GIT_CONFIG_KEY_1").as_deref(), Some("gc.auto"));
    assert_eq!(get("GIT_CONFIG_KEY_2").as_deref(), Some("maintenance.auto"));
    assert_eq!(get("GIT_CONFIG_KEY_3").as_deref(), Some("core.hooksPath"));
    assert_eq!(
        get("GIT_CONFIG_KEY_4").as_deref(),
        Some("credential.helper")
    );
    assert_eq!(get("GIT_CONFIG_VALUE_4").as_deref(), Some(""));
    assert_eq!(get("GIT_CONFIG_COUNT").as_deref(), Some("5"));
    assert_eq!(get(MARKER), None, "the marker is not the chat's");
}

#[test]
fn arm_keeps_every_pair_a_chat_has_and_adds_the_lock_s_last() {
    let pair = |n: &str, v: &str| (std::ffi::OsString::from(n), std::ffi::OsString::from(v));
    let armed = arm(vec![
        pair("GIT_CONFIG_COUNT", "2"),
        pair("GIT_CONFIG_KEY_0", "a.b"),
        pair("GIT_CONFIG_VALUE_0", "1"),
        pair("GIT_CONFIG_KEY_1", "c.d"),
        pair("GIT_CONFIG_VALUE_1", "2"),
    ]);
    assert_eq!(
        armed[armed.len() - 3..],
        [
            pair("GIT_CONFIG_COUNT", "3"),
            pair("GIT_CONFIG_KEY_2", "credential.helper"),
            pair("GIT_CONFIG_VALUE_2", ""),
        ]
    );
    assert!(armed.contains(&pair("GIT_CONFIG_KEY_1", "c.d")));
    assert!(armed.contains(&pair("GIT_CONFIG_KEY_0", "a.b")));
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

#[test]
fn claude_code_is_denied_a_forge_s_plugin_server_and_its_case_variants() {
    let denied = claude_denied();
    for name in [
        "mcp__plugin_github_github",
        "mcp__plugin_gitlab_gitlab",
        "mcp__GitHub",
        "mcp__github",
    ] {
        assert!(denied.iter().any(|rule| rule == name), "{name}");
    }
}
