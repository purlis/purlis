// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use std::net::IpAddr;
use std::path::Path;

use super::*;

fn read(text: &str) -> Profile {
    Profile::parse(text).expect("a profile purlis writes")
}

fn write(path: &str) -> Op<'_> {
    Op::File {
        what: FileOp::WriteData,
        path: Path::new(path),
        kind: Kind::Regular,
    }
}

fn create(path: &str, kind: Kind) -> Op<'_> {
    Op::File {
        what: FileOp::Create,
        path: Path::new(path),
        kind,
    }
}

fn reads(path: &str) -> Op<'_> {
    Op::File {
        what: FileOp::ReadData,
        path: Path::new(path),
        kind: Kind::Regular,
    }
}

fn tcp(ip: &str, port: u16) -> Op<'static> {
    Op::Connect(Remote::Ip {
        ip: ip.parse::<IpAddr>().expect("an address"),
        port,
    })
}

const FULL: &str = r#"(version 1)
(deny default)
(allow process-exec)
(allow signal (target same-sandbox))
(allow file-read*)
(allow mach-lookup
  (global-name "com.apple.logd"))
(allow network-outbound (remote ip "localhost:4100"))
(allow network-outbound (remote unix-socket (path-literal "/run/purlis/hooks.sock")))
(allow file-write*
  (subpath "/home/u/p/w")
  (subpath "/tmp/chat")
  (literal "/dev/null"))
(deny file-read* file-write* (subpath "/home/u/p/.purlis/vaults"))
(deny file-write* (literal "/home/u/p/w"))
(deny file-write* (regex "^/home/u/p/w/(.*/)?\\.git/hooks(/.*)?$"))
(deny file-write-create (require-all (vnode-type DIRECTORY) (regex #"^/home/u/p/w/(.*/)?\.git/modules/.+")))
"#;

#[test]
fn a_write_inside_a_granted_folder_is_allowed_and_one_outside_is_not() {
    let profile = read(FULL);
    assert!(profile.allows(&write("/home/u/p/w/src/main.rs")));
    assert!(profile.allows(&write("/tmp/chat/x")));
    assert!(!profile.allows(&write("/home/u/p/notes.md")));
    assert!(
        !profile.allows(&write("/home/u/p/wx")),
        "a sibling sharing a prefix"
    );
    assert!(!profile.allows(&write("/etc/passwd")));
}

#[test]
fn the_last_rule_that_matches_decides() {
    let profile = read(FULL);
    assert!(profile.allows(&reads("/etc/passwd")));
    assert!(!profile.allows(&reads("/home/u/p/.purlis/vaults/ops.json")));
    assert!(!profile.allows(&reads("/home/u/p/.purlis/vaults")));
    assert!(profile.allows(&reads("/home/u/p/.purlis/vaultsx")));
}

#[test]
fn a_protected_name_is_held_at_any_depth_of_a_written_folder() {
    let profile = read(FULL);
    assert!(!profile.allows(&create(
        "/home/u/p/w/a/b/.git/hooks/pre-commit",
        Kind::Regular
    )));
    assert!(!profile.allows(&create("/home/u/p/w/.git/hooks", Kind::Directory)));
    assert!(profile.allows(&create("/home/u/p/w/a/.git/hookz", Kind::Regular)));
}

#[test]
fn a_folder_pinned_as_an_entry_is_not_written_but_what_is_in_it_is() {
    let profile = read(FULL);
    let unlink = Op::File {
        what: FileOp::Unlink,
        path: Path::new("/home/u/p/w"),
        kind: Kind::Directory,
    };
    assert!(!profile.allows(&unlink));
    assert!(profile.allows(&create("/home/u/p/w/new", Kind::Regular)));
}

#[test]
fn a_rule_on_one_kind_of_entry_leaves_the_others() {
    let profile = read(FULL);
    assert!(!profile.allows(&create("/home/u/p/w/.git/modules/x", Kind::Directory)));
    assert!(profile.allows(&create("/home/u/p/w/.git/modules/x", Kind::Regular)));
}

#[test]
fn only_the_proxy_port_on_this_machine_is_reached() {
    let profile = read(FULL);
    assert!(profile.allows(&tcp("127.0.0.1", 4100)));
    assert!(profile.allows(&tcp("::1", 4100)));
    assert!(!profile.allows(&tcp("127.0.0.1", 4101)));
    assert!(
        !profile.allows(&tcp("93.184.216.34", 4100)),
        "another machine on the proxy's port"
    );
    assert!(!profile.allows(&tcp("::ffff:93.184.216.34", 4100)));
}

#[test]
fn only_the_named_unix_socket_is_reached() {
    let profile = read(FULL);
    let unix = |path: Option<&'static str>| Op::Connect(Remote::Unix(path.map(Path::new)));
    assert!(profile.allows(&unix(Some("/run/purlis/hooks.sock"))));
    assert!(!profile.allows(&unix(Some("/run/user/1000/bus"))));
    assert!(!profile.allows(&unix(None)), "an abstract socket");
}

#[test]
fn the_floor_reaches_other_machines_but_no_port_or_socket_of_this_one() {
    let profile = read(
        r#"(version 1)
(allow default)
(deny network-outbound (remote unix-socket))
(deny network-outbound (remote ip "localhost:*"))
(allow network-outbound (remote ip "localhost:4100"))
(deny file-write* (regex #"^/(.*/)?\.git/hooks(/.*)?$"))
"#,
    );
    assert!(profile.allows(&tcp("93.184.216.34", 443)));
    assert!(profile.allows(&tcp("127.0.0.1", 4100)));
    assert!(!profile.allows(&tcp("127.0.0.1", 22)));
    assert!(!profile.allows(&tcp("127.8.0.1", 22)));
    assert!(!profile.allows(&Op::Connect(Remote::Unix(Some(Path::new("/x"))))));
    assert!(profile.allows(&write("/etc/x")));
    assert!(!profile.allows(&create("/srv/r/.git/hooks/post-merge", Kind::Regular)));
}

#[test]
fn a_rule_held_but_inside_an_exception_is_given_back() {
    let profile = read(
        r#"(version 1)
(deny default)
(allow file-read*)
(deny file-read* file-write* (require-all (subpath "/h/.codex") (require-not (subpath "/h/.codex/packages"))))
"#,
    );
    assert!(!profile.allows(&reads("/h/.codex/auth.json")));
    assert!(profile.allows(&reads("/h/.codex/packages/codex")));
}

#[test]
fn a_rule_purlis_did_not_expect_refuses_the_whole_profile() {
    for text in [
        "(version 1)\n(allow file-write* (path-regex \"x\"))\n",
        "(version 1)\n(allow file-write-everything (subpath \"/x\"))\n",
        "(version 1)\n(import \"system.sb\")\n",
        "(version 1)\n(allow file-write* (subpath \"/x\")\n",
        "(version 1)\n(allow network-outbound (remote tcp \"*:443\"))\n",
    ] {
        assert!(Profile::parse(text).is_err(), "read: {text}");
    }
}

#[test]
fn rules_for_what_linux_has_no_counterpart_of_are_left_aside() {
    let profile = read(
        r#"(version 1)
(deny default)
(allow mach-lookup (global-name "com.apple.trustd.agent"))
(allow ipc-posix-shm-read* (ipc-posix-name-prefix "apple.cfprefs."))
(allow file-ioctl (literal "/dev/ptmx") (regex #"^/dev/ttys[0-9]+$"))
(deny process-exec (literal "/bin/launchctl"))
(deny appleevent-send)
(allow file-read*)
"#,
    );
    assert!(profile.allows(&reads("/x")));
    assert!(!profile.allows(&write("/x")));
}

#[test]
fn a_quoted_string_keeps_its_escaped_characters() {
    let profile =
        read("(version 1)\n(deny default)\n(allow file-write* (literal \"/a \\\"b\\\" \\\\c\"))\n");
    assert!(profile.allows(&write("/a \"b\" \\c")));
}

#[test]
fn what_a_profile_writes_and_reaches_is_listed_for_the_kernel_s_own_rules() {
    let profile = read(FULL);
    let reach = profile.reach();
    assert_eq!(reach.ports, vec![4100]);
    assert!(!reach.any_host);
    assert!(
        reach
            .written
            .contains(&Path::new("/home/u/p/w").to_path_buf())
    );
    assert!(
        reach
            .written
            .contains(&Path::new("/tmp/chat").to_path_buf())
    );
    let floor = read("(version 1)\n(allow default)\n");
    assert!(floor.reach().any_host);
    assert_eq!(floor.reach().written, vec![Path::new("/").to_path_buf()]);
    let regex = read(
        "(version 1)\n(deny default)\n(allow file-write* (regex \"^/d/opencode/log/.+$\") (regex \"^/d/opencode\\\\.db(-wal)?$\"))\n",
    );
    assert_eq!(
        regex.reach().written,
        vec![
            Path::new("/d/opencode/log").to_path_buf(),
            Path::new("/d").to_path_buf()
        ]
    );
}

/// #2019: a chat may bind the one socket its profile names to serve on, and no other; the
/// socket is listed for the supervisor's own rules.
#[test]
fn a_chat_binds_the_one_socket_it_serves_on() {
    let socket = "/data/serve/chat-1/5173.sock";
    let profile = read(&format!(
        "{FULL}(allow network-bind (local unix-socket (path-literal \"{socket}\")))\n\
         (allow network-inbound (local unix-socket (path-literal \"{socket}\")))\n\
         (allow file-write-create file-write-unlink (literal \"{socket}\"))\n"
    ));
    assert!(profile.allows(&Op::BindUnix(Path::new(socket))));
    assert!(!profile.allows(&Op::BindUnix(Path::new("/data/serve/chat-1/5174.sock"))));
    assert!(!profile.allows(&Op::BindUnix(Path::new("/tmp/other.sock"))));
    assert_eq!(
        profile.reach().serves,
        vec![Path::new(socket).to_path_buf()]
    );
    assert!(!read(FULL).allows(&Op::BindUnix(Path::new(socket))));
    for text in [
        "(version 1)\n(allow network-bind (local unix-socket (subpath \"/data\")))\n",
        "(version 1)\n(allow network-bind (local unix-socket (path-literal \"relative.sock\")))\n",
        "(version 1)\n(allow network-bind (local tcp \"localhost:5173\"))\n",
    ] {
        assert!(Profile::parse(text).is_err(), "read: {text}");
    }
}
