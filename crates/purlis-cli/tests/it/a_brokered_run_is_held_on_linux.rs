// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A brokered `secret exec` for a sandboxed chat runs inside purlis's own wrap on Linux**
//! (#2342), as it runs inside Seatbelt on macOS.
//!
//! The app's half ([`brokered::serve`]) is handed a Codex chat's record as a start makes it,
//! at the full sandbox and at the floor, with one declared tool (`/bin/sh`, handed a credential
//! file) and this build's own `purlis` as the wrap the chat's start held. The tool tries what a
//! chat would: read its credential file, read the vault itself, write the chat's folder, and
//! write outside the project. The profile it runs under is the chat's own, applied by
//! `purlis _sandbox`.
//!
//! Never skipped on Linux: a kernel without Landlock ABI 6 fails here, saying so.

#![cfg(target_os = "linux")]

use crate::support;

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use base64::Engine as _;
use purlis_core::harness::Harness;
use purlis_core::sandbox::tools::{Handed, Ready, Tool};
use purlis_core::sandbox::{self, Decided, Level, Machine, Os};
use purlis_core::secrets::Env;
use purlis_core::secrets::brokered::{self, Asker, Frame, Wanted};

const KUBECONFIG: &str = "apiVersion: v1\nclusters: [k8s-held]\n";

const LEVELS: [(Level, &str, &str); 2] = [
    (Level::Full, "on", "[sandbox]\nmode = \"on\"\n"),
    (Level::Floor, "floor", "schema = 1\n"),
];

/// A project whose vault `team` is a plain file tagged for persona `devops`, with a folder
/// `work` for the chat.
fn lay_out(root: &Path, manifest: &str) {
    fs::create_dir_all(root.join("work")).expect("the chat's folder");
    fs::write(root.join("purlis.toml"), manifest).expect("the manifest");
    fs::write(
        root.join("vaults.json"),
        serde_json::json!({ "vaults": {
            "team": {"provider": "plain-file", "config": {"file": "team.json"}, "persona": "devops"},
        }})
        .to_string(),
    )
    .expect("the registry");
    fs::write(
        root.join("team.json"),
        serde_json::json!({ "KUBECONFIG": KUBECONFIG }).to_string(),
    )
    .expect("the vault");
}

/// Bytes written to a connection, kept where the test can read them.
#[derive(Clone, Default)]
struct Wire(Arc<Mutex<Vec<u8>>>);

impl Write for Wire {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().expect("the wire").extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn tries(what: &str) -> String {
    format!("if {what} 2>/dev/null; then echo yes; else echo no; fi")
}

/// The chat's run of `script` by its declared tool `/bin/sh`, handed the vault's `KUBECONFIG` as
/// a file: every frame the app wrote.
fn served(asker: &Asker, root: &Path, script: &str) -> Vec<Frame> {
    let wanted = Wanted {
        vault: "team".into(),
        file: vec!["KUBECONFIG=KUBECONFIG".into()],
        command: vec!["/bin/sh".into(), "-c".into(), script.into()],
        cwd: Some(root.join("work")),
        environment: vec![("PATH".into(), "/usr/bin:/bin".into())],
        ..Wanted::default()
    };
    // The asker's connection stays open until the run is done.
    let (ours, _theirs) = std::os::unix::net::UnixStream::pair().expect("a connection");
    let wire = Wire::default();
    brokered::serve(
        asker,
        wanted,
        Box::new(std::io::BufReader::new(ours)),
        Box::new(wire.clone()),
        Arc::new(|_| {}),
    );
    let text = String::from_utf8(wire.0.lock().expect("the wire").clone()).expect("text");
    text.lines()
        .map(|line| serde_json::from_str(line).expect("a frame"))
        .collect()
}

fn stdout(frames: &[Frame]) -> String {
    let mut out = Vec::new();
    for frame in frames {
        if let Frame::Stdout(bytes) = frame {
            out.extend(
                base64::engine::general_purpose::STANDARD
                    .decode(bytes)
                    .expect("base64"),
            );
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[test]
fn a_declared_tool_s_run_is_held_by_the_chat_s_own_profile_on_linux() {
    purlis_core::unsteered!();
    assert!(
        purlis_linux_sandbox::abi() >= purlis_linux_sandbox::MIN_ABI,
        "this kernel's Landlock is version {}, and purlis's Linux sandbox needs {} (Linux 6.12 or \
         later)",
        purlis_linux_sandbox::abi(),
        purlis_linux_sandbox::MIN_ABI
    );
    let scratch = tempfile::tempdir().expect("a folder");
    let out = scratch.path().canonicalize().expect("a real folder");
    let home = out.join("home");
    let bin = out.join("bin");
    let outside = out.join("outside");
    for dir in [&home, &bin, &outside] {
        fs::create_dir_all(dir).expect("a folder");
    }
    let machine = Machine {
        env: Env::of(&[("PATH", &bin.display().to_string())]),
        home: Some(home.clone()),
        os: Os::Linux,
    };
    for (level, name, manifest) in LEVELS {
        let root = out.join(name).join("project");
        lay_out(&root, manifest);
        let decided = sandbox::decide(Harness::Codex, &root, &machine, &|_| true, None, None)
            .expect("decided")
            .expect("a sandbox in force");
        let Decided::Sandboxed(mut applied) = decided else {
            panic!("not sandboxed: {decided:?}");
        };
        assert_eq!(applied.level(), level, "{name}");
        // The wrap the chat's start checked and held: this build's own `purlis`.
        applied.wrap_with(
            Path::new(support::PURLIS)
                .canonicalize()
                .expect("purlis's real path"),
        );
        let mut confines = applied.confines().clone();
        // The persona's tool, as the chat's start armed it.
        confines.tools.push(Ready {
            tool: Tool::one("sh", "team", "KUBECONFIG", Handed::File, "KUBECONFIG"),
            program: PathBuf::from("/bin/sh"),
        });
        let asker = Asker {
            root: root.clone(),
            env: Env::of(&[("PATH", "/usr/bin:/bin")]),
            chat: 7,
            persona: Some("devops".into()),
            folder: Some(root.join("work")),
            confines: Some(confines),
            harness: Some(Harness::Codex),
        };
        let script = [
            r#"head -c 10 "$KUBECONFIG"; echo"#.to_owned(),
            tries(&format!(
                "head -c 1 {} >/dev/null",
                root.join("team.json").display()
            )),
            tries("touch ran"),
            tries(&format!("touch {}/x", outside.display())),
        ]
        .join("; ");
        let frames = served(&asker, &root, &script);
        assert_eq!(frames.last(), Some(&Frame::Exit(0)), "{name}: {frames:?}");
        let written_outside = if level == Level::Floor { "yes" } else { "no" };
        assert_eq!(
            stdout(&frames),
            format!("apiVersion\nno\nyes\n{written_outside}\n"),
            "{name}: its credential file, the vault itself, the chat's folder, outside the \
             project: {frames:?}"
        );
        assert!(root.join("work/ran").exists(), "{name}: the folder written");
        let _ = fs::remove_file(outside.join("x"));
    }
}

/// A tool declared under a name its program's file does not have (#1977) runs, inside the
/// wrap, under that name: `unxz`, a link to xz, unpacks where `xz -c` would pack again.
#[test]
fn a_tool_declared_under_another_name_runs_under_it_inside_the_wrap() {
    purlis_core::unsteered!();
    assert!(
        purlis_linux_sandbox::abi() >= purlis_linux_sandbox::MIN_ABI,
        "this kernel's Landlock is version {}, and purlis's Linux sandbox needs {}",
        purlis_linux_sandbox::abi(),
        purlis_linux_sandbox::MIN_ABI
    );
    let Some(xz) = ["/usr/bin/xz", "/bin/xz"]
        .iter()
        .map(Path::new)
        .find(|path| path.is_file())
        .and_then(|path| path.canonicalize().ok())
    else {
        panic!("no xz on this machine");
    };
    let scratch = tempfile::tempdir().expect("a folder");
    let out = scratch.path().canonicalize().expect("a real folder");
    let home = out.join("home");
    let bin = out.join("bin");
    for dir in [&home, &bin] {
        fs::create_dir_all(dir).expect("a folder");
    }
    let machine = Machine {
        env: Env::of(&[("PATH", &bin.display().to_string())]),
        home: Some(home),
        os: Os::Linux,
    };
    let (_, _, manifest) = LEVELS[0];
    let root = out.join("project");
    lay_out(&root, manifest);
    let packed = {
        let mut child = support::command(&xz)
            .arg("-c")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .spawn()
            .expect("xz runs");
        child
            .stdin
            .take()
            .expect("a stdin")
            .write_all(b"unpacked\n")
            .expect("written");
        child.wait_with_output().expect("xz ends").stdout
    };
    let file = root.join("work/packed.xz");
    fs::write(&file, packed).expect("the packed file");
    let decided = sandbox::decide(Harness::Codex, &root, &machine, &|_| true, None, None)
        .expect("decided")
        .expect("a sandbox in force");
    let Decided::Sandboxed(mut applied) = decided else {
        panic!("not sandboxed: {decided:?}");
    };
    applied.wrap_with(
        Path::new(support::PURLIS)
            .canonicalize()
            .expect("purlis's real path"),
    );
    let mut confines = applied.confines().clone();
    confines.tools.push(Ready {
        tool: Tool::one("unxz", "team", "T", Handed::Env, "KUBECONFIG"),
        program: xz.clone(),
    });
    let asker = Asker {
        root: root.clone(),
        env: Env::of(&[("PATH", "/usr/bin:/bin")]),
        chat: 7,
        persona: Some("devops".into()),
        folder: Some(root.join("work")),
        confines: Some(confines),
        harness: Some(Harness::Codex),
    };
    let wanted = Wanted {
        vault: "team".into(),
        env: vec!["T=KUBECONFIG".into()],
        command: vec![
            xz.display().to_string(),
            "-c".into(),
            file.display().to_string(),
        ],
        cwd: Some(root.join("work")),
        environment: vec![("PATH".into(), "/usr/bin:/bin".into())],
        ..Wanted::default()
    };
    let (ours, _theirs) = std::os::unix::net::UnixStream::pair().expect("a connection");
    let wire = Wire::default();
    brokered::serve(
        &asker,
        wanted,
        Box::new(std::io::BufReader::new(ours)),
        Box::new(wire.clone()),
        Arc::new(|_| {}),
    );
    let text = String::from_utf8(wire.0.lock().expect("the wire").clone()).expect("text");
    let frames: Vec<Frame> = text
        .lines()
        .map(|line| serde_json::from_str(line).expect("a frame"))
        .collect();
    assert_eq!(frames.last(), Some(&Frame::Exit(0)), "{frames:?}");
    assert_eq!(stdout(&frames), "unpacked\n", "{frames:?}");
    // The run's named link went with it.
    let exec = purlis_core::secrets::Ctx::new(&root, Env::of(&[]))
        .vaults_dir()
        .join(brokered::EXEC_DIR);
    let left: Vec<_> = fs::read_dir(&exec)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|entry| entry.file_name().to_string_lossy().starts_with("as-"))
        .map(|entry| entry.path())
        .collect();
    assert!(left.is_empty(), "left behind: {left:?}");
}
