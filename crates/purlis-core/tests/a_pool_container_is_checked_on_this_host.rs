// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The Docker pool's container check, measured on this host** (#2680): a container made the
//! pool's way ([`Runner::create_args`]) on the `docker` this host has, what `inspect` and `info`
//! write of it recorded, and the check held to it. A correctly made container passes, and each
//! loosened one is refused.
//!
//! It needs an image named by digest in `DOCKER_POOL_MEASURE_IMAGE` that keeps running whatever
//! its command (a stand-in will do: nothing in it is run), the command `DOCKER_POOL_TEST_DOCKER`
//! names (`docker` when unset; a Podman, or a script running one under `sudo`, answers as it
//! does), and a folder in `DOCKER_POOL_MEASURE_OUT` to write what it read into. Without the
//! image it runs nothing and passes: `.github/workflows/docker-pool-hosts.yml` runs it on each
//! host it is measured on.

#![cfg(unix)]

use std::path::{Path, PathBuf};
use std::process::Command;

use purlis_core::dockerpool::{self, ContainerId, Image, Made, Runner, User};

/// What a host's `docker` answered `args` with: its status, what it printed and what it said.
struct Answered {
    ok: bool,
    out: String,
    said: String,
}

fn run(docker: &Path, args: &[String]) -> Answered {
    let output = purlis_core::forklock::output(Command::new(docker).args(args))
        .unwrap_or_else(|why| panic!("{} could not be run: {why}", docker.display()));
    Answered {
        ok: output.status.success(),
        out: String::from_utf8_lossy(&output.stdout).into_owned(),
        said: String::from_utf8_lossy(&output.stderr).into_owned(),
    }
}

fn words(words: &[&str]) -> Vec<String> {
    words.iter().map(|word| (*word).to_owned()).collect()
}

/// Writes `answered` under `out` as `<name>.json` (what it printed) and `<name>.said` (what it
/// said, with its status), so the run's record holds both.
fn record(out: &Path, name: &str, answered: &Answered) {
    std::fs::write(out.join(format!("{name}.json")), &answered.out).unwrap();
    std::fs::write(
        out.join(format!("{name}.said")),
        format!("ok: {}\n{}", answered.ok, answered.said),
    )
    .unwrap();
}

/// A container made for `runner` with `loosened` added to the pool's words, inspected, and
/// removed with its volume whatever happens: what `inspect` printed, and the container's id.
fn made_and_inspected(
    docker: &Path,
    runner: &Runner,
    loosened: &[&str],
    out: &Path,
    name: &str,
) -> Result<(Made, serde_json::Value), String> {
    // The runner's seccomp profile, kept where `docker` reads it, as the pool keeps it for a run.
    let seccomp = out.join(purlis_core::dockerpool::SECCOMP_FILE);
    std::fs::write(&seccomp, purlis_core::dockerpool::SECCOMP_PROFILE)
        .map_err(|why| format!("the runner's seccomp profile could not be kept: {why}"))?;
    let mut args = runner.create_args(&seccomp);
    // Each loosening as a flag and its value: in place of the pool's own value of a flag the
    // pool gives once (`--network`, `--ipc`, `--cgroupns`) and of the pool's own seccomp
    // profile (an engine takes one, and Podman the first), else after `run --detach`.
    for pair in loosened.chunks(2) {
        let (flag, value) = match pair {
            [flag] => (*flag, None),
            [flag, value] => (*flag, Some(*value)),
            _ => unreachable!(),
        };
        let once = matches!(flag, "--network" | "--ipc" | "--cgroupns");
        if let Some(seccomp) = value.filter(|value| value.starts_with("seccomp="))
            && let Some(at) = args.iter().position(|word| word.starts_with("seccomp="))
        {
            args[at] = seccomp.to_owned();
            continue;
        }
        match args.iter().position(|word| word == flag) {
            Some(at) if once => {
                if let Some(value) = value {
                    args[at + 1] = value.to_owned();
                }
            }
            _ => {
                let mut words = vec![flag.to_owned()];
                words.extend(value.map(str::to_owned));
                for (offset, word) in words.into_iter().enumerate() {
                    args.insert(2 + offset, word);
                }
            }
        }
    }
    let made = run(docker, &args);
    record(out, &format!("{name}-run"), &made);
    let removes = Removes(docker.to_path_buf(), runner.container(), runner.volume());
    // An engine that cannot make a container this way says why, which is recorded.
    if !made.ok {
        return Err(made.said.trim().to_owned());
    }
    let id = ContainerId::new(made.out.trim()).expect("docker run printed the container's id");
    // Long enough for a container that would end to have ended.
    std::thread::sleep(std::time::Duration::from_secs(2));
    let inspected = run(docker, &words(&["container", "inspect", id.as_str()]));
    record(out, &format!("{name}-inspect"), &inspected);
    // Podman's own record of what it asked its runtime for, beside what it says of the
    // container: kept to be measured (#2895), and read by nothing here.
    if let Some(oci) = serde_json::from_str::<serde_json::Value>(&inspected.out)
        .ok()
        .and_then(|value| {
            value
                .pointer("/0/OCIConfigPath")
                .and_then(|path| path.as_str().map(str::to_owned))
        })
    {
        let read = run(Path::new("/bin/cat"), &words(&[&oci]));
        let read = if read.ok {
            read
        } else {
            run(Path::new("/usr/bin/sudo"), &words(&["cat", &oci]))
        };
        record(out, &format!("{name}-oci"), &read);
    }
    drop(removes);
    assert!(inspected.ok, "{name}: not inspected: {}", inspected.said);
    let value = serde_json::from_str(&inspected.out).expect("inspect printed JSON");
    Ok((
        Made {
            container: id,
            runner: runner.clone(),
        },
        value,
    ))
}

/// The check purlis makes of a container on `engine`: what `inspect` says of it, and, on
/// rootless Podman, what Podman asked its runtime for, as it was recorded beside it.
fn checked_whole(
    docker: &Path,
    inspected: &serde_json::Value,
    made: &Made,
    engine: dockerpool::Engine,
    out: &Path,
    name: &str,
) -> Result<(), dockerpool::Refused> {
    dockerpool::check_on(inspected, made, engine)?;
    if engine.rootless {
        let oci = std::fs::read_to_string(out.join(format!("{name}-oci.json"))).unwrap_or_default();
        let oci = serde_json::from_str(&oci).unwrap_or(serde_json::Value::Null);
        let config = inspected
            .pointer("/0/OCIConfigPath")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        let store = serde_json::from_str(
            &run(docker, &words(&["info", "--format", "{{json .Store}}"])).out,
        )
        .unwrap_or(serde_json::Value::Null);
        dockerpool::check_oci(
            &oci,
            made,
            Path::new(config),
            &dockerpool::podman_store(&store)?,
        )?;
    }
    Ok(())
}

/// Removes a container and its home volume, however the test ends.
struct Removes(PathBuf, String, String);

impl Drop for Removes {
    fn drop(&mut self) {
        let _ = run(&self.0, &words(&["container", "rm", "--force", &self.1]));
        let _ = run(&self.0, &words(&["volume", "rm", "--force", &self.2]));
    }
}

#[test]
fn a_container_made_the_pools_way_passes_the_check_here_and_a_loosened_one_does_not() {
    purlis_core::unsteered!();
    let Ok(image) = std::env::var("DOCKER_POOL_MEASURE_IMAGE") else {
        return;
    };
    let out =
        PathBuf::from(std::env::var_os("DOCKER_POOL_MEASURE_OUT").expect("a folder to write in"));
    std::fs::create_dir_all(&out).unwrap();
    let docker =
        PathBuf::from(std::env::var_os("DOCKER_POOL_TEST_DOCKER").unwrap_or("docker".into()));
    let image = Image::pinned(&image).expect("the image is named by digest");

    for (name, args) in [
        ("version", words(&["version", "--format", "{{json .}}"])),
        ("info", words(&["info", "--format", "{{json .}}"])),
        (
            "info-security",
            words(&["info", "--format", "{{json .SecurityOptions}}"]),
        ),
        (
            "info-host-security",
            words(&["info", "--format", "{{json .Host.Security}}"]),
        ),
    ] {
        record(&out, name, &run(&docker, &args));
    }

    let user = User::new(10001, 10001).unwrap();
    let suffix = std::process::id();
    let runner = |name: &str| {
        Runner::new(
            &format!("{name}-{suffix}"),
            image.clone(),
            user,
            "/home/purlis",
        )
        .unwrap()
    };
    let mut seen = Vec::new();

    // The pool's own way, whole: the daemon's check, the container made, checked and its home
    // volume checked, then removed.
    let pool = dockerpool::Docker::at(docker.clone()).with_settings_in(out.join("docker-settings"));
    let whole = pool.make(&runner("whole"));
    if let Ok(made) = &whole {
        let _ = pool.remove(made);
    }
    let whole = whole.map(drop);
    seen.push(format!("made by the pool: {whole:?}"));

    let (made, inspected) = made_and_inspected(&docker, &runner("measured"), &[], &out, "made")
        .expect("the pool's own container is made");
    // The engine its daemon's check finds: a rootless Podman says so.
    let engine = serde_json::from_str(
        &run(
            &docker,
            &words(&["info", "--format", "{{json .Host.Security}}"]),
        )
        .out,
    )
    .ok()
    .and_then(|security| dockerpool::check_podman(&security).ok())
    .unwrap_or_default();
    seen.push(format!("engine: {engine:?}"));
    let checked = checked_whole(&docker, &inspected, &made, engine, &out, "made");
    seen.push(format!("made the pool's way: {checked:?}"));

    let mut loosened_taken = Vec::new();
    for (name, loosened) in [
        ("privileged", &["--privileged"][..]),
        ("capability", &["--cap-add", "NET_ADMIN"][..]),
        ("unconfined", &["--security-opt", "apparmor=unconfined"][..]),
        ("host-pid", &["--pid", "host"][..]),
        (
            "seccomp-unconfined",
            &["--security-opt", "seccomp=unconfined"][..],
        ),
        ("host-users", &["--userns", "host"][..]),
        ("host-network", &["--network", "host"][..]),
        // #2895: the machine's IPC, host name and cgroups, a folder of the machine, a device.
        ("host-ipc", &["--ipc", "host"][..]),
        ("host-uts", &["--uts", "host"][..]),
        ("host-cgroups", &["--cgroupns", "host"][..]),
        ("host-folder", &["--volume", "/:/host:ro"][..]),
        ("device", &["--device", "/dev/fuse"][..]),
    ] {
        let (made, inspected) =
            match made_and_inspected(&docker, &runner(name), loosened, &out, name) {
                Ok(both) => both,
                // Nothing this engine makes this way is ever a runner: said, not failed.
                Err(why) => {
                    seen.push(format!("{name}: not made by this engine ({why})"));
                    continue;
                }
            };
        let checked = checked_whole(&docker, &inspected, &made, engine, &out, name);
        seen.push(format!("{name}: {checked:?}"));
        // Podman's own user namespace is the machine's unless it is asked for another, so
        // `--userns host` makes the container the pool makes, word for word (measured
        // 2026-10-20): nothing loosened, and nothing to refuse.
        let podman = inspected.pointer("/0/OCIRuntime").is_some();
        if name == "host-users" && podman {
            assert_eq!(
                inspected.pointer("/0/HostConfig/UsernsMode"),
                Some(&serde_json::Value::from("")),
                "{name}"
            );
            continue;
        }
        if checked.is_ok() {
            loosened_taken.push(name);
        }
    }
    let seen = seen.join("\n");
    std::fs::write(out.join("checked.txt"), &seen).unwrap();
    println!("{seen}");
    assert!(loosened_taken.is_empty(), "{seen}");
    assert!(whole.is_ok(), "{seen}");
    assert!(checked.is_ok(), "{seen}");
}
