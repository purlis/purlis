// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

/// A command run to its end, for its answer.
fn out(command: &mut std::process::Command) -> std::process::Output {
    crate::forklock::output(command).expect("a command ran")
}

const SENTINEL: &str = "SENTINEL-0123456789abcdef0123456789abcdef";

fn repo(host: &str, owner: &str, name: &str, kind: ForgeKind) -> Repo {
    Repo::new(host, owner, name, kind).expect("a repo")
}

fn route(dir: &Path, repos: Vec<Repo>) -> Route {
    std::fs::write(dir.join("ca.pem"), "-----BEGIN CERTIFICATE-----\n").expect("a CA");
    Route {
        egress_url: "http://127.0.0.1:43001".to_owned(),
        gh_socket: dir.join("sock/gh.sock"),
        ca_cert: dir.join("ca.pem"),
        sentinel: SecretString::from(SENTINEL),
        helper_exe: PathBuf::from("/opt/purlis/purlis"),
        repos,
    }
}

fn get<'a>(env: &'a [(String, String)], name: &str) -> Option<&'a str> {
    env.iter()
        .rev()
        .find(|(n, _)| n == name)
        .map(|(_, v)| v.as_str())
}

#[test]
fn the_versions_the_route_was_run_with_are_those_of_spike_e() {
    let all = |tool| (proven(tool, Os::Linux), proven(tool, Os::MacOs));
    assert_eq!(all(Tool::Gh), (Some("2.103.3"), Some("2.105.0")));
    assert_eq!(all(Tool::Glab), (Some("1.119.0"), Some("1.131.0")));
    assert_eq!(all(Tool::Git), (Some("2.56.1"), Some("2.56.1")));
}

#[test]
fn a_versions_number_is_read_from_the_first_line_of_what_a_tool_says() {
    let said = |text| parse_version(text);
    assert_eq!(
        said("gh version 2.105.0 (2026-11-02)\nhttps://x"),
        Some("2.105.0".into())
    );
    assert_eq!(said("glab 1.131.0 (2026-10-01)"), Some("1.131.0".into()));
    assert_eq!(said("git version 2.56.1"), Some("2.56.1".into()));
    assert_eq!(
        said("git version 2.39.5 (Apple Git-154)"),
        Some("2.39.5".into())
    );
    assert_eq!(said("no number here"), None);
    assert_eq!(said(""), None);
    // Only the first line: a later one is a changelog's, not the tool's.
    assert_eq!(said("hello\ngh version 2.0.0"), None);
}

#[test]
fn a_version_that_was_run_gets_no_advice_and_any_other_gets_a_sentence_with_a_fix() {
    assert_eq!(check_version(Tool::Gh, Os::MacOs, Some("2.105.0")), None);
    assert_eq!(check_version(Tool::Gh, Os::Linux, Some(" 2.103.3 ")), None);
    // Each system's own: the macOS version is not the Linux one.
    let linux_on_mac = check_version(Tool::Gh, Os::MacOs, Some("2.103.3")).expect("advice");
    assert_eq!(linux_on_mac.cause, "forge-route-unproven-gh");
    assert_eq!(
        linux_on_mac.fix,
        Fix::Install {
            tool: Tool::Gh,
            version: "2.105.0"
        }
    );
    assert_eq!(
        linux_on_mac.sentence,
        "gh 2.103.3 is not a version purlis has run its forge route with on macOS (2.105.0), so \
         gh may fail on it. Install gh 2.105.0, or Dismiss to carry on."
    );
    // A tool whose version could not be read is as unproven as one that differs.
    let unknown = check_version(Tool::Glab, Os::Linux, None).expect("advice");
    assert!(
        unknown.sentence.starts_with("this glab is not a version"),
        "{unknown:?}"
    );
}

#[test]
fn cargo_set_to_use_libgit2_gets_the_named_gap_and_the_set_one_gets_none() {
    let on = vec![(FETCH_WITH_CLI_ENV.to_owned(), "true".to_owned())];
    assert_eq!(libgit2_gap(&on), None);
    let off = vec![(FETCH_WITH_CLI_ENV.to_owned(), "false".to_owned())];
    let advice = libgit2_gap(&off).expect("the gap");
    assert_eq!(advice.fix, Fix::Setting("net.git-fetch-with-cli"));
    assert_eq!(advice.cause, "forge-route-cargo-libgit2");
    assert!(
        advice
            .sentence
            .contains("Set `net.git-fetch-with-cli` to true"),
        "{advice:?}"
    );
    assert!(libgit2_gap(&[]).is_some());
    // The last of a name is the one cargo reads.
    let last = vec![
        (FETCH_WITH_CLI_ENV.to_owned(), "true".to_owned()),
        (FETCH_WITH_CLI_ENV.to_owned(), "false".to_owned()),
    ];
    assert!(libgit2_gap(&last).is_some());
}

#[test]
fn a_repo_with_a_character_that_would_need_quoting_is_refused() {
    for (host, owner, name) in [
        ("github.com", "o", "r"),
        ("ghe.corp.example", "my-org", "re.po_1"),
    ] {
        assert!(Repo::new(host, owner, name, ForgeKind::GitHub).is_ok());
    }
    for (host, owner, name) in [
        ("github.com", "o'x", "r"),
        ("github.com", "o", "r r"),
        ("github.com", "o/p", "r"),
        ("github.com", "o", ".."),
        ("github.com", "", "r"),
        ("gi\nthub.com", "o", "r"),
        ("github.com", "o", "r\"; rm -rf ~; \""),
        ("github.com", "$(id)", "r"),
    ] {
        assert!(
            Repo::new(host, owner, name, ForgeKind::GitHub).is_err(),
            "{host}/{owner}/{name}"
        );
    }
    // A GitLab owner is a path of groups, each one checked.
    assert!(Repo::new("gitlab.com", "grp/sub", "r", ForgeKind::GitLab).is_ok());
    assert!(Repo::new("gitlab.com", "grp//sub", "r", ForgeKind::GitLab).is_err());
    assert!(Repo::new("gitlab.com", "grp/../x", "r", ForgeKind::GitLab).is_err());
    // A host is lowercase, without a final dot.
    assert_eq!(
        repo("GitHub.COM.", "o", "r", ForgeKind::GitHub).host(),
        "github.com"
    );
}

#[test]
fn the_composed_environment_is_exactly_what_the_design_names() {
    let dir = tempfile::tempdir().unwrap();
    let route = route(
        dir.path(),
        vec![
            repo("github.com", "acme", "app", ForgeKind::GitHub),
            repo("gitlab.com", "grp", "lib", ForgeKind::GitLab),
        ],
    );
    let wiring = compose(&route, dir.path()).expect("composed");
    let gh = wiring.bin.parent().unwrap().join("gh");
    let mut expected = vec![
        ("GH_HOST", "github.localhost".to_owned()),
        ("GH_CONFIG_DIR", gh.display().to_string()),
        ("GH_TOKEN", SENTINEL.to_owned()),
        ("GH_ENTERPRISE_TOKEN", SENTINEL.to_owned()),
        ("GITLAB_TOKEN", SENTINEL.to_owned()),
        (
            "GLAB_CA_CERT",
            gh.parent().unwrap().join(CA_COPY).display().to_string(),
        ),
        ("HTTPS_PROXY", "http://127.0.0.1:43001".to_owned()),
        ("https_proxy", "http://127.0.0.1:43001".to_owned()),
        ("CARGO_NET_GIT_FETCH_WITH_CLI", "true".to_owned()),
        ("PURLIS_FORGE_SENTINEL", SENTINEL.to_owned()),
    ]
    .into_iter()
    .map(|(n, v)| (n.to_owned(), v))
    .collect::<Vec<_>>();
    expected.sort();
    assert_eq!(wiring.env, expected);
    // `GITLAB_HOST` stays the real one (glab's own inference), so it is not set.
    assert_eq!(get(&wiring.env, "GITLAB_HOST"), None);
    // In a folder made new inside the one given, never beside the chat's own.
    assert_eq!(wiring.bin.file_name().unwrap(), "bin");
    let folder = wiring.bin.parent().unwrap();
    assert_eq!(folder.parent().unwrap(), wire_folder(dir.path()));
    assert!(
        folder
            .file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with(&format!("chat-{}-", std::process::id()))
    );
}

#[test]
fn nothing_widens_a_trust_store_or_names_a_token_for_npm_or_pip() {
    let dir = tempfile::tempdir().unwrap();
    let route = route(
        dir.path(),
        vec![repo("github.com", "o", "r", ForgeKind::GitHub)],
    );
    let wiring = compose(&route, dir.path()).unwrap();
    for never in [
        "NODE_EXTRA_CA_CERTS",
        "PIP_CERT",
        "GIT_SSL_CAINFO",
        "GIT_SSL_CAPATH",
        "SSL_CERT_FILE",
        "SSL_CERT_DIR",
        "CURL_CA_BUNDLE",
        "REQUESTS_CA_BUNDLE",
        "GIT_SSL_NO_VERIFY",
        "GITLAB_HOST",
        "GH_REPO",
        "NO_PROXY",
        "no_proxy",
        "GIT_CONFIG_GLOBAL",
    ] {
        assert_eq!(get(&wiring.env, never), None, "{never} is set");
    }
    assert!(
        wiring
            .git_pairs
            .iter()
            .all(|(key, _)| key != "http.sslCAInfo"
                && key != "http.extraHeader"
                && !key.starts_with("url.")),
        "{:?}",
        wiring.git_pairs
    );
}

#[test]
fn gits_pairs_name_the_reset_first_then_a_helper_and_a_ca_for_each_recorded_host_alone() {
    let dir = tempfile::tempdir().unwrap();
    let route = route(
        dir.path(),
        vec![
            repo("github.com", "acme", "app", ForgeKind::GitHub),
            repo("github.com", "acme", "other", ForgeKind::GitHub),
            repo("git.example.org", "grp", "lib", ForgeKind::GitLab),
        ],
    );
    let wiring = compose(&route, dir.path()).unwrap();
    // The chat's own copy, in its wire folder: the route folder is held from the chat.
    let ca = wiring
        .bin
        .parent()
        .unwrap()
        .join(CA_COPY)
        .display()
        .to_string();
    let helper = |_host: &str| {
        "!'/opt/purlis/purlis' purlis-git-sentinel --host git.example.org --host github.com"
            .to_owned()
    };
    assert_eq!(
        wiring.git_pairs,
        vec![
            ("credential.helper".to_owned(), String::new()),
            (
                "credential.https://git.example.org.helper".to_owned(),
                helper("")
            ),
            (
                "credential.https://github.com.helper".to_owned(),
                helper("")
            ),
            (
                "http.https://git.example.org/.sslCAInfo".to_owned(),
                ca.clone()
            ),
            ("http.https://github.com/.sslCAInfo".to_owned(), ca),
        ]
        .into_iter()
        .chain(
            // A proxy for each forge host and for every name under it, so `NO_PROXY` and a
            // person's own `http.proxy` cannot send git direct to one.
            [
                "api.github.com",
                "git.example.org",
                "github.com",
                "githubusercontent.com",
                "gitlab.com",
            ]
            .into_iter()
            .flat_map(|host| [host.to_owned(), format!("*.{host}")])
            .map(|url| {
                (
                    format!("http.https://{url}/.proxy"),
                    "http://127.0.0.1:43001".to_owned(),
                )
            }),
        )
        .collect::<Vec<_>>()
    );
}

#[test]
fn the_gh_folder_holds_one_setting_and_no_login() {
    let dir = tempfile::tempdir().unwrap();
    let route = route(
        dir.path(),
        vec![repo("github.com", "o", "r", ForgeKind::GitHub)],
    );
    let wiring = compose(&route, dir.path()).unwrap();
    let gh = wiring.bin.parent().unwrap().join("gh");
    let mut names: Vec<_> = std::fs::read_dir(&gh)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    assert_eq!(names, ["config.yml"], "no hosts.yml, no login");
    let text = std::fs::read_to_string(gh.join("config.yml")).unwrap();
    let settings: Vec<&str> = text.lines().filter(|l| !l.starts_with('#')).collect();
    assert_eq!(
        settings,
        [format!(
            "http_unix_socket: '{}'",
            dir.path().join("sock/gh.sock").display()
        )]
    );
    assert!(
        !text.contains(SENTINEL),
        "the sentinel is never written to a file"
    );
}

#[cfg(unix)]
#[test]
fn the_folders_are_the_chats_alone_and_no_file_holds_the_sentinel() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let route = route(
        dir.path(),
        vec![repo("github.com", "o", "r", ForgeKind::GitHub)],
    );
    let wiring = compose(&route, dir.path()).unwrap();
    let root = wiring.bin.parent().unwrap().to_path_buf();
    let mode = |p: &Path| std::fs::metadata(p).unwrap().permissions().mode() & 0o777;
    for folder in [
        root.clone(),
        root.join("gh"),
        root.join("bin"),
        wire_folder(dir.path()),
    ] {
        assert_eq!(mode(&folder), 0o700, "{}", folder.display());
    }
    assert_eq!(mode(&root.join("gh/config.yml")), 0o600);
    assert_eq!(mode(&root.join("bin/gh")), 0o700);
    let wrapper = std::fs::read_to_string(root.join("bin/gh")).unwrap();
    assert!(!wrapper.contains(SENTINEL));
}

#[test]
fn a_route_that_is_not_the_chats_own_loopback_port_or_has_an_unsafe_sentinel_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let good = || {
        route(
            dir.path(),
            vec![repo("github.com", "o", "r", ForgeKind::GitHub)],
        )
    };
    let mut remote = good();
    remote.egress_url = "http://proxy.example.com:3128".to_owned();
    assert!(compose(&remote, dir.path()).is_err());
    for bad in ["", "has space", "line\nbreak"] {
        let mut route = good();
        route.sentinel = SecretString::from(bad);
        assert!(compose(&route, dir.path()).is_err(), "{bad:?}");
    }
    let mut quote = good();
    quote.helper_exe = PathBuf::from("/opt/it's/purlis");
    assert!(compose(&quote, dir.path()).is_err());
}

#[test]
fn a_route_never_prints_its_sentinel() {
    let dir = tempfile::tempdir().unwrap();
    let route = route(dir.path(), vec![]);
    assert!(!format!("{route:?}").contains(SENTINEL));
}

#[test]
fn the_pairs_go_after_every_pair_already_there_and_the_count_covers_them() {
    let dir = tempfile::tempdir().unwrap();
    let route = route(
        dir.path(),
        vec![repo("github.com", "o", "r", ForgeKind::GitHub)],
    );
    let wiring = compose(&route, dir.path()).unwrap();
    let env = vec![
        ("GIT_CONFIG_COUNT".to_owned(), "2".to_owned()),
        ("GIT_CONFIG_KEY_0".to_owned(), "user.name".to_owned()),
        ("GIT_CONFIG_VALUE_0".to_owned(), "a profile".to_owned()),
        ("GIT_CONFIG_KEY_1".to_owned(), "core.hooksPath".to_owned()),
        ("GIT_CONFIG_VALUE_1".to_owned(), "/hooks".to_owned()),
        ("PATH".to_owned(), "/usr/bin".to_owned()),
    ];
    let out = apply(env, &wiring);
    assert_eq!(
        get(&out, "GIT_CONFIG_COUNT"),
        Some((2 + wiring.git_pairs.len()).to_string().as_str())
    );
    assert_eq!(get(&out, "GIT_CONFIG_KEY_0"), Some("user.name"));
    assert_eq!(get(&out, "GIT_CONFIG_KEY_1"), Some("core.hooksPath"));
    // The reset is the first of purlis's, and the helper follows it.
    assert_eq!(get(&out, "GIT_CONFIG_KEY_2"), Some("credential.helper"));
    assert_eq!(get(&out, "GIT_CONFIG_VALUE_2"), Some(""));
    assert_eq!(
        get(&out, "GIT_CONFIG_KEY_3"),
        Some("credential.https://github.com.helper")
    );
    assert_eq!(
        get(&out, "GIT_CONFIG_KEY_4"),
        Some("http.https://github.com/.sslCAInfo")
    );
    assert_eq!(get(&out, "PATH"), Some("/usr/bin"));
}

#[test]
fn a_locked_chats_marker_is_taken_out_so_its_empty_helper_cannot_come_after_purlis_s() {
    let dir = tempfile::tempdir().unwrap();
    let route = route(
        dir.path(),
        vec![repo("github.com", "o", "r", ForgeKind::GitHub)],
    );
    let wiring = compose(&route, dir.path()).unwrap();
    let env = vec![
        (crate::forgelock::MARKER.to_owned(), "1".to_owned()),
        ("GH_CONFIG_DIR".to_owned(), "/the/locks/own".to_owned()),
        ("GIT_SSL_CAINFO".to_owned(), "/a/bundle.pem".to_owned()),
        ("GIT_SSL_CAPATH".to_owned(), "/a/dir".to_owned()),
    ];
    let out = apply(env, &wiring);
    assert_eq!(get(&out, crate::forgelock::MARKER), None);
    assert_eq!(get(&out, "GIT_SSL_CAINFO"), None);
    assert_eq!(get(&out, "GIT_SSL_CAPATH"), None);
    // The chat's own gh folder, not the lock's empty one.
    assert_eq!(
        get(&out, "GH_CONFIG_DIR"),
        Some(wiring.bin.parent().unwrap().join("gh").to_str().unwrap())
    );
    assert_eq!(out.iter().filter(|(n, _)| n == "GH_CONFIG_DIR").count(), 1);
}

#[test]
fn every_ambient_forge_token_is_scrubbed_and_the_chat_holds_the_sentinel_alone() {
    let dir = tempfile::tempdir().unwrap();
    let route = route(
        dir.path(),
        vec![repo("github.com", "o", "r", ForgeKind::GitHub)],
    );
    let wiring = compose(&route, dir.path()).unwrap();
    let ambient = [
        "GH_TOKEN",
        "GITHUB_TOKEN",
        "GH_ENTERPRISE_TOKEN",
        "GITLAB_TOKEN",
        "GLAB_TOKEN",
        "CI_JOB_TOKEN",
        "GITHUB_ENTERPRISE_TOKEN",
        "OAUTH_TOKEN",
        "GITLAB_ACCESS_TOKEN",
        "PURLIS_FORGE_TOKEN",
        "PURLIS_FORGE_TOKEN_GITHUB",
        "CHARTER_FORGE_TOKEN",
        "CHARTER_FORGE_TOKEN_GITHUB",
        "CHARTER_FORGE_TOKEN__6769746875622e636f6d",
        "PURLIS_FORGE_TOKEN_FILE__6769746875622e636f6d",
        "CHARTER_FORGE_TOKEN_EXPIRES_AT__6769746875622e636f6d",
        "PURLIS_FORGE_KIND__6769746875622e636f6d",
        "PURLIS_FORGE_ACCEPT__6769746875622e636f6d",
        "gh_token",
    ];
    let mut env: Vec<(String, String)> = ambient
        .iter()
        .map(|name| ((*name).to_owned(), "ambient-value".to_owned()))
        .collect();
    env.push(("PATH".to_owned(), "/usr/bin".to_owned()));
    let out = apply(env, &wiring);
    for name in ambient {
        let held = get(&out, name);
        assert!(
            held.is_none() || held == Some(SENTINEL),
            "{name} kept an ambient value"
        );
    }
    assert_eq!(get(&out, "GITLAB_TOKEN"), Some(SENTINEL));
    assert_eq!(get(&out, "GH_TOKEN"), Some(SENTINEL));
    assert_eq!(get(&out, "GITHUB_TOKEN"), None);
    assert_eq!(get(&out, "GLAB_TOKEN"), None);
    assert_eq!(get(&out, "CI_JOB_TOKEN"), None);
    assert_eq!(get(&out, "PURLIS_FORGE_TOKEN_GITHUB"), None);
    for name in [
        "GITHUB_ENTERPRISE_TOKEN",
        "OAUTH_TOKEN",
        "GITLAB_ACCESS_TOKEN",
        "CHARTER_FORGE_TOKEN",
        "CHARTER_FORGE_TOKEN_GITHUB",
        "CHARTER_FORGE_TOKEN__6769746875622e636f6d",
        "PURLIS_FORGE_TOKEN_FILE__6769746875622e636f6d",
        "CHARTER_FORGE_TOKEN_EXPIRES_AT__6769746875622e636f6d",
        "PURLIS_FORGE_KIND__6769746875622e636f6d",
        "PURLIS_FORGE_ACCEPT__6769746875622e636f6d",
    ] {
        assert_eq!(get(&out, name), None, "{name} survived");
    }
    assert!(!out.iter().any(|(_, v)| v == "ambient-value"));
    assert_eq!(get(&out, "PATH"), Some("/usr/bin"));
}

#[test]
fn the_wrappers_folder_goes_first_ahead_of_the_one_already_named() {
    let bin = Path::new("/scratch/forge-wire/bin");
    assert_eq!(first_on_path(None, bin), "/scratch/forge-wire/bin");
    assert_eq!(first_on_path(Some(""), bin), "/scratch/forge-wire/bin");
    assert_eq!(
        first_on_path(Some("/tools/bin"), bin),
        "/scratch/forge-wire/bin:/tools/bin"
    );
}

#[test]
fn the_forge_hosts_are_denied_as_a_tunnel_and_the_tunnelled_names_never_are() {
    let repos = vec![
        repo("ghe.corp.example", "o", "r", ForgeKind::GitHub),
        repo("git.example.org", "g", "r", ForgeKind::GitLab),
        repo("github.com", "o", "r", ForgeKind::GitHub),
    ];
    let denied = denied_hosts(&repos);
    for host in [
        "github.com",
        "api.github.com",
        "githubusercontent.com",
        "gitlab.com",
        "ghe.corp.example",
        "git.example.org",
    ] {
        assert!(denied.iter().any(|d| d == host), "{host} in {denied:?}");
    }
    for entry in TUNNELLED {
        let host = entry.split(':').next().unwrap();
        assert!(!denied.iter().any(|d| d == host), "{host} is denied");
    }
    // Once each, in order.
    let mut sorted = denied.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(denied, sorted);
    // A recorded repo on a tunnelled name cannot make it denied.
    let odd = vec![repo("codeload.github.com", "o", "r", ForgeKind::GitHub)];
    assert!(
        !denied_hosts(&odd)
            .iter()
            .any(|d| d == "codeload.github.com")
    );
}

/// The wiring of a chat in a folder under `dir`, for the tests of where it is written.
fn wired(dir: &Path) -> Route {
    route(dir, vec![repo("github.com", "o", "r", ForgeKind::GitHub)])
}

#[test]
fn the_wiring_is_written_in_the_apps_folder_no_chat_writes_never_the_chats_own() {
    let root = Path::new("/plane");
    let folder = wire_folder(root);
    // Under the state's `app`, the folder every chat is denied writing.
    assert!(folder.starts_with(crate::names::state(root).join("app")));
    assert!(folder.ends_with("forge-wire"));
}

#[test]
fn a_folder_that_is_there_already_is_refused_and_nothing_is_written_through_it() {
    let dir = tempfile::tempdir().unwrap();
    let planted = dir.path().join("chat-1-planted");
    std::fs::create_dir(&planted).unwrap();
    assert!(compose_at(&wired(dir.path()), &planted).is_err());
    assert_eq!(
        std::fs::read_dir(&planted).unwrap().count(),
        0,
        "wrote into a folder a chat could have planted"
    );
}

#[cfg(unix)]
#[test]
fn a_link_in_the_folders_place_is_refused_and_its_target_is_left_alone() {
    let dir = tempfile::tempdir().unwrap();
    let target = tempfile::tempdir().unwrap();
    let link = dir.path().join("chat-1-link");
    std::os::unix::fs::symlink(target.path(), &link).unwrap();
    assert!(compose_at(&wired(dir.path()), &link).is_err());
    assert_eq!(std::fs::read_dir(target.path()).unwrap().count(), 0);
    // A link that points nowhere is no way in either.
    let dangling = dir.path().join("chat-2-dangling");
    std::os::unix::fs::symlink(dir.path().join("nowhere"), &dangling).unwrap();
    assert!(compose_at(&wired(dir.path()), &dangling).is_err());
    assert!(!dir.path().join("nowhere").exists());
}

#[cfg(unix)]
#[test]
fn a_wire_folder_that_is_a_link_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let elsewhere = tempfile::tempdir().unwrap();
    let within = wire_folder(dir.path());
    std::fs::create_dir_all(within.parent().unwrap()).unwrap();
    std::os::unix::fs::symlink(elsewhere.path(), &within).unwrap();
    assert!(compose(&wired(dir.path()), dir.path()).is_err());
    assert_eq!(std::fs::read_dir(elsewhere.path()).unwrap().count(), 0);
}

#[cfg(unix)]
#[test]
fn a_wire_folder_reached_through_a_link_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let elsewhere = tempfile::tempdir().unwrap();
    let app = crate::names::state(dir.path()).join("app");
    std::fs::create_dir_all(app.parent().unwrap()).unwrap();
    std::os::unix::fs::symlink(elsewhere.path(), &app).unwrap();
    assert!(compose(&wired(dir.path()), dir.path()).is_err());
    assert_eq!(std::fs::read_dir(elsewhere.path()).unwrap().count(), 0);
}

#[cfg(unix)]
#[test]
fn nothing_in_the_wiring_is_writable_by_anyone_but_the_person() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let within = wire_folder(dir.path());
    let wiring = compose(&wired(dir.path()), dir.path()).unwrap();
    let mut seen = vec![within.clone(), wiring.bin.parent().unwrap().to_path_buf()];
    let mut at = 0;
    while at < seen.len() {
        if seen[at].is_dir() {
            for entry in std::fs::read_dir(&seen[at]).unwrap() {
                seen.push(entry.unwrap().path());
            }
        }
        at += 1;
    }
    for path in seen {
        let mode = std::fs::symlink_metadata(&path)
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o077, 0, "{} is open to others", path.display());
    }
}

#[test]
fn an_enterprise_host_stands_for_the_names_under_it_in_the_per_host_proxy_pairs() {
    let dir = tempfile::tempdir().unwrap();
    let route = route(
        dir.path(),
        vec![repo("ghe.corp.example", "o", "r", ForgeKind::GitHub)],
    );
    let wiring = compose(&route, dir.path()).unwrap();
    for key in [
        "http.https://ghe.corp.example/.proxy",
        "http.https://*.ghe.corp.example/.proxy",
        "http.https://github.com/.proxy",
        "http.https://*.github.com/.proxy",
        "http.https://*.gitlab.com/.proxy",
    ] {
        assert!(
            wiring
                .git_pairs
                .iter()
                .any(|(k, v)| k == key && v == "http://127.0.0.1:43001"),
            "{key} in {:?}",
            wiring.git_pairs
        );
    }
}

/// A `gh` that prints each argument on a line, and a clone with the `remotes` given.
#[cfg(unix)]
struct Shop {
    dir: tempfile::TempDir,
    bin: PathBuf,
}

#[cfg(unix)]
impl Shop {
    fn new(repos: Vec<Repo>, remotes: &[(&str, &str)]) -> Self {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let wiring = compose(&route(dir.path(), repos), dir.path()).unwrap();
        let real = dir.path().join("real");
        std::fs::create_dir_all(&real).unwrap();
        std::fs::write(
            real.join("gh"),
            "#!/bin/sh\nfor a in \"$@\"; do printf '%s\\n' \"$a\"; done\n",
        )
        .unwrap();
        std::fs::set_permissions(real.join("gh"), std::fs::Permissions::from_mode(0o755)).unwrap();
        assert!(wiring.bin.join("gh").is_file());
        let clone = dir.path().join("clone");
        std::fs::create_dir_all(&clone).unwrap();
        let git = |args: &[&str]| {
            let out = out(std::process::Command::new("git")
                .args(args)
                .current_dir(&clone)
                .env("GIT_CONFIG_GLOBAL", "/dev/null")
                .env("GIT_CONFIG_SYSTEM", "/dev/null"));
            assert!(
                out.status.success(),
                "{args:?}: {}",
                String::from_utf8_lossy(&out.stderr)
            );
        };
        git(&["init", "-q"]);
        for (name, url) in remotes {
            git(&["remote", "add", name, url]);
        }
        Self {
            bin: wiring.bin,
            dir,
        }
    }

    /// What the real `gh` was given when the wrapper ran `args` from the clone.
    fn run(&self, args: &[&str]) -> Vec<String> {
        let path = format!(
            "{}:{}:/usr/bin:/bin",
            self.bin.display(),
            self.dir.path().join("real").display()
        );
        let out = out(std::process::Command::new("/bin/sh")
            .arg(self.bin.join("gh"))
            .args(args)
            .current_dir(self.dir.path().join("clone"))
            .env_clear()
            .env("PATH", path)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_SYSTEM", "/dev/null")
            // A `slug` in the chat's environment must not be trusted.
            .env("slug", "github.localhost/evil/repo"));
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8(out.stdout)
            .unwrap()
            .lines()
            .map(str::to_owned)
            .collect()
    }
}

#[cfg(unix)]
fn recorded() -> Vec<Repo> {
    vec![
        repo("github.com", "acme", "app", ForgeKind::GitHub),
        repo("ghe.corp.example", "corp", "tool", ForgeKind::GitHub),
        repo("gitlab.com", "grp", "lib", ForgeKind::GitLab),
    ]
}

#[cfg(unix)]
#[test]
fn the_wrapper_names_the_recorded_repo_of_the_directory_for_a_subcommand_that_takes_it() {
    let shop = Shop::new(recorded(), &[("origin", "https://github.com/acme/app.git")]);
    assert_eq!(
        shop.run(&["pr", "view", "1", "--json", "title"]),
        [
            "pr",
            "view",
            "-R",
            "github.localhost/acme/app",
            "1",
            "--json",
            "title"
        ]
    );
    assert_eq!(
        shop.run(&["run", "list", "--limit", "5"]),
        [
            "run",
            "list",
            "-R",
            "github.localhost/acme/app",
            "--limit",
            "5"
        ]
    );
    assert_eq!(
        shop.run(&["browse", "src/main.rs"]),
        ["browse", "-R", "github.localhost/acme/app", "src/main.rs"]
    );
}

#[cfg(unix)]
#[test]
fn the_wrapper_reads_each_way_a_remote_is_written() {
    // Synthetic userinfo URLs, built from parts: they test that userinfo is stripped.
    let (user, host) = ("someone", "github.com");
    let with_user = format!("https://{user}@{host}/acme/app.git");
    let with_user_and_password = format!("https://{user}:secret@{host}/acme/app.git");
    for url in [
        "https://github.com/acme/app",
        "https://github.com/acme/app.git",
        "https://github.com/acme/app/",
        with_user.as_str(),
        with_user_and_password.as_str(),
        "git@github.com:acme/app.git",
        "ssh://git@github.com/acme/app.git",
    ] {
        let shop = Shop::new(recorded(), &[("origin", url)]);
        assert_eq!(
            shop.run(&["pr", "checks", "7"]),
            ["pr", "checks", "-R", "github.localhost/acme/app", "7"],
            "{url}"
        );
    }
}

#[cfg(unix)]
#[test]
fn the_wrapper_finds_the_recorded_repo_among_several_remotes_and_a_ghes_one() {
    let shop = Shop::new(
        recorded(),
        &[
            ("fork", "https://github.com/someone/app.git"),
            ("upstream", "https://ghe.corp.example/corp/tool.git"),
        ],
    );
    assert_eq!(
        shop.run(&["issue", "list"]),
        ["issue", "list", "-R", "github.localhost/corp/tool"]
    );
}

#[cfg(unix)]
#[test]
fn the_chats_own_repo_flag_stands_and_a_directory_with_no_recorded_remote_is_left_alone() {
    let shop = Shop::new(recorded(), &[("origin", "https://github.com/acme/app.git")]);
    for own in [
        vec!["pr", "view", "1", "-R", "o/r"],
        vec!["pr", "view", "1", "--repo", "o/r"],
        vec!["pr", "view", "1", "--repo=o/r"],
        vec!["pr", "view", "1", "-Ro/r"],
    ] {
        assert_eq!(shop.run(&own), own, "{own:?}");
    }
    // After `--` a flag is no flag of gh's.
    assert_eq!(
        shop.run(&["pr", "create", "--", "-R"]),
        [
            "pr",
            "create",
            "-R",
            "github.localhost/acme/app",
            "--",
            "-R"
        ]
    );
    let unrecorded = Shop::new(
        recorded(),
        &[("origin", "https://github.com/other/thing.git")],
    );
    assert_eq!(unrecorded.run(&["pr", "view", "1"]), ["pr", "view", "1"]);
    // A GitLab repo is not gh's.
    let gitlab = Shop::new(recorded(), &[("origin", "https://gitlab.com/grp/lib.git")]);
    assert_eq!(gitlab.run(&["pr", "view", "1"]), ["pr", "view", "1"]);
    let none = Shop::new(recorded(), &[]);
    assert_eq!(none.run(&["pr", "view", "1"]), ["pr", "view", "1"]);
}

#[cfg(unix)]
#[test]
fn the_wrapper_leaves_every_other_command_exactly_as_the_chat_typed_it() {
    let shop = Shop::new(recorded(), &[("origin", "https://github.com/acme/app.git")]);
    for args in [
        vec!["api", "user"],
        vec!["api", "graphql", "-f", "query={ viewer { login } }"],
        vec!["repo", "view", "o/r"],
        vec!["auth", "status"],
        vec!["--version"],
        vec!["pr"],
        vec!["pr", "nonsense"],
        vec![],
    ] {
        assert_eq!(shop.run(&args), args, "{args:?}");
    }
}

#[cfg(unix)]
#[test]
fn the_wrapper_never_runs_itself_and_says_so_where_no_gh_is_installed() {
    let dir = tempfile::tempdir().unwrap();
    let wiring = compose(
        &route(
            dir.path(),
            vec![repo("github.com", "o", "r", ForgeKind::GitHub)],
        ),
        dir.path(),
    )
    .unwrap();
    // A PATH holding the wrapper and `dirname` alone: a runner with its own gh in /usr/bin
    // would otherwise run it.
    let tools = dir.path().join("tools");
    std::fs::create_dir(&tools).unwrap();
    let dirname = ["/usr/bin/dirname", "/bin/dirname"]
        .into_iter()
        .find(|path| Path::new(path).exists())
        .unwrap();
    std::os::unix::fs::symlink(dirname, tools.join("dirname")).unwrap();
    let out = out(std::process::Command::new("/bin/sh")
        .arg(wiring.bin.join("gh"))
        .arg("api")
        .env_clear()
        .env(
            "PATH",
            format!("{}:{}", wiring.bin.display(), tools.display()),
        ));
    assert_eq!(out.status.code(), Some(127));
    assert_eq!(
        String::from_utf8_lossy(&out.stderr).trim(),
        "purlis: no gh is installed on this chat's PATH"
    );
}

/// The table is held to the `gh` on the machine where there is one: each subcommand of it has
/// `--repo`. (A subcommand missing from the table is the other way a chat meets *"none of the git
/// remotes configured…"*, which the design says is a test failure and not a message.)
#[cfg(unix)]
#[test]
fn every_subcommand_in_the_table_takes_a_repo_flag_in_the_gh_on_this_machine() {
    // No gh here: the table cannot be held to it, and CI's runners have one.
    if crate::forklock::output(std::process::Command::new("gh").arg("--version")).is_err() {
        return;
    }
    for (group, subs) in TAKES_REPO {
        for sub in *subs {
            let out = out(std::process::Command::new("gh")
                .args([group, sub, "--help"])
                .env("GH_CONFIG_DIR", "/nonexistent"));
            if !out.status.success() {
                // A subcommand this gh does not have yet: nothing to hold the table to.
                continue;
            }
            let text = String::from_utf8_lossy(&out.stdout);
            assert!(
                text.contains("--repo"),
                "gh {group} {sub} has no --repo:\n{text}"
            );
        }
    }
}

#[test]
fn the_apps_git_for_a_chat_goes_through_the_chats_proxy_with_its_sentinel_and_no_login_of_the_persons()
 {
    let held = tempfile::tempdir().unwrap();
    let dir = held.path();
    let github = repo("github.com", "acme", "app", ForgeKind::GitHub);
    let network = git_network(&route(dir, vec![github])).expect("a route for git");
    let held = |network: &crate::worktree::git::NetworkRoute| format!("{network:?}");
    let shown = held(&network);
    // The proxy and the sentinel are handed by name, the sentinel to the helper alone.
    assert!(
        shown.contains("HTTPS_PROXY") && shown.contains("http://127.0.0.1:43001"),
        "{shown}"
    );
    assert!(shown.contains(SENTINEL_ENV), "{shown}");
    // The helper is purlis's own, named for the recorded host, after a reset; the chat's authority
    // is the one trusted for it; and the forge hosts are each proxied.
    assert!(shown.contains("credential.helper="), "{shown}");
    assert!(
        shown.contains("credential.https://github.com.helper="),
        "{shown}"
    );
    assert!(shown.contains(&SENTINEL_HELPER_ARG.to_string()), "{shown}");
    assert!(
        shown.contains(&format!(
            "http.https://github.com/.sslCAInfo={}",
            dir.join("ca.pem").display()
        )),
        "{shown}"
    );
    assert!(
        shown.contains("http.https://github.com/.proxy=http://127.0.0.1:43001"),
        "{shown}"
    );
    // No forge CLI login is named anywhere.
    for cli in ["gh auth", "glab auth", "git-credential"] {
        assert!(!shown.contains(cli), "{cli} is named: {shown}");
    }
}

#[test]
fn a_route_git_cannot_use_is_refused_and_nothing_is_composed() {
    let held = tempfile::tempdir().unwrap();
    let dir = held.path();
    let mut off_loopback = route(dir, vec![]);
    off_loopback.egress_url = "http://10.0.0.1:43001".to_owned();
    assert!(git_network(&off_loopback).is_err());
    let mut bad_sentinel = route(dir, vec![]);
    bad_sentinel.sentinel = SecretString::from("has a space");
    assert!(git_network(&bad_sentinel).is_err());
}

#[test]
fn the_ca_is_copied_into_the_chat_s_own_wire_folder_so_git_and_glab_never_read_the_held_route_folder()
 {
    let dir = tempfile::tempdir().unwrap();
    let route = route(
        dir.path(),
        vec![repo("github.com", "acme", "app", ForgeKind::GitHub)],
    );
    let wiring = compose(&route, dir.path()).unwrap();
    let copy = wiring.bin.parent().unwrap().join(CA_COPY);
    assert_eq!(
        std::fs::read_to_string(&copy).unwrap(),
        std::fs::read_to_string(&route.ca_cert).unwrap()
    );
    assert_eq!(get(&wiring.env, "GLAB_CA_CERT"), copy.to_str());
    assert!(
        wiring
            .git_pairs
            .iter()
            .filter(|(k, _)| k.ends_with(".sslCAInfo"))
            .all(|(_, v)| v == copy.to_str().unwrap()),
        "{:?}",
        wiring.git_pairs
    );
    assert!(
        !wiring
            .env
            .iter()
            .any(|(_, v)| v == route.ca_cert.to_str().unwrap())
    );
}

#[test]
fn the_route_folder_is_one_answer_for_a_stale_runtime_directory_and_an_unset_tmpdir() {
    let who = purlis_same_user::Uid::effective().as_raw().to_string();
    let home = tempfile::tempdir().expect("a home");
    let machine = |os, vars: &[(&str, &str)]| crate::sandbox::Machine {
        env: crate::secrets::Env::of(vars),
        home: Some(home.path().to_path_buf()),
        os,
    };
    // A stale XDG_RUNTIME_DIR (a folder that is not there) is the machine's own folder, as
    // `route_dir_in` answers it for the same inputs: the app and the sandbox cannot differ.
    let stale = machine(
        crate::sandbox::Os::Linux,
        &[("XDG_RUNTIME_DIR", "/run/user/not-a-folder-4417")],
    );
    let config = crate::machine::rooted(None, None, Some(home.path().to_path_buf()));
    assert_eq!(
        route_root(&stale),
        route_dir_in(None, config, PathBuf::from("/tmp"), &who, true)
    );
    // macOS with TMPDIR unset: `/tmp`, never a folder one side invented.
    let mac = machine(crate::sandbox::Os::MacOs, &[]);
    assert_eq!(
        route_root(&mac),
        Some(PathBuf::from("/tmp").join(format!("pf-{who}")))
    );
}

#[test]
fn a_repo_is_asked_by_its_own_https_address_and_no_other_spelling() {
    let own = repo("github.com", "acme", "app", ForgeKind::GitHub);
    for url in [
        "https://github.com/acme/app",
        "https://github.com/acme/app.git",
        "https://github.com/acme/app/",
        "https://GitHub.com/acme/app.git",
        // The wire's repo is lower-cased; the address a person's git has is not.
        "https://github.com/Acme/App.git",
        "https://GITHUB.com/ACME/APP/",
    ] {
        assert!(own.is_url(url), "{url}");
    }
    for url in [
        "",
        "https://github.com/acme/app2",
        "https://github.com/acme/app.git.git",
        "https://github.com/acme/app/extra",
        "https://github.com/acme/other",
        "https://github.com.evil.example/acme/app",
        "https://evil.example/github.com/acme/app",
        "https://user@github.com/acme/app",
        "https://github.com:8443/acme/app",
        "https://github.com/acme/app?x=1",
        "https://github.com/acme/app#x",
        "http://github.com/acme/app",
        "git@github.com:acme/app.git",
        "ssh://git@github.com/acme/app.git",
    ] {
        assert!(!own.is_url(url), "{url}");
    }
    let lab = repo("gitlab.com", "group/sub", "proj", ForgeKind::GitLab);
    assert!(lab.is_url("https://gitlab.com/group/sub/proj.git"));
    assert!(!lab.is_url("https://gitlab.com/group/proj.git"));
}
