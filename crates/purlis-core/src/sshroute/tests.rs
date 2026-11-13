// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use std::path::Path;

use super::*;

const KEY: &str =
    "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAINiGzfjDDJsgYLYCmdkf8Fel9sE6Ma14Y2+c33Y/qZ2n";

fn key() -> HostKey {
    HostKey::parse(KEY).unwrap()
}

/// The value given to `-o name=…` in `args`, where there is one.
fn option<'a>(args: &'a [String], name: &str) -> Option<&'a str> {
    args.windows(2).find_map(|two| {
        (two[0] == "-o")
            .then(|| two[1].strip_prefix(&format!("{name}=")))
            .flatten()
    })
}

#[test]
fn a_route_is_read_and_spelled_back_as_docker_spells_it() {
    for text in [
        "ssh://build-box",
        "ssh://me@build-box",
        "ssh://me@build-box:2222",
        "ssh://me@build-box:2222/var/run/docker.sock",
        "ssh://me@[fd00::1]:2222/run/user/1000/docker.sock",
        "ssh://box/run/podman/podman.sock",
    ] {
        assert_eq!(Route::parse(text).unwrap().spelled(), text);
    }
    let route = Route::parse("ssh://me@build-box:2222/var/run/docker.sock").unwrap();
    assert_eq!(route.user(), Some("me"));
    assert_eq!(route.host(), "build-box");
    assert_eq!(route.port(), Some(2222));
    assert_eq!(route.socket(), Some("/var/run/docker.sock"));
}

#[test]
fn plain_tcp_is_refused_with_the_reason_and_the_ways_forward() {
    let why = Route::parse("tcp://build-box:2375").unwrap_err();
    assert_eq!(why, PLAIN_TCP);
    assert!(why.contains("anyone who can reach that port"), "{why}");
    assert!(why.contains("ssh://") && why.contains("TLS"), "{why}");
}

#[test]
fn a_host_or_user_ssh_could_take_for_an_option_is_refused() {
    for text in [
        "ssh://-oProxyCommand=evil",
        "ssh://-oProxyCommand=evil@box",
        "ssh://me@-box",
        "ssh://me@box lan",
        "ssh://me@box;rm",
        "ssh://",
        "ssh://me@box:0",
        "ssh://me@box:99999",
        "ssh://me@box/relative sock",
        "ssh://me@box/var/run/a:b.sock",
        "ssh://me@box/var/../etc/shadow",
        "ssh://me@box/var/run/$(id).sock",
    ] {
        assert!(Route::parse(text).is_err(), "{text} was read");
    }
}

#[test]
fn a_socket_found_at_connect_is_written_into_the_route() {
    let route = Route::parse("ssh://me@box").unwrap();
    let found = route.with_socket("/run/user/1000/docker.sock").unwrap();
    assert_eq!(found.spelled(), "ssh://me@box/run/user/1000/docker.sock");
    assert!(
        Route::parse("ssh://me@box")
            .unwrap()
            .with_socket("not/absolute")
            .is_err()
    );
}

#[test]
fn a_host_key_is_read_with_its_fingerprint_as_ssh_keygen_prints_it() {
    let key = HostKey::parse(&format!("{KEY} someone@somewhere")).unwrap();
    assert_eq!(key.spelled(), KEY);
    assert_eq!(key.algorithm(), "ssh-ed25519");
    // `ssh-keygen -lf` of the same key.
    assert_eq!(
        key.fingerprint(),
        "SHA256:dmeCC9oOb+c7VYVBSUAdgQchZHZQyXKWgKnGQxfh2rA"
    );
}

#[test]
fn a_key_whose_blob_names_another_algorithm_is_no_key() {
    let (_, blob) = KEY.split_once(' ').unwrap();
    assert!(HostKey::parse(&format!("ssh-rsa {blob}")).is_err());
    assert!(HostKey::parse("ssh-ed25519 not-base64!").is_err());
    assert!(HostKey::parse("ssh-ed25519").is_err());
}

#[test]
fn the_forward_trusts_the_pinned_key_alone_under_the_pools_alias() {
    let route = Route::parse("ssh://me@box:2222/var/run/docker.sock").unwrap();
    let args = forward_args(
        &route,
        "build",
        Path::new("/data/docker-settings/run-x/known_hosts"),
        Path::new("/data/engine-links/build/docker.sock"),
    )
    .unwrap();
    // Held by a pipe from purlis, never left behind (`-N` would outlive it): see HELD_BY.
    assert_eq!(args[0], "-T");
    assert!(!args.contains(&"-N".to_owned()));
    assert_eq!(option(&args, "StrictHostKeyChecking"), Some("yes"));
    assert_eq!(
        option(&args, "UserKnownHostsFile"),
        Some("/data/docker-settings/run-x/known_hosts")
    );
    assert_eq!(option(&args, "GlobalKnownHostsFile"), Some("/dev/null"));
    assert_eq!(option(&args, "KnownHostsCommand"), Some("none"));
    assert_eq!(option(&args, "VerifyHostKeyDNS"), Some("no"));
    assert_eq!(option(&args, "UpdateHostKeys"), Some("no"));
    assert_eq!(option(&args, "HostKeyAlias"), Some("purlis-pool-build"));
    assert_eq!(
        known_hosts("build", &key()),
        format!("purlis-pool-build {KEY}\n")
    );
}

#[test]
fn the_forward_shares_no_connection_asks_nothing_and_hands_nothing_on() {
    let route = Route::parse("ssh://me@box/var/run/docker.sock").unwrap();
    let args = forward_args(&route, "build", Path::new("/k"), Path::new("/s")).unwrap();
    for (name, value) in [
        ("ControlMaster", "no"),
        ("ControlPath", "none"),
        ("BatchMode", "yes"),
        ("PasswordAuthentication", "no"),
        ("KbdInteractiveAuthentication", "no"),
        ("ForwardAgent", "no"),
        ("ForwardX11", "no"),
        ("PermitLocalCommand", "no"),
        ("ExitOnForwardFailure", "yes"),
        ("StreamLocalBindUnlink", "yes"),
        ("StreamLocalBindMask", "0177"),
    ] {
        assert_eq!(option(&args, name), Some(value), "{name}");
    }
}

#[test]
fn a_lost_connection_is_noticed_within_a_minute() {
    let route = Route::parse("ssh://me@box/var/run/docker.sock").unwrap();
    let args = forward_args(&route, "build", Path::new("/k"), Path::new("/s")).unwrap();
    let every: u32 = option(&args, "ServerAliveInterval")
        .unwrap()
        .parse()
        .unwrap();
    let count: u32 = option(&args, "ServerAliveCountMax")
        .unwrap()
        .parse()
        .unwrap();
    assert!(every * count <= 60, "{every} × {count}");
}

#[test]
fn the_forward_ends_with_the_socket_pair_and_the_destination_after_its_options() {
    let route = Route::parse("ssh://me@box:2222/run/user/1000/docker.sock").unwrap();
    let args = forward_args(
        &route,
        "build",
        Path::new("/k"),
        Path::new("/data/engine-links/build/docker.sock"),
    )
    .unwrap();
    let tail = &args[args.len() - 7..];
    assert_eq!(
        tail,
        [
            "-p",
            "2222",
            "-L",
            "/data/engine-links/build/docker.sock:/run/user/1000/docker.sock",
            "--",
            "me@box",
            HELD_BY,
        ]
    );
}

#[test]
fn a_route_with_no_socket_yet_opens_no_forward() {
    let route = Route::parse("ssh://me@box").unwrap();
    assert!(forward_args(&route, "build", Path::new("/k"), Path::new("/s")).is_err());
}

#[test]
fn a_first_contact_learns_the_key_and_signs_in_with_nothing() {
    let route = Route::parse("ssh://me@box").unwrap();
    let args = contact_args(&route, Path::new("/run/known_hosts"));
    assert_eq!(option(&args, "StrictHostKeyChecking"), Some("accept-new"));
    assert_eq!(
        option(&args, "UserKnownHostsFile"),
        Some("/run/known_hosts")
    );
    assert_eq!(option(&args, "PubkeyAuthentication"), Some("no"));
    // A system's ssh_config may hash names (Debian's and Ubuntu's do): the key is found by its
    // alias only when it is written plainly.
    assert_eq!(option(&args, "HashKnownHosts"), Some("no"));
    assert_eq!(option(&args, "PasswordAuthentication"), Some("no"));
    assert_eq!(option(&args, "KbdInteractiveAuthentication"), Some("no"));
    assert_eq!(option(&args, "ForwardAgent"), Some("no"));
    assert_eq!(&args[args.len() - 3..], ["--", "me@box", "true"]);
}

#[test]
fn the_key_a_first_contact_met_is_read_from_what_ssh_wrote() {
    let written = format!("purlis-first-contact {KEY}\n");
    assert_eq!(key_met(&written), Some(key()));
    assert_eq!(key_met(""), None);
    assert_eq!(key_met(&format!("another-host {KEY}\n")), None);
}

#[test]
fn the_probe_runs_pinned_and_checks_only_the_socket_a_route_names() {
    let found = Route::parse("ssh://me@box").unwrap();
    let args = probe_args(&found, "build", Path::new("/k"));
    assert_eq!(option(&args, "StrictHostKeyChecking"), Some("yes"));
    assert_eq!(args.last().unwrap(), PROBE);
    let named = Route::parse("ssh://me@box/run/podman/podman.sock").unwrap();
    let args = probe_args(&named, "build", Path::new("/k"));
    assert!(
        args.last()
            .unwrap()
            .starts_with("s=/run/podman/podman.sock;")
    );
}

#[test]
fn the_probe_looks_where_docker_and_podman_keep_their_sockets_in_order() {
    let at = |needle: &str| PROBE.find(needle).unwrap();
    assert!(at("/var/run/docker.sock") < at("$r/docker.sock"));
    assert!(at("$r/docker.sock") < at("/run/podman/podman.sock"));
    assert!(at("/run/podman/podman.sock") < at("$r/podman/podman.sock"));
}

#[test]
fn a_socket_path_too_long_for_this_system_is_refused_with_a_shorter_name_to_choose() {
    let long = Path::new("/a").join("b".repeat(80));
    let why = socket_of(&long, "a-pool-with-a-rather-long-name").unwrap_err();
    assert!(why.contains("too long"), "{why}");
    assert!(socket_of(Path::new("/data/engine-links"), "build").is_ok());
}

#[test]
fn ssh_settings_are_read_from_what_ssh_g_prints() {
    let printed = "user me\nhostname 10.0.0.5\nport 2222\nproxyjump none\n\
                   userknownhostsfile ~/.ssh/known_hosts ~/.ssh/known_hosts2\n";
    let resolved = connect::resolved_from(printed).unwrap();
    assert_eq!(resolved.hostname, "10.0.0.5");
    assert_eq!(resolved.port, 2222);
    assert_eq!(resolved.user, "me");
    assert!(!resolved.proxied);
    assert_eq!(resolved.known_hosts.len(), 2);
    let jumped = connect::resolved_from("hostname box\nproxyjump bastion\n").unwrap();
    assert!(jumped.proxied);
}

#[test]
fn the_machines_ssh_settings_name_are_offered_once_each_and_patterns_never() {
    let config = "Host build-box gpu\n  User me\nHost *.internal !bad\nhost=nas\n\
                  Host build-box\nHost -oProxyCommand=x\nMatch host foo\n";
    assert_eq!(hosts_in(config), ["build-box", "gpu", "nas"]);
}

#[test]
fn the_forward_takes_no_command_tunnel_or_forward_clearing_from_the_persons_settings() {
    let route = Route::parse("ssh://me@box/var/run/docker.sock").unwrap();
    let args = forward_args(&route, "build", Path::new("/k"), Path::new("/s")).unwrap();
    // On ssh's command line the first value wins over every ssh_config's.
    for (name, value) in [
        ("RemoteCommand", "none"),
        ("Tunnel", "no"),
        ("RequestTTY", "no"),
        ("PermitLocalCommand", "no"),
        // `yes` would clear the forward purlis asks for too.
        ("ClearAllForwardings", "no"),
    ] {
        assert_eq!(option(&args, name), Some(value), "{name}");
    }
    assert_eq!(args.iter().filter(|arg| *arg == "-L").count(), 1);
    assert!(
        !args
            .iter()
            .any(|arg| arg == "-R" || arg == "-D" || arg == "-w")
    );
}

#[test]
fn a_sign_in_a_probe_and_a_first_contact_forward_nothing_and_run_only_their_command() {
    let route = Route::parse("ssh://me@box").unwrap();
    for args in [
        sign_in_args(&route, "build", Path::new("/k")),
        probe_args(&route, "build", Path::new("/k")),
        contact_args(&route, Path::new("/scratch")),
    ] {
        assert_eq!(option(&args, "ClearAllForwardings"), Some("yes"));
        assert_eq!(option(&args, "RemoteCommand"), Some("none"));
        assert_eq!(option(&args, "Tunnel"), Some("no"));
    }
}

#[test]
fn settings_that_would_ride_on_the_forward_are_named_from_what_ssh_g_prints() {
    let clean = "user me\nhostname box\ntunnel false\nsessiontype default\nstdinnull no\n\
                 forkafterauthentication no\nclearallforwardings no\n";
    assert!(connect::not_carried(clean).is_empty());
    let carrying = "user me\nlocalforward [localhost]:8080 [localhost]:80\n\
                    remoteforward [localhost]:9000 [localhost]:22\ndynamicforward [localhost]:1080\n\
                    tunnel point-to-point\nsessiontype none\nstdinnull yes\n\
                    forkafterauthentication yes\n";
    assert_eq!(
        connect::not_carried(carrying),
        [
            "LocalForward",
            "RemoteForward",
            "DynamicForward",
            "Tunnel",
            "SessionType",
            "StdinNull",
            "ForkAfterAuthentication",
        ]
    );
    // Named once, however many lines.
    assert_eq!(
        connect::not_carried("localforward 1 2\nlocalforward 3 4\n"),
        ["LocalForward"]
    );
}

#[test]
fn the_unspecified_address_is_no_machine_to_reach() {
    for route in [
        "ssh://me@[::]",
        "ssh://me@[::]:22",
        "ssh://me@::",
        "ssh://me@[0:0:0:0:0:0:0:0]",
    ] {
        assert!(Route::parse(route).is_err(), "{route}");
    }
    // Nor is a string of colons that is no address.
    assert!(Route::parse("ssh://me@[:::]").is_err());
    assert!(Route::parse("ssh://me@[1::2::3]").is_err());
    let loopback = Route::parse("ssh://me@[::1]:2222").unwrap();
    assert_eq!(loopback.host(), "::1");
    assert!(Route::parse("ssh://me@[fe80::1]").is_ok());
}

#[test]
fn ssh_keygen_is_the_one_path_names() {
    let dir = tempfile::tempdir().unwrap();
    let keygen = dir.path().join("ssh-keygen");
    std::fs::write(&keygen, "#!/bin/sh\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(&keygen, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    let path = dir.path().display().to_string();
    assert_eq!(connect::keygen_on(Some(&path)), Some(keygen));
    let empty = tempfile::tempdir().unwrap();
    assert_eq!(
        connect::keygen_on(Some(&empty.path().display().to_string())),
        None
    );
}

#[test]
fn the_forwards_session_ends_the_whole_connection_once_purlis_lets_go() {
    // It waits on purlis's pipe, then ends the session's own sshd process, which takes every
    // channel the connection holds with it (a `docker exec` too), so ssh here ends.
    assert!(HELD_BY.contains("read line"), "{HELD_BY}");
    assert!(HELD_BY.contains("kill -HUP \"$PPID\""), "{HELD_BY}");
    assert!(HELD_BY.starts_with("exec sh -c '"), "{HELD_BY}");
}

// --- Podman over SSH (#6269) ------------------------------------------------------------------

/// What `podman system connection list --format json` prints, as Podman 4 and 5 write it.
const CONNECTIONS: &str = r#"[
  {"Name": "build-box", "URI": "ssh://me@build-box:22/run/user/1000/podman/podman.sock",
   "Identity": "/home/me/.ssh/id_ed25519", "IsMachine": false, "Default": true, "ReadWrite": true},
  {"Name": "podman-machine-default", "URI": "ssh://core@127.0.0.1:50123/run/user/501/podman/podman.sock",
   "Identity": "/home/me/.local/share/containers/podman/machine/machine", "IsMachine": true,
   "Default": false, "ReadWrite": true},
  {"Name": "rootful", "URI": "ssh://root@build-box/run/podman/podman.sock", "Identity": "",
   "IsMachine": false, "Default": false, "ReadWrite": true},
  {"Name": "here", "URI": "unix:///run/podman/podman.sock", "Identity": "", "IsMachine": false},
  {"Name": "odd-key", "URI": "ssh://me@build-box/run/podman/podman.sock",
   "Identity": "keys/%h", "IsMachine": false}
]"#;

#[test]
fn a_podman_connection_is_read_into_the_ssh_route_to_its_socket_with_its_key_file() {
    let read = podman_connections_in(CONNECTIONS);
    let names: Vec<&str> = read.iter().map(|one| one.name.as_str()).collect();
    // A Podman machine of this machine's own is this machine; a socket here is no route; a key
    // file ssh would read tokens in is no key file.
    assert_eq!(names, ["build-box", "rootful"]);
    let first = &read[0].route;
    assert_eq!(
        first.spelled(),
        "ssh://me@build-box:22/run/user/1000/podman/podman.sock"
    );
    assert_eq!(first.socket(), Some("/run/user/1000/podman/podman.sock"));
    assert_eq!(
        first.identity(),
        Some(Path::new("/home/me/.ssh/id_ed25519"))
    );
    assert_eq!(read[1].route.identity(), None);
    assert!(podman_connections_in("not json").is_empty());
}

#[test]
fn the_forward_check_asks_sshd_for_one_channel_to_the_engines_socket_pinned_and_runs_nothing() {
    let route = Route::parse("ssh://me@box/var/run/docker.sock").unwrap();
    let args = forward_check_args(&route, "build", Path::new("/k")).unwrap();
    assert_eq!(option(&args, "StrictHostKeyChecking"), Some("yes"));
    assert_eq!(option(&args, "UserKnownHostsFile"), Some("/k"));
    assert_eq!(option(&args, "ExitOnForwardFailure"), Some("yes"));
    assert_eq!(option(&args, "ClearAllForwardings"), Some("yes"));
    let w = args.iter().position(|arg| arg == "-W").unwrap();
    assert_eq!(args[w + 1], "/var/run/docker.sock");
    // The destination is the last word: `-W` runs no command there.
    assert_eq!(&args[args.len() - 2..], ["--", "me@box"]);
    assert!(
        forward_check_args(&Route::parse("ssh://me@box").unwrap(), "build", Path::new("/k"))
            .is_err(),
        "no socket, nothing to ask for"
    );
}

#[test]
fn a_routes_key_file_is_handed_to_every_ssh_it_runs() {
    let route = Route::parse("ssh://me@build-box:2222/run/user/1000/podman/podman.sock")
        .unwrap()
        .with_identity(Path::new("/home/me/.ssh/id_ed25519"))
        .unwrap();
    let identity = |args: &[String]| {
        args.windows(2)
            .find(|two| two[0] == "-i")
            .map(|two| two[1].clone())
    };
    let known = Path::new("/k/known_hosts");
    for args in [
        forward_args(&route, "p", known, Path::new("/l/docker.sock")).unwrap(),
        sign_in_args(&route, "p", known),
        probe_args(&route, "p", known),
        forward_check_args(&route, "p", known).unwrap(),
        contact_args(&route, Path::new("/k/scratch")),
        connect::settings_args(&route),
    ] {
        assert_eq!(
            identity(&args).as_deref(),
            Some("/home/me/.ssh/id_ed25519"),
            "{args:?}"
        );
        // Before the destination, where ssh reads it as an option.
        let at = args.iter().position(|arg| arg == "--").unwrap();
        assert!(args.iter().position(|arg| arg == "-i").unwrap() < at);
    }
    // Not part of Docker's spelling, which has no place for it.
    assert!(!route.spelled().contains("id_ed25519"));
}

#[test]
fn a_key_file_ssh_could_misread_is_refused() {
    let route = Route::parse("ssh://me@build-box").unwrap();
    for bad in [
        "relative/key",
        "/home/me/%d/key",
        // OpenSSH 8.4 and later expand an environment variable in a key file's path.
        "/home/${USER}/key",
        "${HOME}/.ssh/id_ed25519",
        "/home/me/key\nHost *",
        "",
    ] {
        assert!(
            route.clone().with_identity(Path::new(bad)).is_err(),
            "{bad:?}"
        );
    }
}

/// What rootless Podman's `podman info --format '{{json .}}'` says of itself, in part.
fn podman_info(socket: &str, graph_root: &str) -> serde_json::Value {
    serde_json::json!({
        "host": {"remoteSocket": {"exists": true, "path": socket}, "security": {"rootless": true}},
        "store": {"graphRoot": graph_root, "runRoot": "/run/user/1000/containers"},
        "version": {"Version": "4.9.3"}
    })
}

#[test]
fn a_podmans_pin_is_its_socket_and_graph_root_as_podman_info_says_them() {
    let pin = EnginePin::podman_in(&podman_info(
        "/run/user/1000/podman/podman.sock",
        "/home/me/.local/share/containers/storage",
    ))
    .expect("a Podman's")
    .expect("read");
    assert_eq!(
        pin,
        EnginePin::Podman {
            socket: "/run/user/1000/podman/podman.sock".into(),
            graph_root: "/home/me/.local/share/containers/storage".into(),
        }
    );
    // A remote client may name the socket by its URL.
    assert_eq!(
        EnginePin::podman_in(&podman_info("unix:///run/podman/podman.sock", "/var/lib/c"))
            .unwrap()
            .unwrap(),
        EnginePin::Podman {
            socket: "/run/podman/podman.sock".into(),
            graph_root: "/var/lib/c".into(),
        }
    );
    // Docker's info names no store: not a Podman's.
    assert!(EnginePin::podman_in(&serde_json::json!({"ID": "ABCD"})).is_none());
    // A Podman that doesn't say where, or says what no path is, is refused.
    assert!(
        EnginePin::podman_in(&podman_info("", "/var/lib/c"))
            .unwrap()
            .is_err()
    );
    assert!(
        EnginePin::podman_in(&podman_info("/run/podman/podman.sock", "relative"))
            .unwrap()
            .is_err()
    );
}

#[test]
fn a_different_graph_root_under_the_same_socket_is_another_engine() {
    let at = |graph_root: &str| EnginePin::Podman {
        socket: "/run/user/1000/podman/podman.sock".into(),
        graph_root: graph_root.into(),
    };
    assert_ne!(
        at("/home/me/.local/share/containers/storage"),
        at("/srv/other")
    );
    assert!(at("/srv/other").shown().contains("/srv/other"));
}

#[test]
fn a_pin_is_kept_as_its_engine_says_and_read_back_whole_or_not_at_all() {
    for engine in [
        EnginePin::Docker("ABCD:EFGH".into()),
        EnginePin::Podman {
            socket: "/run/user/1000/podman/podman.sock".into(),
            graph_root: "/home/me/.local/share/containers/storage".into(),
        },
    ] {
        let pin = Pin {
            host_key: key(),
            engine,
        };
        let kept = pin.to_json();
        assert_eq!(
            Pin::from_json(kept.as_object().unwrap()).as_ref(),
            Some(&pin)
        );
    }
    let read = |value: serde_json::Value| Pin::from_json(value.as_object().unwrap());
    // Both kinds at once, neither, a third key, or a Podman missing a half: not as purlis writes.
    assert!(read(serde_json::json!({"hostKey": KEY, "engine": "A", "podman": {}})).is_none());
    assert!(read(serde_json::json!({"hostKey": KEY})).is_none());
    assert!(read(serde_json::json!({"hostKey": KEY, "podman": {"socket": "/s"}})).is_none());
    assert!(
        read(serde_json::json!({"hostKey": KEY, "podman": {"socket": "/s", "graphRoot": "g"}}))
            .is_none()
    );
}

/// A file is kept there in a folder of its own beside the engine's socket, from standard input,
/// and only that folder is ever removed.
#[test]
fn a_file_is_kept_there_in_a_folder_of_its_own_and_only_that_folder_let_go() {
    let route = Route::parse("ssh://me@build-box/run/user/1000/podman/podman.sock").unwrap();
    let args = keep_args(
        &route,
        "p",
        Path::new("/k/known_hosts"),
        "/run/user/1000/podman",
        "runner-seccomp-engine.json",
    )
    .unwrap();
    assert_eq!(
        args.last().unwrap(),
        "umask 077 && d=$(mktemp -d -p /run/user/1000/podman purlis-kept.XXXXXXXX) && cat > \
         \"$d/runner-seccomp-engine.json\" && printf '%s\\n' \"$d/runner-seccomp-engine.json\""
    );
    assert_eq!(option(&args, "StrictHostKeyChecking"), Some("yes"));
    for (beside, name) in [
        ("/a b", "f.json"),
        ("relative", "f.json"),
        ("/a/../b", "f.json"),
        ("/run", "a/b"),
        ("/run", "$(x)"),
        ("/run", ".hidden"),
        ("/run", ""),
    ] {
        assert!(
            keep_args(&route, "p", Path::new("/k"), beside, name).is_err(),
            "{beside:?} {name:?}"
        );
    }
    let args = let_go_args(
        &route,
        "p",
        Path::new("/k"),
        "/run/user/1000/podman/purlis-kept.Ab12Cd34",
    )
    .unwrap();
    assert_eq!(
        args.last().unwrap(),
        "rm -rf -- /run/user/1000/podman/purlis-kept.Ab12Cd34"
    );
    for bad in [
        "/run/user/1000/podman",
        "/home/me",
        "/run/purlis-kept.",
        "/run/purlis-kept.a b",
        "purlis-kept.x",
    ] {
        assert!(
            let_go_args(&route, "p", Path::new("/k"), bad).is_err(),
            "{bad:?}"
        );
    }
}

/// Podman's init is looked for there with `test -f` alone, pinned, and only at a plain path.
#[test]
fn a_file_is_looked_for_there_only_at_a_plain_path() {
    let route = Route::parse("ssh://me@build-box/run/podman/podman.sock").unwrap();
    let args = is_file_args(
        &route,
        "p",
        Path::new("/k/known_hosts"),
        "/usr/libexec/podman/catatonit",
    )
    .unwrap();
    assert_eq!(
        args.last().unwrap(),
        "test -f /usr/libexec/podman/catatonit"
    );
    assert_eq!(option(&args, "StrictHostKeyChecking"), Some("yes"));
    for bad in ["/a b", "/a;rm", "relative", "/a/../b", "/a$(x)"] {
        assert!(
            is_file_args(&route, "p", Path::new("/k"), bad).is_err(),
            "{bad:?}"
        );
    }
}

#[test]
fn a_file_is_read_there_only_where_it_is_a_plain_file_at_a_plain_path() {
    let route = Route::parse("ssh://me@build-box/run/podman/podman.sock").unwrap();
    let args = read_args(
        &route,
        "p",
        Path::new("/k/known_hosts"),
        "/var/lib/c/overlay-containers/abc/userdata/config.json",
    )
    .unwrap();
    assert_eq!(
        args.last().unwrap(),
        "test -f /var/lib/c/overlay-containers/abc/userdata/config.json && exec cat -- \
         /var/lib/c/overlay-containers/abc/userdata/config.json"
    );
    // Pinned, as a sign-in is.
    assert_eq!(option(&args, "StrictHostKeyChecking"), Some("yes"));
    for bad in ["/a b", "/a;rm", "relative", "/a/../b"] {
        assert!(
            read_args(&route, "p", Path::new("/k"), bad).is_err(),
            "{bad:?}"
        );
    }
}

/// An `ssh` that ends with `w` when asked for a channel with `-W`, lets a sign-in (`true` there)
/// in, and ends a probe with `probed`.
#[cfg(unix)]
fn an_ssh_ending(folder: &Path, w: i32, probed: i32) -> std::path::PathBuf {
    use std::os::unix::fs::PermissionsExt as _;
    let ssh = folder.join("ssh");
    std::fs::write(
        &ssh,
        format!(
            "#!/bin/sh\nfor a in \"$@\"; do [ \"$a\" = -W ] && exit {w}; last=$a; done\n\
             [ \"$last\" = true ] && exit 0\nexit {probed}\n"
        ),
    )
    .unwrap();
    std::fs::set_permissions(&ssh, std::fs::Permissions::from_mode(0o755)).unwrap();
    ssh
}

#[cfg(unix)]
#[test]
fn a_forward_is_taken_as_refused_only_where_the_channel_fails_and_the_probe_holds() {
    let route = Route::parse("ssh://me@box/var/run/docker.sock").unwrap();
    for (w, probed, refused) in [
        // The server signs the person in, the socket is there, and the channel isn't opened.
        (255, 0, true),
        // ssh's 255 for a sign-in refused or a connection lost: not a refusal to forward.
        (255, 255, false),
        // A channel opened to a socket that isn't there any more: the engine's.
        (255, PROBE_NONE, false),
        (255, PROBE_NOT_ALLOWED, false),
        // The channel opened: whatever is wrong is the engine's.
        (0, 0, false),
        (1, 0, false),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let ssh = an_ssh_ending(dir.path(), w, probed);
        let (links, settings) = (dir.path().join("l"), dir.path().join("s"));
        let key = key();
        let opening = forward::Opening {
            ssh: &ssh,
            route: &route,
            pool: "build",
            key: &key,
            engine_links: &links,
            settings_in: &settings,
        };
        assert_eq!(
            connect::forwarding_refused(&opening),
            refused,
            "-W ended {w}, the probe {probed}"
        );
        assert_eq!(
            matches!(
                connect::silent_or_refused(&opening, "EOF".to_owned()),
                connect::Finding::ForwardingRefused { .. }
            ),
            refused
        );
    }
}
