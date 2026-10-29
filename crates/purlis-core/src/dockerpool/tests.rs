// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

const DIGEST: &str = "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

fn image() -> Image {
    Image::pinned(&format!("ghcr.io/purlis/purlis-runner@{DIGEST}")).unwrap()
}

fn runner() -> Runner {
    Runner::new(
        "first",
        image(),
        User::new(10001, 10001).unwrap(),
        "/home/purlis",
    )
    .unwrap()
}

fn has_pair(args: &[String], flag: &str, value: &str) -> bool {
    args.windows(2).any(|w| w[0] == flag && w[1] == value)
}

#[test]
fn a_runner_container_starts_with_every_capability_dropped_and_no_new_privileges() {
    let args = runner().create_args(std::path::Path::new(SECCOMP_AT));
    assert_eq!(args[0], "run");
    assert!(has_pair(&args, "--cap-drop", "ALL"), "{args:?}");
    assert!(
        has_pair(&args, "--security-opt", "no-new-privileges"),
        "{args:?}"
    );
    assert!(has_pair(&args, "--user", "10001:10001"), "{args:?}");
    assert!(has_pair(&args, "--network", "bridge"), "{args:?}");
    assert!(has_pair(&args, "--ipc", "private"), "{args:?}");
    assert!(has_pair(&args, "--cgroupns", "private"), "{args:?}");
}

/// The runner's seccomp profile as `docker inspect` writes it back (#3410).
fn runners_seccomp() -> String {
    let profile: serde_json::Value = serde_json::from_str(SECCOMP_PROFILE).expect("JSON");
    format!("seccomp={profile}")
}

/// Where a test names the runner's seccomp profile as kept for a run of `docker`.
const SECCOMP_AT: &str = "/data/docker-settings/run-x/runner-seccomp.json";

#[test]
fn a_runner_container_has_the_runners_seccomp_profile_and_is_never_privileged() {
    let args = runner().create_args(std::path::Path::new(SECCOMP_AT));
    assert!(!args.iter().any(|a| a.contains("privileged")), "{args:?}");
    let seccomp: Vec<&String> = args.iter().filter(|a| a.contains("seccomp")).collect();
    assert_eq!(seccomp, [&format!("seccomp={SECCOMP_AT}")], "{args:?}");
    assert!(!args.iter().any(|a| a.contains("apparmor")), "{args:?}");
    assert!(!args.iter().any(|a| a == "--cap-add"), "{args:?}");
}

/// #3410: the runner's profile is Docker's default with `pidfd_getfd` allowed besides (and the
/// browser engine's sandbox's calls, #3298): every rule of Docker's own is there as it was.
#[test]
fn the_runners_seccomp_profile_is_dockers_default_and_one_call() {
    let profile: serde_json::Value = serde_json::from_str(SECCOMP_PROFILE).expect("JSON");
    assert_eq!(profile["defaultAction"], "SCMP_ACT_ERRNO");
    let rules = profile["syscalls"].as_array().expect("its rules");
    let added: Vec<&serde_json::Value> = rules
        .iter()
        .filter(|rule| rule["names"] == serde_json::json!(["pidfd_getfd"]))
        .collect();
    assert_eq!(
        added,
        [&serde_json::json!({"names": ["pidfd_getfd"], "action": "SCMP_ACT_ALLOW"})]
    );
    // Docker's own rule for it stays as Docker wrote it, behind the capability no runner has.
    assert!(rules.iter().any(|rule| {
        rule["names"]
            .as_array()
            .is_some_and(|names| names.contains(&"pidfd_getfd".into()))
            && rule["includes"]["caps"] == serde_json::json!(["CAP_SYS_PTRACE"])
    }));
    assert_eq!(rules.len(), 41);
}

#[test]
fn a_runner_container_runs_the_pinned_image_by_digest_as_its_session_host() {
    let args = runner().create_args(std::path::Path::new(SECCOMP_AT));
    let at = args
        .iter()
        .position(|a| a == &format!("ghcr.io/purlis/purlis-runner@{DIGEST}"))
        .expect("the image is named by its digest");
    assert_eq!(&args[at + 1..], ["serve", "--suspends-unlinked"]);
}

#[test]
fn a_runner_of_a_pool_with_its_own_time_suspends_after_it() {
    let at_the_end = |runner: Runner| {
        let args = runner.create_args(std::path::Path::new(SECCOMP_AT));
        args[args.len() - 2..].to_vec()
    };
    assert_eq!(
        at_the_end(runner().suspending_after(std::time::Duration::from_secs(90 * 60))),
        ["serve", "--suspends-unlinked=90m"]
    );
    assert_eq!(
        at_the_end(runner().suspending_after(std::time::Duration::from_secs(48 * 60 * 60))),
        ["serve", "--suspends-unlinked=48h"]
    );
    // The default is said as it was before a pool could set its own.
    assert_eq!(
        at_the_end(runner().suspending_after(std::time::Duration::from_secs(24 * 60 * 60))),
        ["serve", "--suspends-unlinked"]
    );
}

/// How a runner's container was made to suspend itself is read back from its command (#3717):
/// one made before purlis suspended runners was made with `serve` alone and never suspends.
#[test]
fn how_a_runner_was_made_to_suspend_is_read_from_its_containers_command() {
    let made_with = |runner: Runner| {
        let args = runner.create_args(std::path::Path::new(SECCOMP_AT));
        let mut inspected = inspected();
        inspected[0]["Config"]["Cmd"] = serde_json::json!(args[args.len() - 2..]);
        made_suspending(&inspected)
    };
    let secs = std::time::Duration::from_secs;
    assert_eq!(
        made_with(runner()),
        Some(MadeSuspending::After(SUSPEND_AFTER))
    );
    assert_eq!(
        made_with(runner().suspending_after(secs(90 * 60))),
        Some(MadeSuspending::After(secs(90 * 60)))
    );
    let with_cmd = |cmd: serde_json::Value| {
        let mut inspected = inspected();
        inspected[0]["Config"]["Cmd"] = cmd;
        made_suspending(&inspected)
    };
    assert_eq!(
        with_cmd(serde_json::json!(["serve"])),
        Some(MadeSuspending::Never)
    );
    // What purlis cannot read as a runner's command says nothing.
    assert_eq!(with_cmd(serde_json::Value::Null), None);
    assert_eq!(with_cmd(serde_json::json!(["sh"])), None);
    assert_eq!(
        with_cmd(serde_json::json!(["serve", "--suspends-unlinked=soon"])),
        None
    );
    // A time `purlis serve` refuses, outside a pool's bounds, is not a time it suspends after.
    for refused in ["30s", "721h"] {
        assert_eq!(
            with_cmd(serde_json::json!([
                "serve",
                format!("--suspends-unlinked={refused}")
            ])),
            None,
            "{refused}"
        );
    }
    assert_eq!(
        with_cmd(serde_json::json!(["serve", "--suspends-unlinked=720h"])),
        Some(MadeSuspending::After(MOST_SUSPEND_AFTER))
    );
}

#[test]
fn a_pools_time_before_its_runner_suspends_is_a_minute_to_30_days() {
    let secs = std::time::Duration::from_secs;
    assert!(suspend_after(secs(59)).is_err());
    assert!(suspend_after(secs(60)).is_ok());
    assert!(suspend_after(secs(30 * 24 * 60 * 60)).is_ok());
    assert!(suspend_after(secs(30 * 24 * 60 * 60 + 1)).is_err());
}

// The host is linked on macOS and Linux alone.
#[cfg(unix)]
#[test]
fn a_pools_runner_suspends_after_the_hosts_own_day_by_default() {
    assert_eq!(SUSPEND_AFTER, purlis_host::UNLINKED_FOR);
}

#[test]
fn a_runner_container_holds_its_home_on_a_volume_of_its_own_and_no_folder_of_this_machine() {
    let args = runner().create_args(std::path::Path::new(SECCOMP_AT));
    let mounts: Vec<&String> = args
        .windows(2)
        .filter(|w| w[0] == "--mount")
        .map(|w| &w[1])
        .collect();
    assert_eq!(
        mounts,
        ["type=volume,source=purlis-runner-first-home,target=/home/purlis"]
    );
    assert!(!args.iter().any(|a| a == "-v" || a == "--volume"));
}

/// The `purlis` whose holding of namespaces the check asks is the one that serves, whatever the
/// image's entrypoint, under the runtime's init (#3702).
#[test]
fn a_runner_container_serves_the_purlis_the_check_asks_under_the_runtimes_init() {
    let args = runner().create_args(std::path::Path::new(SECCOMP_AT));
    assert!(has_pair(&args, "--entrypoint", PURLIS), "{args:?}");
    assert!(args.iter().any(|a| a == "--init"), "{args:?}");
    let at = args
        .iter()
        .position(|a| a == image().as_str())
        .expect("the image");
    assert!(
        args.iter().position(|a| a == "--entrypoint").unwrap() < at,
        "a flag of the run, not the program's"
    );
    assert_eq!(args[at + 1], "serve");
}

#[test]
fn a_runner_container_has_a_read_only_root_and_a_tmp_that_runs_no_program() {
    let args = runner().create_args(std::path::Path::new(SECCOMP_AT));
    assert!(args.iter().any(|a| a == "--read-only"), "{args:?}");
    assert!(
        has_pair(&args, "--tmpfs", "/tmp:rw,nosuid,nodev,noexec,size=1024m"),
        "{args:?}"
    );
}

#[test]
fn a_runner_container_is_held_to_its_pools_limits_or_the_defaults() {
    let args = runner().create_args(std::path::Path::new(SECCOMP_AT));
    for (flag, value) in [
        ("--cpus", "2"),
        ("--memory", "4096m"),
        ("--memory-swap", "4096m"),
        ("--pids-limit", "1024"),
        // A quarter of its memory, for the browser engine's shared memory (#3298).
        ("--shm-size", "1024m"),
    ] {
        assert!(has_pair(&args, flag, value), "{flag} {value}: {args:?}");
    }
    let limits = Limits::new(1500, 8192, 256, 512).unwrap();
    let limited = runner().limited(limits);
    let args = limited.create_args(std::path::Path::new(SECCOMP_AT));
    for (flag, value) in [
        ("--cpus", "1.500"),
        ("--memory", "8192m"),
        ("--memory-swap", "8192m"),
        ("--pids-limit", "256"),
        ("--tmpfs", "/tmp:rw,nosuid,nodev,noexec,size=512m"),
        ("--shm-size", "2048m"),
    ] {
        assert!(has_pair(&args, flag, value), "{flag} {value}: {args:?}");
    }
    // The check holds the container to the limits it was made with.
    let made = Made {
        container: ContainerId::new(ID).unwrap(),
        runner: limited,
    };
    assert!(check(&inspected(), &made).is_err());
    let held = with(|c| {
        c["HostConfig"]["NanoCpus"] = 1_500_000_000u64.into();
        c["HostConfig"]["Memory"] = 8_589_934_592u64.into();
        c["HostConfig"]["MemorySwap"] = 8_589_934_592u64.into();
        c["HostConfig"]["PidsLimit"] = 256.into();
        c["HostConfig"]["Tmpfs"] =
            serde_json::json!({ "/tmp": "rw,nosuid,nodev,noexec,size=512m" });
    });
    assert_eq!(check(&held, &made), Ok(()));
}

#[test]
fn a_pools_limits_leave_a_runner_room_to_start_a_chat() {
    assert!(Limits::new(2000, 4096, 1024, 1024).is_ok());
    assert!(Limits::new(10, 512, 64, 512).is_ok(), "the least of each");
    for (cpus, memory, pids, tmp) in [
        (0, 4096, 1024, 1024),
        (9, 4096, 1024, 1024),
        (2000, 256, 1024, 128),
        (2000, 4096, 8, 1024),
        (2000, 4096, 1024, 0),
        (2000, 4096, 1024, 8192),
    ] {
        assert!(
            Limits::new(cpus, memory, pids, tmp).is_err(),
            "{cpus} {memory} {pids} {tmp} was taken"
        );
    }
}

#[test]
fn a_runner_container_is_named_and_labelled_as_the_pools() {
    let args = runner().create_args(std::path::Path::new(SECCOMP_AT));
    assert!(has_pair(&args, "--name", "purlis-runner-first"), "{args:?}");
    assert!(
        has_pair(&args, "--label", "purlis.runner=first"),
        "{args:?}"
    );
    assert!(args.iter().any(|a| a == "--detach"));
}

#[test]
fn an_image_named_by_a_tag_is_refused() {
    for refused in [
        "ghcr.io/purlis/purlis-runner:main",
        "ghcr.io/purlis/purlis-runner",
        "ghcr.io/purlis/purlis-runner@sha256:abc",
        "ghcr.io/purlis/purlis-runner:main@sha256:ABCDEF0123456789abcdef0123456789abcdef0123456789abcdef0123456789",
        "-v/:/host@sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        "",
    ] {
        assert!(Image::pinned(refused).is_err(), "{refused} was taken");
    }
}

#[test]
fn an_image_with_a_tag_beside_its_digest_is_taken_by_the_digest() {
    let image = Image::pinned(&format!("localhost:5000/team/runner:v1@{DIGEST}")).unwrap();
    assert_eq!(
        image.as_str(),
        format!("localhost:5000/team/runner:v1@{DIGEST}")
    );
}

#[test]
fn root_is_refused_as_a_runners_user() {
    assert!(User::new(0, 10001).is_err());
    assert!(User::new(10001, 0).is_err());
}

#[test]
fn a_runner_name_that_is_not_lower_case_letters_digits_and_dashes_is_refused() {
    for refused in [
        "",
        "-x",
        "x-",
        "First",
        "a b",
        "a/b",
        "a.b",
        &"x".repeat(50),
    ] {
        assert!(
            Runner::new(
                refused,
                image(),
                User::new(10001, 10001).unwrap(),
                "/home/purlis"
            )
            .is_err(),
            "{refused:?} was taken"
        );
    }
}

#[test]
fn a_home_that_is_not_an_absolute_plain_path_is_refused() {
    for refused in ["", "home", "/home/../etc", "/home/a,b", "/home/a=b", "/"] {
        assert!(
            Runner::new("first", image(), User::new(10001, 10001).unwrap(), refused).is_err(),
            "{refused:?} was taken"
        );
    }
}

pub(crate) const ID: &str = "bedaed78395a1217511b3b656d2e4d5d3b9f279c0c7270290596f86688c01fe4";

pub(crate) fn made() -> Made {
    Made {
        container: ContainerId::new(ID).unwrap(),
        runner: runner(),
    }
}

/// What `docker inspect` printed for a container made by [`Runner::create_args`] (Docker
/// 29.8, 2026-10-17; the limits, read-only root and `/tmp` measured 2026-10-18), cut to the
/// fields the check reads and a few beside them.
pub(crate) fn inspected() -> serde_json::Value {
    serde_json::json!([{
        "Id": ID,
        "State": { "Running": true },
        "Image": DIGEST,
        "AppArmorProfile": "",
        "ProcessLabel": "",
        "MountLabel": "",
        "HostConfig": {
            "Privileged": false,
            "SecurityOpt": ["no-new-privileges", runners_seccomp()],
            "CapAdd": null,
            "CapDrop": ["ALL"],
            "NetworkMode": "bridge",
            "PidMode": "",
            "IpcMode": "private",
            "UTSMode": "",
            "UsernsMode": "",
            "CgroupnsMode": "private",
            "Devices": [],
            "DeviceRequests": null,
            "Binds": null,
            "VolumesFrom": null,
            "ReadonlyRootfs": true,
            "Tmpfs": { "/tmp": "rw,nosuid,nodev,noexec,size=1024m" },
            "NanoCpus": 2_000_000_000u64,
            "Memory": 4_294_967_296u64,
            "MemorySwap": 4_294_967_296u64,
            "PidsLimit": 1024,
            "OomKillDisable": null,
            "Runtime": "runc",
            "PortBindings": {},
            "PublishAllPorts": false
        },
        "NetworkSettings": { "Networks": { "bridge": {} } },
        "Mounts": [{
            "Type": "volume",
            "Name": "purlis-runner-first-home",
            "Destination": "/home/purlis",
            "Driver": "local",
            "RW": true
        }],
        "Config": {
            "User": "10001:10001",
            "Image": format!("ghcr.io/purlis/purlis-runner@{DIGEST}"),
            "Labels": { "purlis.runner": "first" }
        }
    }])
}

fn with(change: impl FnOnce(&mut serde_json::Value)) -> serde_json::Value {
    let mut inspected = inspected();
    change(&mut inspected[0]);
    inspected
}

#[test]
fn a_container_confined_as_it_was_made_passes_the_check() {
    assert_eq!(check(&inspected(), &made()), Ok(()));
}

/// One way a container differs from how it was made.
type Change = Box<dyn Fn(&mut serde_json::Value)>;

#[test]
fn a_container_that_is_not_confined_as_it_was_made_is_refused() {
    let cases: Vec<(&str, Change)> = vec![
        (
            "privileged",
            Box::new(|c| c["HostConfig"]["Privileged"] = true.into()),
        ),
        (
            "seccomp off",
            Box::new(|c| {
                c["HostConfig"]["SecurityOpt"] =
                    serde_json::json!(["no-new-privileges", "seccomp=unconfined"])
            }),
        ),
        (
            "a seccomp profile of its own",
            Box::new(|c| {
                c["HostConfig"]["SecurityOpt"] = serde_json::json!([
                    "no-new-privileges",
                    "seccomp={\"defaultAction\":\"SCMP_ACT_ALLOW\"}"
                ])
            }),
        ),
        (
            "apparmor off",
            Box::new(|c| {
                c["HostConfig"]["SecurityOpt"] = serde_json::json!([
                    "no-new-privileges",
                    "apparmor=unconfined",
                    runners_seccomp()
                ])
            }),
        ),
        (
            "apparmor off, read from the profile",
            Box::new(|c| c["AppArmorProfile"] = "unconfined".into()),
        ),
        (
            "labels off",
            Box::new(|c| {
                c["HostConfig"]["SecurityOpt"] =
                    serde_json::json!(["no-new-privileges", "label=disable", runners_seccomp()])
            }),
        ),
        (
            "new privileges",
            Box::new(|c| c["HostConfig"]["SecurityOpt"] = serde_json::json!([])),
        ),
        (
            "a capability added",
            Box::new(|c| c["HostConfig"]["CapAdd"] = serde_json::json!(["SYS_ADMIN"])),
        ),
        (
            "capabilities kept",
            Box::new(|c| c["HostConfig"]["CapDrop"] = serde_json::json!(["NET_RAW"])),
        ),
        (
            "the machine's network",
            Box::new(|c| c["HostConfig"]["NetworkMode"] = "host".into()),
        ),
        (
            "another container's network",
            Box::new(|c| c["HostConfig"]["NetworkMode"] = "container:abc".into()),
        ),
        (
            "the machine's processes",
            Box::new(|c| c["HostConfig"]["PidMode"] = "host".into()),
        ),
        (
            "another container's processes",
            Box::new(|c| c["HostConfig"]["PidMode"] = "container:abc".into()),
        ),
        (
            "the machine's shared memory",
            Box::new(|c| c["HostConfig"]["IpcMode"] = "host".into()),
        ),
        (
            "the machine's host name",
            Box::new(|c| c["HostConfig"]["UTSMode"] = "host".into()),
        ),
        (
            "the machine's users",
            Box::new(|c| c["HostConfig"]["UsernsMode"] = "host".into()),
        ),
        (
            "the machine's cgroups",
            Box::new(|c| c["HostConfig"]["CgroupnsMode"] = "host".into()),
        ),
        (
            "a device",
            Box::new(|c| {
                c["HostConfig"]["Devices"] = serde_json::json!([{"PathOnHost": "/dev/kvm"}])
            }),
        ),
        (
            "a GPU",
            Box::new(|c| {
                c["HostConfig"]["DeviceRequests"] = serde_json::json!([{"Driver": "nvidia"}])
            }),
        ),
        (
            "another container's volumes",
            Box::new(|c| c["HostConfig"]["VolumesFrom"] = serde_json::json!(["abc"])),
        ),
        (
            "another runtime",
            Box::new(|c| c["HostConfig"]["Runtime"] = "sysbox-runc".into()),
        ),
        (
            "a folder of this machine",
            Box::new(
                |c| c["Mounts"] = serde_json::json!([{"Type": "bind", "Source": "/var/run/docker.sock", "Destination": "/var/run/docker.sock", "RW": true}]),
            ),
        ),
        (
            "a second volume",
            Box::new(|c| {
                c["Mounts"].as_array_mut().unwrap().push(serde_json::json!({"Type": "volume", "Name": "other", "Destination": "/data", "RW": true}))
            }),
        ),
        (
            "another volume as its home",
            Box::new(|c| c["Mounts"][0]["Name"] = "purlis-runner-second-home".into()),
        ),
        ("root", Box::new(|c| c["Config"]["User"] = "0:0".into())),
        (
            "root by name",
            Box::new(|c| c["Config"]["User"] = "root".into()),
        ),
        (
            "the image's own user",
            Box::new(|c| c["Config"]["User"] = "".into()),
        ),
        (
            "another user",
            Box::new(|c| c["Config"]["User"] = "10002:10002".into()),
        ),
        (
            "another image",
            Box::new(|c| c["Config"]["Image"] = "ghcr.io/purlis/purlis-runner:main".into()),
        ),
        (
            "another runner's label",
            Box::new(|c| c["Config"]["Labels"]["purlis.runner"] = "second".into()),
        ),
        (
            "no label",
            Box::new(|c| c["Config"]["Labels"] = serde_json::json!({})),
        ),
        (
            "another container",
            Box::new(|c| c["Id"] = "0".repeat(64).into()),
        ),
        (
            "stopped",
            Box::new(|c| c["State"]["Running"] = false.into()),
        ),
        (
            "a field missing",
            Box::new(|c| {
                c["HostConfig"]
                    .as_object_mut()
                    .unwrap()
                    .remove("Privileged");
            }),
        ),
        (
            "a field of another shape",
            Box::new(|c| c["HostConfig"]["SecurityOpt"] = "no-new-privileges".into()),
        ),
        (
            "a port of this machine",
            Box::new(|c| {
                c["HostConfig"]["PortBindings"] =
                    serde_json::json!({ "22/tcp": [{ "HostIp": "", "HostPort": "2222" }] })
            }),
        ),
        (
            "every port published",
            Box::new(|c| c["HostConfig"]["PublishAllPorts"] = true.into()),
        ),
        (
            "a second network",
            Box::new(|c| {
                c["NetworkSettings"]["Networks"] = serde_json::json!({ "bridge": {}, "team": {} })
            }),
        ),
        (
            "another network",
            Box::new(|c| c["NetworkSettings"]["Networks"] = serde_json::json!({ "team": {} })),
        ),
        (
            "a memory-backed folder",
            Box::new(|c| c["HostConfig"]["Tmpfs"] = serde_json::json!({ "/run": "" })),
        ),
        (
            "a memory-backed folder beside its /tmp",
            Box::new(|c| {
                c["HostConfig"]["Tmpfs"] =
                    serde_json::json!({ "/tmp": "rw,nosuid,nodev,noexec,size=1024m", "/run": "" })
            }),
        ),
        (
            "a /tmp that may run programs",
            Box::new(|c| c["HostConfig"]["Tmpfs"] = serde_json::json!({ "/tmp": "rw,size=1024m" })),
        ),
        (
            "no /tmp of its own",
            Box::new(|c| c["HostConfig"]["Tmpfs"] = serde_json::Value::Null),
        ),
        (
            "a writable root",
            Box::new(|c| c["HostConfig"]["ReadonlyRootfs"] = false.into()),
        ),
        (
            "no CPU limit",
            Box::new(|c| c["HostConfig"]["NanoCpus"] = 0.into()),
        ),
        (
            "no memory limit",
            Box::new(|c| c["HostConfig"]["Memory"] = 0.into()),
        ),
        (
            "swap without end",
            Box::new(|c| c["HostConfig"]["MemorySwap"] = (-1).into()),
        ),
        (
            "no process limit",
            Box::new(|c| c["HostConfig"]["PidsLimit"] = serde_json::Value::Null),
        ),
        (
            "a process limit of none",
            Box::new(|c| c["HostConfig"]["PidsLimit"] = 0.into()),
        ),
        (
            "the out-of-memory killer turned off",
            Box::new(|c| c["HostConfig"]["OomKillDisable"] = true.into()),
        ),
        (
            "no word of the out-of-memory killer",
            Box::new(|c| {
                c["HostConfig"]
                    .as_object_mut()
                    .unwrap()
                    .remove("OomKillDisable");
            }),
        ),
        (
            "a kernel setting",
            Box::new(|c| {
                c["HostConfig"]["Sysctls"] = serde_json::json!({ "net.ipv4.ip_forward": "1" })
            }),
        ),
        (
            "a home on another driver",
            Box::new(|c| c["Mounts"][0]["Driver"] = "nfs".into()),
        ),
        (
            "shareable memory",
            Box::new(|c| c["HostConfig"]["IpcMode"] = "shareable".into()),
        ),
    ];
    for (case, change) in cases {
        assert!(
            check(&with(|c| change(c)), &made()).is_err(),
            "{case} passed the check"
        );
    }
}

#[test]
fn what_is_not_one_container_is_refused() {
    assert!(check(&serde_json::json!([]), &made()).is_err());
    assert!(check(&serde_json::json!({}), &made()).is_err());
    let mut two = inspected();
    two.as_array_mut().unwrap().push(inspected()[0].clone());
    assert!(check(&two, &made()).is_err());
}

#[test]
fn a_container_id_is_sixty_four_lower_case_hex() {
    assert!(ContainerId::new(ID).is_ok());
    for refused in [
        "",
        "bedaed78395a",
        "purlis-runner-first",
        &ID.to_uppercase(),
        &format!("-{}", &ID[1..]),
    ] {
        assert!(ContainerId::new(refused).is_err(), "{refused:?} was taken");
    }
}

#[test]
fn the_connector_runs_the_bridge_as_the_runners_user_and_group_in_the_container_it_pinned() {
    assert_eq!(
        made().connect_args(),
        ["exec", "-i", "--user", "10001:10001", ID, PURLIS, "bridge"]
    );
}

#[test]
fn pairing_runs_the_bridge_to_pair_as_the_runners_user_and_group_in_the_container_it_pinned() {
    assert_eq!(
        made().pair_args(),
        [
            "exec",
            "-i",
            "--user",
            "10001:10001",
            ID,
            PURLIS,
            "bridge",
            "--pair"
        ]
    );
}

/// A profile is signed in on the runner by the runner's own `purlis profile sign-in`, with a
/// terminal, as the runner's user and group in the container it pinned (#2049).
#[test]
fn a_profile_is_signed_in_on_the_runner_by_its_own_sign_in_in_the_container_it_pinned() {
    assert_eq!(
        made().sign_in_args("work", None),
        [
            "exec",
            "-it",
            "--user",
            "10001:10001",
            ID,
            PURLIS,
            "profile",
            "sign-in",
            "work"
        ]
    );
    assert_eq!(
        made().sign_in_args("chatgpt", Some("openai"))[7..],
        ["sign-in", "chatgpt", "--provider", "openai"]
    );
}

/// **The line a sign-in on a pool's runner types reads none of the person's Docker settings**
/// (#2049): it runs `docker` by its path through `/usr/bin/env`, with Docker's settings a folder
/// of purlis's own (`--config` and `DOCKER_CONFIG`), plugin hooks off, no context, the engine
/// on the line, and the container by its id.
#[test]
fn a_sign_in_on_a_runner_runs_docker_with_settings_of_purliss_own_and_no_hooks() {
    let line = Docker::at("/usr/local/bin/docker")
        .sign_in_login(
            &made(),
            std::path::Path::new("/var/run/docker.sock"),
            std::path::Path::new("/data/purlis/docker-settings/sign-in"),
            "work",
            None,
        )
        .unwrap()
        .line()
        .unwrap();

    assert_eq!(
        line,
        format!(
            "/usr/bin/env -u DOCKER_CONTEXT DOCKER_CONFIG=/data/purlis/docker-settings/sign-in \
             DOCKER_CLI_HOOKS=false /usr/local/bin/docker --config \
             /data/purlis/docker-settings/sign-in --host unix:///var/run/docker.sock exec -it \
             --user 10001:10001 {ID} {PURLIS} profile sign-in work\n"
        )
    );
}

/// The settings that line names hold nothing of the person's: a `config.json` of `{}`, in a
/// folder of this user's alone, which a sweep of the runs' copies leaves.
#[test]
fn the_settings_a_sign_in_names_are_an_empty_config_of_purliss_own() {
    let dir = tempfile::tempdir().unwrap();
    let settings_in = dir.path().join(settings::DIR);

    let folder = settings::for_a_sign_in(&settings_in).unwrap();

    assert_eq!(folder, settings_in.join(settings::SIGN_IN));
    assert_eq!(
        std::fs::read_to_string(folder.join("config.json")).unwrap(),
        "{}"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&folder).unwrap().permissions().mode();
        assert_eq!(mode & 0o077, 0, "{mode:o}");
    }
    let later = std::time::SystemTime::now() + std::time::Duration::from_secs(365 * 24 * 3600);
    assert_eq!(settings::sweep(&settings_in, later), 0);
    assert!(folder.join("config.json").exists());
}

/// A stand-in for `docker`: a script that logs each call's words and answers from the files
/// of its folder: `run` with `id`, `container inspect` with `inspect`, `volume inspect` with
/// `volume`, `volume ls` with `volumes`, `info` with `info`, `image inspect` with `image`, and
/// `exec` with `paired`, after keeping its standard input, and `container ls` with
/// `containers`; `run` fails where `run-fails` is there. `container rm` and `volume rm` exit
/// with what `rm-fails-container` or `rm-fails-volume` holds, else what `rm-fails` holds, 0
/// when there is none.
pub(crate) struct FakeDocker {
    dir: tempfile::TempDir,
}

impl FakeDocker {
    pub(crate) fn new(inspect: &serde_json::Value) -> FakeDocker {
        let dir = tempfile::tempdir().unwrap();
        let at = |name: &str| dir.path().join(name).display().to_string();
        let script = format!(
            "#!/bin/sh\necho \"$*\" >> '{log}'\ncase \"$*\" in *_no-namespaces*) \
             prev=; for a in \"$@\"; do [ \"$prev\" = --cidfile ] && echo {probe} > \"$a\"; \
             prev=$a; done; \
             exit \"$(cat '{holds}' 2>/dev/null || echo 0)\" ;; esac\ncase \"$1 $2\" in\n  \
             'run '*) [ -f '{run_fails}' ] && \
             {{ echo 'the runtime refused it' >&2; exit 125; }}; cat '{id}' ;;\n  \
             'container inspect') cat '{inspect}' ;;\n  \
             'volume inspect') cat '{volume}' ;;\n  \
             'volume ls') cat '{volumes}' 2>/dev/null || true ;;\n  \
             'info '*) cat '{info}' ;;\n  \
             'image inspect') cat '{image}' ;;\n  \
             'exec '*) cat > '{input}'; cat '{paired}' ;;\n  \
             'container ls') case \"$*\" in *--no-trunc*.ID*) cat '{containers}-full' 2>/dev/null || true ;; \
             *.ID*) cut -c1-12,65- '{containers}-full' 2>/dev/null || true ;; \
             *) cat '{containers}' 2>/dev/null || true ;; esac ;;\n  \
             *' rm') code=$(cat '{rm_fails}'-\"$1\" 2>/dev/null || cat '{rm_fails}' 2>/dev/null || echo 0); \
             [ \"$code\" = 0 ] && rm -f \"$(dirname '{log}')/$1s\" \"$(dirname '{log}')/$1s-full\"; exit $code ;;\nesac\n",
            log = at("log"),
            id = at("id"),
            inspect = at("inspect"),
            volume = at("volume"),
            volumes = at("volumes"),
            info = at("info"),
            image = at("image"),
            input = at("input"),
            paired = at("paired"),
            rm_fails = at("rm-fails"),
            run_fails = at("run-fails"),
            containers = at("containers"),
            holds = at("holds"),
            probe = PROBE_ID,
        );
        stand_in::program(dir.path(), "docker", &script);
        let fake = FakeDocker { dir };
        fake.set("id", &format!("{ID}\n"));
        fake.set("inspect", &inspect.to_string());
        fake.set(
            "volume",
            r#"[{"Name":"purlis-runner-first-home","Driver":"local","Options":null,"Labels":null}]"#,
        );
        fake.set(
            "info",
            r#"["name=seccomp,profile=builtin","name=cgroupns"]"#,
        );
        fake
    }

    pub(crate) fn docker(&self) -> Docker {
        Docker::at(self.dir.path().join("docker"))
            .with_settings_in(self.dir.path().join("settings"))
    }

    pub(crate) fn file(&self, name: &str) -> String {
        std::fs::read_to_string(self.dir.path().join(name)).unwrap_or_default()
    }

    pub(crate) fn set(&self, name: &str, text: &str) {
        std::fs::write(self.dir.path().join(name), text).unwrap();
    }
}

#[test]
fn a_runner_is_not_made_on_a_home_volume_that_is_already_there() {
    let fake = FakeDocker::new(&inspected());
    fake.set(
        "volumes",
        "purlis-runner-first-home-old\npurlis-runner-first-home\n",
    );
    let refused = fake.docker().make(&runner()).unwrap_err();
    assert!(refused.0.contains("already there"), "{refused}");
    assert!(!fake.file("log").contains("run "), "{}", fake.file("log"));
}

#[test]
fn a_volume_whose_name_only_begins_like_the_homes_is_not_the_home() {
    let fake = FakeDocker::new(&inspected());
    fake.set("volumes", "purlis-runner-first-home-old\n");
    assert!(fake.docker().make(&runner()).is_ok());
}

#[test]
fn a_daemon_without_its_built_in_seccomp_profile_makes_no_runner() {
    let fake = FakeDocker::new(&inspected());
    fake.set(
        "info",
        r#"["name=seccomp,profile=unconfined","name=cgroupns"]"#,
    );
    assert!(fake.docker().make(&runner()).is_err());
    assert!(!fake.file("log").contains("run "), "{}", fake.file("log"));
}

#[test]
fn the_daemons_seccomp_profile_must_be_its_built_in_one() {
    for passes in [
        serde_json::json!(["name=seccomp,profile=builtin", "name=cgroupns"]),
        serde_json::json!(["name=apparmor", "name=seccomp,profile=default"]),
    ] {
        assert_eq!(check_daemon(&passes), Ok(()), "{passes}");
    }
    for refused in [
        serde_json::json!(["name=seccomp,profile=unconfined"]),
        serde_json::json!(["name=seccomp,profile=/etc/docker/seccomp.json"]),
        serde_json::json!(["name=cgroupns"]),
        serde_json::json!([]),
        serde_json::json!([
            "name=seccomp,profile=builtin",
            "name=seccomp,profile=unconfined"
        ]),
        serde_json::json!(null),
        serde_json::json!("name=seccomp,profile=builtin"),
    ] {
        assert!(check_daemon(&refused).is_err(), "{refused} passed");
    }
}

#[test]
fn a_home_volume_must_be_dockers_own_with_no_options() {
    let volume = |driver: &str, options: serde_json::Value| serde_json::json!([{ "Name": "purlis-runner-first-home", "Driver": driver, "Options": options }]);
    assert_eq!(
        check_volume(&volume("local", serde_json::Value::Null), &made()),
        Ok(())
    );
    assert_eq!(
        check_volume(&volume("local", serde_json::json!({})), &made()),
        Ok(())
    );
    for refused in [
        volume(
            "local",
            serde_json::json!({ "type": "none", "o": "bind", "device": "/Users" }),
        ),
        volume("nfs", serde_json::Value::Null),
        serde_json::json!([{ "Name": "other", "Driver": "local", "Options": null }]),
        serde_json::json!([]),
    ] {
        assert!(check_volume(&refused, &made()).is_err(), "{refused} passed");
    }
}

#[test]
fn a_runner_whose_home_volume_was_made_with_options_is_removed() {
    let fake = FakeDocker::new(&inspected());
    fake.set(
        "volume",
        r#"[{"Name":"purlis-runner-first-home","Driver":"local","Options":{"o":"bind","device":"/Users","type":"none"}}]"#,
    );
    assert!(fake.docker().make(&runner()).is_err());
    assert!(
        fake.file("log")
            .contains(&format!("container rm --force --volumes {ID}"))
    );
}

#[test]
fn a_container_whose_id_cannot_be_read_is_removed_by_its_name() {
    let fake = FakeDocker::new(&inspected());
    fake.set("id", "WARNING: something\n");
    assert!(fake.docker().make(&runner()).is_err());
    let log = fake.file("log");
    assert!(
        log.contains("container rm --force --volumes purlis-runner-first\n"),
        "{log}"
    );
    assert!(
        log.contains("volume rm --force purlis-runner-first-home"),
        "{log}"
    );
}

#[test]
fn a_removal_that_fails_is_said_with_the_refusal() {
    let fake = FakeDocker::new(&with(|c| c["HostConfig"]["Privileged"] = true.into()));
    fake.set("rm-fails", "1");
    let refused = fake.docker().make(&runner()).unwrap_err();
    assert!(refused.0.contains("privileged"), "{refused}");
    assert!(refused.0.contains("could not all be removed"), "{refused}");
}

#[test]
fn a_pairing_that_prints_more_than_one_line_of_its_length_is_refused() {
    use purlis_session_protocol::hostlink::{LinkKey, Pinned};
    let fake = FakeDocker::new(&inspected());
    fake.set("paired", &"x".repeat(5000));
    let desktop = Pinned {
        device: "01JA0DESKT0P00000000000000".into(),
        key: LinkKey::generate().unwrap().public(),
    };
    let refused = fake.docker().pair(&made(), &desktop).unwrap_err();
    assert!(refused.0.contains("printed more"), "{refused}");
}

#[test]
fn a_runner_made_and_confined_as_it_should_be_is_handed_back_by_its_container_id() {
    let fake = FakeDocker::new(&inspected());
    let made = fake.docker().make(&runner()).unwrap();
    assert_eq!(made, self::made());
    let log = fake.file("log");
    assert!(
        log.contains("\nrun --detach --name purlis-runner-first"),
        "{log}"
    );
    assert!(log.contains(&format!("container inspect {ID}")), "{log}");
    assert!(!log.contains(" rm "), "{log}");
}

/// The runtime's own default profile is taken only for a runner made before #3410: one made
/// now that came up without the runner's profile is refused and removed with its volume.
#[test]
fn a_runner_made_now_on_the_runtimes_default_profile_is_refused_and_removed() {
    let fake = FakeDocker::new(&on_the_runtimes_default(inspected()));
    let refused = fake.docker().make(&runner()).unwrap_err();
    assert!(refused.0.contains("seccomp"), "{refused}");
    let log = fake.file("log");
    assert!(
        log.contains(&format!("container rm --force --volumes {ID}")),
        "{log}"
    );
    // The same container, made before, is still linked.
    assert_eq!(fake.docker().check(&made()), Ok(()));
}

#[test]
fn a_runner_whose_container_fails_the_check_is_removed_with_its_volume() {
    let fake = FakeDocker::new(&with(|c| c["HostConfig"]["Privileged"] = true.into()));
    let refused = fake.docker().make(&runner()).unwrap_err();
    assert!(refused.0.contains("privileged"), "{refused}");
    let log = fake.file("log");
    assert!(
        log.contains(&format!("container rm --force --volumes {ID}")),
        "{log}"
    );
    assert!(
        log.contains("volume rm --force purlis-runner-first-home"),
        "{log}"
    );
}

#[test]
fn pairing_hands_the_desktops_key_to_the_runner_and_answers_the_runners() {
    use purlis_session_protocol::hostlink::{LinkKey, Pinned};
    let fake = FakeDocker::new(&inspected());
    let desktop = Pinned {
        device: "01JA0DESKT0P00000000000000".into(),
        key: LinkKey::generate().unwrap().public(),
    };
    let runner_pin = Pinned {
        device: "01JB0RUNNER0000000000000000".into(),
        key: LinkKey::generate().unwrap().public(),
    };
    fake.set(
        "paired",
        &format!("{}\n", crate::runnerlink::to_line(&runner_pin)),
    );
    assert_eq!(fake.docker().pair(&made(), &desktop).unwrap(), runner_pin);
    assert_eq!(
        fake.file("input"),
        format!("{}\n", crate::runnerlink::to_line(&desktop))
    );
    assert!(fake.file("log").contains(&format!(
        "exec -i --user 10001:10001 {ID} {PURLIS} bridge --pair"
    )),);
}

#[test]
fn a_runner_whose_container_fails_the_check_is_not_paired() {
    use purlis_session_protocol::hostlink::{LinkKey, Pinned};
    let fake = FakeDocker::new(&with(|c| {
        c["HostConfig"]["CapAdd"] = serde_json::json!(["ALL"])
    }));
    let desktop = Pinned {
        device: "01JA0DESKT0P00000000000000".into(),
        key: LinkKey::generate().unwrap().public(),
    };
    assert!(fake.docker().pair(&made(), &desktop).is_err());
    assert!(!fake.file("log").contains("exec"));
}

#[test]
fn the_user_and_home_a_runner_is_made_with_are_the_images_own() {
    let fake = FakeDocker::new(&inspected());
    fake.set(
        "image",
        r#"[{"Config":{"User":"10001:10001","Env":["PATH=/usr/bin","HOME=/home/purlis"]}}]"#,
    );
    assert_eq!(
        fake.docker().declared(&image()).unwrap(),
        (User::new(10001, 10001).unwrap(), "/home/purlis".to_owned())
    );
    for (user, env) in [
        ("", r#"["HOME=/home/purlis"]"#),
        ("purlis", r#"["HOME=/home/purlis"]"#),
        ("10001", r#"["HOME=/home/purlis"]"#),
        ("0:0", r#"["HOME=/root"]"#),
        ("10001:10001", r#"["PATH=/usr/bin"]"#),
    ] {
        fake.set(
            "image",
            &format!(r#"[{{"Config":{{"User":"{user}","Env":{env}}}}}]"#),
        );
        assert!(
            fake.docker().declared(&image()).is_err(),
            "{user} {env} was taken"
        );
    }
}

#[test]
fn removing_a_runner_removes_its_container_and_then_its_home_volume() {
    let fake = FakeDocker::new(&inspected());
    let (container, volume) = fake.docker().remove_each(&made());
    assert_eq!((container, volume), (Ok(()), Ok(())));
    let log = fake.file("log");
    let container_at = log.find(&format!("container rm --force --volumes {ID}"));
    let volume_at = log.find("volume rm --force purlis-runner-first-home");
    assert!(container_at.is_some() && container_at < volume_at, "{log}");
}

#[test]
fn a_container_or_volume_already_gone_is_removed() {
    let fake = FakeDocker::new(&inspected());
    fake.set("rm-fails", "1");
    // Neither is listed any more: whatever state it was in, it is gone.
    assert_eq!(fake.docker().remove_each(&made()), (Ok(()), Ok(())));
}

#[test]
fn a_part_that_could_not_be_removed_is_said_with_the_command_that_removes_it() {
    let fake = FakeDocker::new(&inspected());
    fake.set("rm-fails-volume", "1");
    fake.set("volumes", "purlis-runner-first-home\n");
    let (container, volume) = fake.docker().remove_each(&made());
    assert_eq!(container, Ok(()));
    let left = volume.unwrap_err().0;
    assert!(
        left.contains("home volume purlis-runner-first-home"),
        "{left}"
    );
    assert!(
        left.contains("`docker volume rm --force purlis-runner-first-home`"),
        "{left}"
    );

    let fake = FakeDocker::new(&inspected());
    fake.set("rm-fails-container", "1");
    fake.set("containers", &format!("{ID}\n"));
    let (container, volume) = fake.docker().remove_each(&made());
    let left = container.unwrap_err().0;
    assert!(
        left.contains(&format!("`docker container rm --force {ID}`")),
        "{left}"
    );
    assert_eq!(
        volume,
        Ok(()),
        "the volume is tried whatever the container did"
    );
}

#[test]
fn a_run_that_fails_after_docker_made_the_container_removes_it_and_its_volume() {
    let fake = FakeDocker::new(&inspected());
    fake.set("run-fails", "");
    let refused = fake.docker().make(&runner()).unwrap_err();
    assert!(refused.0.contains("the runtime refused it"), "{refused}");
    let log = fake.file("log");
    assert!(
        log.contains("container rm --force --volumes purlis-runner-first"),
        "{log}"
    );
    assert!(
        log.contains("volume rm --force purlis-runner-first-home"),
        "{log}"
    );
}

#[test]
fn a_runner_is_not_made_where_a_container_of_its_name_is_already_there() {
    let fake = FakeDocker::new(&inspected());
    fake.set(
        "containers",
        "purlis-runner-first-old\npurlis-runner-first\n",
    );
    let refused = fake.docker().make(&runner()).unwrap_err();
    assert!(refused.0.contains("already there"), "{refused}");
    let log = fake.file("log");
    assert!(!log.contains("run "), "{log}");
    assert!(!log.contains(" rm "), "{log}");
}

#[test]
fn docker_is_found_where_its_installers_put_it_before_the_path() {
    let (on_path, home) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let path = format!("relative:{}", on_path.path().display());
    let installed = DOCKER_INSTALLED
        .iter()
        .map(std::path::PathBuf::from)
        .find(|at| at.is_file());
    // None at home and none on the path: the error says how to get one, unless this machine has
    // Docker where an installer puts it, which a test cannot take away.
    match Docker::on_this_machine(Some(&path), Some(home.path())) {
        Err(refused) => assert!(refused.0.contains("install Docker"), "{refused}"),
        Ok(found) => assert_eq!(Some(found.program().to_path_buf()), installed),
    }
    stand_in::program(on_path.path(), "docker", "#!/bin/sh\n");
    std::fs::create_dir_all(home.path().join(".orbstack/bin")).unwrap();
    stand_in::program(&home.path().join(".orbstack/bin"), "docker", "#!/bin/sh\n");
    // The path comes last: an installer's place is taken before it.
    let found = Docker::on_this_machine(Some(&path), Some(home.path())).unwrap();
    assert_ne!(found.program(), on_path.path().join("docker"));
    assert_eq!(
        found.program(),
        installed.unwrap_or_else(|| home.path().join(".orbstack/bin/docker"))
    );
}

// ---------------------------------------------------------------------------------------------
// The check, held to what each host it was measured on wrote (#2680): `hosts/`, recorded by
// `.github/workflows/docker-pool-hosts.yml` from a container made the pool's way, and one
// loosened each way, on Docker on an AppArmor host, rootful Podman and rootless Podman.
// ---------------------------------------------------------------------------------------------

/// What rootless Podman wrote for `case`, its runtime's seccomp profile what it compiles from
/// the runner's profile now: the recording was measured on 2026-10-27, with the runner's
/// earlier profile (#3298).
fn rootless_now(case: &str) -> serde_json::Value {
    let mut oci = recorded("podman-rootless", case);
    let native = oci["linux"]["seccomp"]["architectures"][0]
        .as_str()
        .expect("its architecture")
        .to_owned();
    oci["linux"]["seccomp"] = compiled_for(SECCOMP_PROFILE, &native).expect("compiled");
    oci
}

/// What `host` wrote for `case`, as it was recorded.
fn recorded(host: &str, case: &str) -> serde_json::Value {
    let text = match format!("{host}-{case}").as_str() {
        "docker-apparmor-made" => include_str!("hosts/docker-apparmor-made.json"),
        "docker-apparmor-privileged" => include_str!("hosts/docker-apparmor-privileged.json"),
        "docker-apparmor-capability" => include_str!("hosts/docker-apparmor-capability.json"),
        "docker-apparmor-unconfined" => include_str!("hosts/docker-apparmor-unconfined.json"),
        "docker-apparmor-host-pid" => include_str!("hosts/docker-apparmor-host-pid.json"),
        "docker-apparmor-info-security" => include_str!("hosts/docker-apparmor-info-security.json"),
        "podman-rootful-made" => include_str!("hosts/podman-rootful-made.json"),
        "podman-rootful-privileged" => include_str!("hosts/podman-rootful-privileged.json"),
        "podman-rootful-capability" => include_str!("hosts/podman-rootful-capability.json"),
        "podman-rootful-unconfined" => include_str!("hosts/podman-rootful-unconfined.json"),
        "podman-rootful-host-pid" => include_str!("hosts/podman-rootful-host-pid.json"),
        "podman-rootful-info-security" => include_str!("hosts/podman-rootful-info-security.json"),
        "podman-rootless-made" => include_str!("hosts/podman-rootless-made.json"),
        "podman-rootless-privileged" => include_str!("hosts/podman-rootless-privileged.json"),
        "podman-rootless-capability" => include_str!("hosts/podman-rootless-capability.json"),
        "podman-rootless-unconfined" => include_str!("hosts/podman-rootless-unconfined.json"),
        "podman-rootless-host-pid" => include_str!("hosts/podman-rootless-host-pid.json"),
        "podman-rootless-info-security" => include_str!("hosts/podman-rootless-info-security.json"),
        "docker-apparmor-seccomp-unconfined" => {
            include_str!("hosts/docker-apparmor-seccomp-unconfined.json")
        }
        "docker-apparmor-host-users" => include_str!("hosts/docker-apparmor-host-users.json"),
        "docker-apparmor-host-network" => include_str!("hosts/docker-apparmor-host-network.json"),
        "podman-rootful-seccomp-unconfined" => {
            include_str!("hosts/podman-rootful-seccomp-unconfined.json")
        }
        "podman-rootful-host-users" => include_str!("hosts/podman-rootful-host-users.json"),
        "podman-rootful-host-network" => include_str!("hosts/podman-rootful-host-network.json"),
        "podman-rootless-seccomp-unconfined" => {
            include_str!("hosts/podman-rootless-seccomp-unconfined.json")
        }
        "podman-rootless-host-users" => include_str!("hosts/podman-rootless-host-users.json"),
        "podman-rootless-host-network" => include_str!("hosts/podman-rootless-host-network.json"),
        "podman-rootless-made-oci" => include_str!("hosts/podman-rootless-made-oci.json"),
        "podman-rootless-device-oci" => include_str!("hosts/podman-rootless-device-oci.json"),
        "podman-rootless-host-folder-oci" => {
            include_str!("hosts/podman-rootless-host-folder-oci.json")
        }
        "docker-apparmor-host-ipc" => include_str!("hosts/docker-apparmor-host-ipc.json"),
        "docker-apparmor-host-uts" => include_str!("hosts/docker-apparmor-host-uts.json"),
        "docker-apparmor-host-cgroups" => include_str!("hosts/docker-apparmor-host-cgroups.json"),
        "docker-apparmor-host-folder" => include_str!("hosts/docker-apparmor-host-folder.json"),
        "docker-apparmor-device" => include_str!("hosts/docker-apparmor-device.json"),
        "podman-rootful-host-ipc" => include_str!("hosts/podman-rootful-host-ipc.json"),
        "podman-rootful-host-uts" => include_str!("hosts/podman-rootful-host-uts.json"),
        "podman-rootful-host-cgroups" => include_str!("hosts/podman-rootful-host-cgroups.json"),
        "podman-rootful-host-folder" => include_str!("hosts/podman-rootful-host-folder.json"),
        "podman-rootful-device" => include_str!("hosts/podman-rootful-device.json"),
        "podman-rootless-host-ipc" => include_str!("hosts/podman-rootless-host-ipc.json"),
        "podman-rootless-host-uts" => include_str!("hosts/podman-rootless-host-uts.json"),
        "podman-rootless-host-cgroups" => include_str!("hosts/podman-rootless-host-cgroups.json"),
        "podman-rootless-host-folder" => include_str!("hosts/podman-rootless-host-folder.json"),
        "podman-rootless-device" => include_str!("hosts/podman-rootless-device.json"),
        "podman-rootless-store" => include_str!("hosts/podman-rootless-store.json"),
        other => panic!("nothing is recorded as {other}"),
    };
    serde_json::from_str(text).unwrap()
}

/// The container `inspected` says it is, as the pool made it: its id, its runner's name, its
/// image, and the pool's user, home and default limits.
fn made_as_recorded(inspected: &serde_json::Value) -> Made {
    let text = |pointer: &str| {
        inspected
            .pointer(pointer)
            .and_then(serde_json::Value::as_str)
            .unwrap()
    };
    Made {
        container: ContainerId::new(text("/0/Id")).unwrap(),
        runner: Runner::new(
            text("/0/Config/Labels/purlis.runner"),
            Image::pinned(text("/0/Config/Image")).unwrap(),
            User::new(10001, 10001).unwrap(),
            "/home/purlis",
        )
        .unwrap(),
    }
}

fn checked(inspected: &serde_json::Value) -> Result<(), Refused> {
    check(inspected, &made_as_recorded(inspected))
}

/// The engine each host was measured on, as its daemon's check says.
fn engine_of(host: &str) -> Engine {
    Engine {
        rootless: host == "podman-rootless",
    }
}

/// What `host` wrote, held to the check on the engine it was measured on.
fn checked_on_its_engine(host: &str, inspected: &serde_json::Value) -> Result<Seccomp, Refused> {
    check_on(inspected, &made_as_recorded(inspected), engine_of(host))
}

#[test]
fn a_container_made_the_pools_way_passes_on_docker_with_apparmor_and_on_podman() {
    // Measured on 2026-10-27, with the runner's earlier profile: Docker keeps the profile it was
    // given, and Podman the path of its file, each of its own name, so each is known as that one
    // (#3298).
    for host in ["docker-apparmor", "podman-rootful", "podman-rootless"] {
        assert_eq!(
            checked_on_its_engine(host, &recorded(host, "made")),
            Ok(Seccomp::RunnersEarlier),
            "{host}"
        );
    }
    let mut now = recorded("docker-apparmor", "made");
    now[0]["HostConfig"]["SecurityOpt"] =
        serde_json::json!(["no-new-privileges", runners_seccomp()]);
    assert_eq!(
        checked_on_its_engine("docker-apparmor", &now),
        Ok(Seccomp::Runners)
    );
    let mut now = recorded("podman-rootful", "made");
    now[0]["HostConfig"]["SecurityOpt"] = serde_json::json!([
        "no-new-privileges",
        format!("seccomp=/home/runner/work/_temp/measured/{SECCOMP_FILE}")
    ]);
    assert_eq!(
        checked_on_its_engine("podman-rootful", &now),
        Ok(Seccomp::Runners)
    );
}

/// A Podman container with no AppArmor profile is taken only on rootless Podman, which loads
/// none, and Podman's own profile is taken only by the name Podman gives it.
#[test]
fn no_apparmor_profile_is_taken_only_on_rootless_podman() {
    let rootless = recorded("podman-rootless", "made");
    assert!(checked(&rootless).is_err(), "taken on a rootful engine");
    let mut rootful = recorded("podman-rootful", "made");
    rootful[0]["AppArmorProfile"] = "".into();
    assert!(checked(&rootful).is_err());
    for (profile, taken) in [
        ("containers-default-0.57.4-apparmor1", true),
        ("containers-default-0.57.4", true),
        ("containers-default-1", true),
        ("containers-default-", false),
        ("containers-default-mine", false),
        ("containers-default-0.57.4-apparmor", false),
        ("containers-default-0.57.4-apparmor1-x", false),
        ("containers-default-0.57.4-unconfined", false),
    ] {
        let mut one = recorded("podman-rootful", "made");
        one[0]["AppArmorProfile"] = profile.into();
        assert_eq!(checked(&one).is_ok(), taken, "{profile}");
    }
}

#[test]
fn a_loosened_container_is_refused_on_every_host_it_was_measured_on() {
    for host in ["docker-apparmor", "podman-rootful", "podman-rootless"] {
        for case in [
            "privileged",
            "capability",
            "unconfined",
            "host-pid",
            "seccomp-unconfined",
            "host-users",
            "host-network",
            "host-ipc",
            "host-uts",
            "host-cgroups",
            "host-folder",
            "device",
        ] {
            // Rootless Podman writes a device only in its runtime's config, which
            // `rootless_podmans_runtime_config_is_held_to_the_pools_own` holds (#2895).
            if case == "device" && host == "podman-rootless" {
                continue;
            }
            // Podman's own user namespace is the machine's: `--userns host` is the pool's own
            // container, word for word (measured 2026-10-20).
            if case == "host-users" && host.starts_with("podman") {
                // Measured with the earlier profile's file (#3298).
                assert_eq!(
                    checked_on_its_engine(host, &recorded(host, case)),
                    Ok(Seccomp::RunnersEarlier),
                    "{host}"
                );
                continue;
            }
            assert!(
                checked_on_its_engine(host, &recorded(host, case)).is_err(),
                "{host}: a {case} container passed"
            );
        }
    }
}

#[test]
fn a_podman_container_is_held_to_what_podman_writes_and_nothing_looser() {
    let made = recorded("podman-rootful", "made");
    for (what, pointer, value) in [
        (
            "an AppArmor profile of its own",
            "/0/AppArmorProfile",
            serde_json::json!("unconfined"),
        ),
        (
            "Docker's AppArmor profile",
            "/0/AppArmorProfile",
            serde_json::json!("docker-default"),
        ),
        (
            "a capability it keeps",
            "/0/EffectiveCaps",
            serde_json::json!(["CAP_SYS_ADMIN"]),
        ),
        (
            "a capability it may gain",
            "/0/BoundingCaps",
            serde_json::json!(["CAP_SYS_ADMIN"]),
        ),
        (
            "the machine's processes",
            "/0/HostConfig/PidMode",
            serde_json::json!("host"),
        ),
        (
            "the machine's cgroups",
            "/0/HostConfig/CgroupMode",
            serde_json::json!("host"),
        ),
        (
            "a folder of this machine",
            "/0/HostConfig/Binds",
            serde_json::json!([
                "purlis-runner-measured-8198-home:/home/purlis:rw,rprivate,nosuid,nodev,rbind",
                "/:/host:rw"
            ]),
        ),
        (
            "a home that may hold a device",
            "/0/HostConfig/Binds",
            serde_json::json!(["purlis-runner-measured-8198-home:/home/purlis:rw,rprivate,rbind"]),
        ),
        (
            "the machine's network",
            "/0/NetworkSettings/Networks",
            serde_json::json!({"host": {}}),
        ),
        (
            "a /tmp that runs programs",
            "/0/HostConfig/Tmpfs",
            serde_json::json!({
                "/tmp": "rw,nosuid,nodev,size=1024m,rprivate,tmpcopyup"
            }),
        ),
        (
            "another runtime",
            "/0/OCIRuntime",
            serde_json::json!("kata"),
        ),
    ] {
        let mut loosened = made.clone();
        *loosened.pointer_mut(pointer).unwrap() = value;
        assert!(
            checked(&loosened).is_err(),
            "a Podman container with {what} passed"
        );
    }
    // Docker's own shape is held to Docker's words: a Docker container written as Podman
    // writes a home is refused.
    let mut docker = recorded("docker-apparmor", "made");
    *docker.pointer_mut("/0/HostConfig/Binds").unwrap() = serde_json::json!([
        "purlis-runner-measured-8419-home:/home/purlis:rw,rprivate,nosuid,nodev,rbind"
    ]);
    assert!(checked(&docker).is_err());
}

#[test]
fn podman_with_its_default_seccomp_profile_makes_runners_rootful_or_rootless() {
    assert_eq!(
        check_daemon(&recorded("docker-apparmor", "info-security")),
        Ok(())
    );
    for host in ["podman-rootful", "podman-rootless"] {
        let security = recorded(host, "info-security");
        assert_eq!(check_podman(&security), Ok(engine_of(host)), "{host}");
        let mut no_seccomp = security.clone();
        no_seccomp["seccompEnabled"] = serde_json::json!(false);
        assert!(check_podman(&no_seccomp).is_err(), "{host}");
        let mut its_own = security.clone();
        its_own["seccompProfilePath"] = serde_json::json!("/etc/mine.json");
        assert!(check_podman(&its_own).is_err(), "{host}");
        let mut unread = security;
        unread.as_object_mut().unwrap().remove("rootless");
        assert!(check_podman(&unread).is_err(), "{host}");
    }
}

/// An SELinux host labels each container (`ProcessLabel`, `MountLabel`), and no SELinux host
/// has been measured (#2680): a container that carries a label is refused until one is, and so
/// is one whose labels cannot be read.
#[test]
fn a_container_with_an_selinux_label_is_refused_until_such_a_host_is_measured() {
    for field in ["ProcessLabel", "MountLabel"] {
        let labelled = with(|one| one[field] = "system_u:system_r:container_t:s0:c1,c2".into());
        let refused = check(&labelled, &made()).unwrap_err();
        assert!(refused.0.contains("SELinux"), "{refused}");
        let unread = with(|one| {
            one.as_object_mut().unwrap().remove(field);
        });
        assert!(check(&unread, &made()).is_err(), "{field}");
    }
    for host in ["docker-apparmor", "podman-rootful", "podman-rootless"] {
        let mut labelled = recorded(host, "made");
        labelled[0]["ProcessLabel"] = "system_u:system_r:container_t:s0:c1,c2".into();
        assert!(checked_on_its_engine(host, &labelled).is_err(), "{host}");
    }
}

/// Docker runs a registry's credential helper (`docker-credential-desktop`, `-osxkeychain`) by
/// its name on its own `PATH`, and a window opened from the Finder has a `PATH` that names none
/// of the folders Docker's installers put one in (#2752). So `docker` is run with the helpers
/// looked for where `docker` itself is: the folder it was found in and the one its link leads
/// to, then each place Docker's installers put it, then the window's `PATH`, of its absolute
/// folders alone.
#[test]
fn docker_looks_for_a_credential_helper_where_docker_itself_is_found() {
    let (found, linked, home) = (
        tempfile::tempdir().unwrap(),
        tempfile::tempdir().unwrap(),
        tempfile::tempdir().unwrap(),
    );
    stand_in::program(linked.path(), "docker", "#!/bin/sh\n");
    std::os::unix::fs::symlink(linked.path().join("docker"), found.path().join("docker")).unwrap();
    // Each installer's folder is there as itself: its `bin` is looked in.
    for holder in [".docker", ".orbstack", ".rd"] {
        std::fs::create_dir_all(home.path().join(holder)).unwrap();
    }
    let docker = Docker::at(found.path().join("docker"));
    let searched =
        docker.helpers_searched(Some("relative:/usr/bin:/opt/tools/bin"), Some(home.path()));
    let real = |path: &std::path::Path| path.canonicalize().unwrap();
    let mut expected = vec![found.path().to_path_buf(), real(linked.path())];
    expected.extend(
        DOCKER_INSTALLED
            .iter()
            .map(|at| std::path::Path::new(at).parent().unwrap().to_path_buf()),
    );
    expected.extend(
        DOCKER_IN_HOME
            .iter()
            .map(|at| home.path().join(at).parent().unwrap().to_path_buf()),
    );
    // `/usr/bin` once, where an installer puts `docker`; the relative folder not at all.
    expected.push("/opt/tools/bin".into());
    assert_eq!(searched, expected);
}

/// A run of `docker` is given Docker's own folders and the system's as its `PATH` (#2752),
/// whatever `PATH` the window was opened with: where every helper it is handed was found, so a
/// window opened from the Finder finds them too.
#[test]
fn a_run_of_docker_is_given_dockers_own_folders_as_its_path() {
    let dir = tempfile::tempdir().unwrap();
    stand_in::program(dir.path(), "docker", "#!/bin/sh\necho \"$PATH\"\n");
    let said = Docker::at(dir.path().join("docker"))
        .with_settings_in(dir.path().join("settings"))
        .run(&["version".into()], None, MOST_INSPECTED)
        .unwrap();
    let expected = std::env::join_paths(settings::run_path()).unwrap();
    assert_eq!(said.trim(), expected.to_str().unwrap());
}

#[test]
fn an_image_this_machine_does_not_hold_is_pulled_by_its_digest_before_it_is_read() {
    let fake = FakeDocker::new(&inspected());
    // No image held: it is pulled by its digest, and read again.
    assert!(fake.docker().declared(&image()).is_err());
    let log = fake.file("log");
    assert!(
        log.contains(&format!("pull --quiet {}", image().as_str())),
        "{log}"
    );
    assert_eq!(log.matches("image inspect").count(), 2, "{log}");
    // An image held is read, and nothing is pulled.
    let fake = FakeDocker::new(&inspected());
    fake.set(
        "image",
        r#"[{"Config":{"User":"10001:10001","Env":["HOME=/home/purlis"]}}]"#,
    );
    assert!(fake.docker().declared(&image()).is_ok());
    assert!(!fake.file("log").contains("pull"), "{}", fake.file("log"));
}

/// Every run of `docker` is given purlis's checked copy of Docker's settings, in a folder made
/// for it that goes once it has run, and no context (#2752, the person's ruling of 2026-10-20).
#[test]
fn every_run_of_docker_is_given_a_checked_copy_of_its_settings_and_no_context() {
    let dir = tempfile::tempdir().unwrap();
    // The copy is kept aside to be read here, and never printed: it holds sign-ins.
    let kept = dir.path().join("copy.json");
    stand_in::program(
        dir.path(),
        "docker",
        &format!(
            "#!/bin/sh\necho \"$DOCKER_CONFIG\"\necho \"${{DOCKER_CONTEXT-unset}}\"\ncp \"$DOCKER_CONFIG/config.json\" '{}'\n",
            kept.display()
        ),
    );
    let settings_in = dir.path().join("settings");
    let docker = Docker::at(dir.path().join("docker")).with_settings_in(&settings_in);
    let said = docker
        .run(&["version".into()], None, MOST_INSPECTED)
        .unwrap();
    let mut lines = said.lines();
    let folder = std::path::PathBuf::from(lines.next().unwrap());
    assert!(folder.starts_with(&settings_in), "{}", folder.display());
    assert_eq!(lines.next(), Some("unset"));
    let copy: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&kept).unwrap()).unwrap();
    let keys: Vec<&String> = copy.as_object().unwrap().keys().collect();
    for gone in ["cliPluginsExtraDirs", "plugins", "currentContext"] {
        assert!(copy.get(gone).is_none(), "{keys:?}");
    }
    // The run's folder went with it.
    assert!(!folder.exists());
}

/// A `docker`, or a helper's folder, in the home is taken only where the installer's folder
/// that holds it is there as itself (#2752): not a link to somewhere else, and not another
/// spelling of its name, which a chat could have made before its sandbox held the name.
#[test]
fn a_home_candidate_is_taken_only_from_the_installers_own_folder() {
    // As on a machine where no installer put one in its own place.
    let installed: Option<std::path::PathBuf> = None;
    let found = |home: &std::path::Path| {
        Docker::found_in(&[], Some("relative:"), Some(home))
            .ok()
            .map(|docker| docker.program().to_path_buf())
    };
    // The installer's own folder, as itself: taken.
    let own = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(own.path().join(".orbstack/bin")).unwrap();
    stand_in::program(&own.path().join(".orbstack/bin"), "docker", "#!/bin/sh\n");
    assert!(its_own_in(own.path(), ".orbstack/bin/docker"));
    assert_eq!(
        found(own.path()),
        Some(
            installed
                .clone()
                .unwrap_or_else(|| own.path().join(".orbstack/bin/docker"))
        )
    );
    // A link in its place, to a folder holding a `docker`: passed over.
    let (linked, elsewhere) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    std::fs::create_dir_all(elsewhere.path().join("bin")).unwrap();
    stand_in::program(&elsewhere.path().join("bin"), "docker", "#!/bin/sh\n");
    std::os::unix::fs::symlink(elsewhere.path(), linked.path().join(".orbstack")).unwrap();
    // Another spelling of its name, which a disk that folds case opens as the same folder.
    let spelled = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(spelled.path().join(".ORBSTACK/bin")).unwrap();
    stand_in::program(
        &spelled.path().join(".ORBSTACK/bin"),
        "docker",
        "#!/bin/sh\n",
    );
    for home in [linked.path(), spelled.path()] {
        assert!(
            !its_own_in(home, ".orbstack/bin/docker"),
            "{}",
            home.display()
        );
        assert_eq!(found(home), installed, "{}", home.display());
        let searched = Docker::at("/nonexistent/docker").helpers_searched(None, Some(home));
        assert!(
            !searched.contains(&home.join(".orbstack/bin")),
            "{}: {searched:?}",
            home.display()
        );
    }
}

#[test]
fn an_image_says_its_name_and_digest_and_whether_it_is_purlis_own() {
    let own = Image::pinned(&format!("ghcr.io/purlis/purlis-runner:main@{DIGEST}")).unwrap();
    assert_eq!(own.name(), "ghcr.io/purlis/purlis-runner");
    assert_eq!(own.digest(), DIGEST.strip_prefix("sha256:").unwrap());
    assert!(own.is_purlis_own());
    let team = Image::pinned(&format!("localhost:5000/team/runner@{DIGEST}")).unwrap();
    assert_eq!(team.name(), "localhost:5000/team/runner");
    assert!(!team.is_purlis_own());
    assert!(
        !Image::pinned(&format!("ghcr.io/purlis/purlis-runner-fork@{DIGEST}"))
            .unwrap()
            .is_purlis_own()
    );
}

#[test]
fn the_version_purlis_says_is_read_from_its_version_line_alone() {
    assert_eq!(
        version_in("purlis 0.4.2-dev.17 (dev-release profile)\n"),
        Some("0.4.2-dev.17")
    );
    assert_eq!(version_in("purlis 0.4.2 (release profile)"), Some("0.4.2"));
    for not in [
        "",
        "purlis",
        "charter 0.4.2 (release profile)",
        "purlis 0.4.2",
        "purlis 0.4.2 release",
    ] {
        assert_eq!(version_in(not), None, "{not:?}");
    }
}

#[test]
fn a_runners_version_is_asked_of_its_own_purlis_once_its_container_passes_the_check() {
    let fake = FakeDocker::new(&inspected());
    fake.set("paired", "purlis 0.4.2-dev.17 (dev-release profile)\n");
    let made = made();
    assert_eq!(fake.docker().version_of(&made).unwrap(), "0.4.2-dev.17");
    assert!(
        fake.file("log").contains(&format!(
            "exec -i --user 10001:10001 {ID} /usr/local/bin/purlis --version"
        )),
        "{}",
        fake.file("log")
    );
    fake.set("paired", "something else\n");
    assert!(fake.docker().version_of(&made).is_err());
}

/// The pool's runner as rootless Podman's runtime was asked to make it, on the run that recorded
/// `hosts/podman-rootless-*-oci.json`: its container's id read from what was recorded.
fn rootless_runner(name: &str, oci: &serde_json::Value) -> Made {
    let id = oci["mounts"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|mount| mount["source"].as_str())
        .find_map(|source| source.split("/overlay-containers/").nth(1))
        .and_then(|rest| rest.split('/').next())
        .unwrap()
        .to_owned();
    Made {
        container: ContainerId::new(&id).unwrap(),
        runner: Runner::new(
            name,
            Image::pinned(&format!("localhost:5000/pool-stand-in@{DIGEST}")).unwrap(),
            User::new(10001, 10001).unwrap(),
            "/home/purlis",
        )
        .unwrap(),
    }
}

/// Where rootless Podman kept that run's container, and its runtime config's path for `made`.
fn rootless_store(made: &Made) -> (PodmanStore, std::path::PathBuf) {
    let store = podman_store(&recorded("podman-rootless", "store")).unwrap();
    let config = store
        .graph_root
        .join("overlay-containers")
        .join(made.container.as_str())
        .join("userdata/config.json");
    (store, config)
}

/// Rootless Podman writes a device it was given nowhere `inspect` reads, only as a mount in what
/// it asked its runtime for (measured 2026-10-21, #2895). So on rootless Podman that, too, is
/// held to the pool's own: its devices none, and its mounts the runtime's own, the container's
/// own files from its own folders of Podman's storage, and its home's volume from Podman's
/// volumes, by exactly those paths.
#[test]
fn rootless_podmans_runtime_config_is_held_to_the_pools_own() {
    let own = rootless_now("made-oci");
    let made = rootless_runner("measured-8364", &own);
    let (store, config) = rootless_store(&made);
    assert_eq!(
        check_oci(&own, &made, &config, &store, Seccomp::Runners),
        Ok(())
    );
    for (case, name) in [
        ("device-oci", "device-8364"),
        ("host-folder-oci", "host-folder-8364"),
    ] {
        let oci = rootless_now(case);
        let made = rootless_runner(name, &oci);
        let (store, config) = rootless_store(&made);
        assert!(
            check_oci(&oci, &made, &config, &store, Seccomp::Runners).is_err(),
            "{case} passed"
        );
    }
    let changed = |change: &dyn Fn(&mut serde_json::Value)| {
        let mut oci = own.clone();
        change(&mut oci);
        check_oci(&oci, &made, &config, &store, Seccomp::Runners)
    };
    // A device named for the runtime.
    assert!(
        changed(&|oci| oci["linux"]["devices"] = serde_json::json!([{"path": "/dev/fuse"}]))
            .is_err()
    );
    // A folder of the machine with `/userdata/` in its path, bound where the container's own is.
    let at = |to: &'static str, from: &'static str| {
        move |oci: &mut serde_json::Value| {
            for mount in oci["mounts"].as_array_mut().unwrap() {
                if mount["destination"] == to {
                    mount["source"] = from.into();
                }
            }
        }
    };
    assert!(changed(&at("/dev/shm", "/home/runner/elsewhere/userdata/shm")).is_err());
    assert!(
        changed(&at(
            "/etc/hosts",
            "/tmp/x/overlay-containers/other/userdata/hosts"
        ))
        .is_err()
    );
    // A home of a volume outside Podman's own volumes.
    assert!(
        changed(&at(
            "/home/purlis",
            "/tmp/volumes/purlis-runner-measured-8364-home/_data"
        ))
        .is_err()
    );
    // Podman's own init, as `--init` binds it read-only (#3702), and nothing else at its place.
    let init = |source: &str, options: serde_json::Value| {
        let (source, options) = (source.to_owned(), options);
        move |oci: &mut serde_json::Value| {
            oci["mounts"]
                .as_array_mut()
                .unwrap()
                .push(serde_json::json!({
                    "destination": "/run/podman-init",
                    "type": "bind",
                    "source": source.clone(),
                    "options": options.clone(),
                }));
        }
    };
    let read_only = serde_json::json!(["bind", "ro", "rprivate"]);
    let here = tempfile::tempdir().unwrap();
    let catatonit = here.path().join("catatonit");
    std::fs::write(&catatonit, "").unwrap();
    let catatonit = catatonit.display().to_string();
    assert_eq!(changed(&init(&catatonit, read_only.clone())), Ok(()));
    assert!(changed(&init("/home/runner/catatonit-not", read_only.clone())).is_err());
    assert!(changed(&init("catatonit", read_only.clone())).is_err());
    // A folder of that name is no init.
    let folder = tempfile::tempdir().unwrap();
    let named = folder.path().join("catatonit");
    std::fs::create_dir(&named).unwrap();
    assert!(changed(&init(&named.display().to_string(), read_only)).is_err());
    assert!(
        changed(&init(
            &catatonit,
            serde_json::json!(["bind", "rw", "rprivate"])
        ))
        .is_err()
    );
    // A config of another container, of another shape, or with no mounts.
    let elsewhere = store
        .graph_root
        .join("overlay-containers/other/userdata/config.json");
    assert!(check_oci(&own, &made, &elsewhere, &store, Seccomp::Runners).is_err());
    assert!(
        check_oci(
            &serde_json::json!({}),
            &made,
            &config,
            &store,
            Seccomp::Runners
        )
        .is_err()
    );
}

/// A runtime config that is no file, such as a FIFO a reader would wait on for ever, is refused
/// at once; and one that is not on this machine, as a Podman machine keeps it in its VM, says so.
#[test]
fn a_runtime_config_that_is_no_file_or_not_here_is_refused_and_said() {
    let dir = tempfile::tempdir().unwrap();
    let fifo = dir.path().join("config.json");
    assert!(
        crate::forklock::status(std::process::Command::new("mkfifo").arg(&fifo))
            .unwrap()
            .success()
    );
    let started = std::time::Instant::now();
    let refused = read_oci(&fifo).unwrap_err();
    assert!(started.elapsed() < std::time::Duration::from_secs(5));
    assert!(refused.0.contains("not a file"), "{refused}");
    let gone = read_oci(std::path::Path::new(
        "/var/home/core/.local/share/containers/storage/overlay-containers/x/userdata/config.json",
    ))
    .unwrap_err();
    assert!(gone.0.contains("Podman machine"), "{gone}");
}

// ---------------------------------------------------------------------------------------------
// #3410: the runner's seccomp profile, and the runtime's default on a runner made before it.
// ---------------------------------------------------------------------------------------------

/// `inspected` with its seccomp option taken out, as a runner made before #3410 was.
fn on_the_runtimes_default(mut inspected: serde_json::Value) -> serde_json::Value {
    inspected[0]["HostConfig"]["SecurityOpt"]
        .as_array_mut()
        .unwrap()
        .retain(|option| !option.as_str().unwrap().starts_with("seccomp="));
    inspected
}

/// A runner made before #3410 runs the runtime's own default profile, which is stricter than the
/// runner's: it is still linked, and the check says which profile it runs.
#[test]
fn a_runner_made_on_the_runtimes_default_profile_is_still_linked() {
    for host in ["docker-apparmor", "podman-rootful", "podman-rootless"] {
        assert_eq!(
            checked_on_its_engine(host, &on_the_runtimes_default(recorded(host, "made"))),
            Ok(Seccomp::RuntimeDefault),
            "{host}"
        );
    }
    assert_eq!(
        check_on(
            &on_the_runtimes_default(inspected()),
            &made(),
            Engine::default()
        ),
        Ok(Seccomp::RuntimeDefault)
    );
}

/// Rootless Podman's runtime config of a runner on the runtime's default holds a profile, which
/// `podman info`'s check holds to Podman's own; one with none is refused.
#[test]
fn rootless_podmans_runtime_config_on_the_runtimes_default_holds_a_profile() {
    let own = recorded("podman-rootless", "made-oci");
    let made = rootless_runner("measured-8364", &own);
    let (store, config) = rootless_store(&made);
    let mut default = own.clone();
    default["linux"]["seccomp"]["syscalls"]
        .as_array_mut()
        .unwrap()
        .retain(|rule| rule["names"] != serde_json::json!(["pidfd_getfd"]));
    assert_eq!(
        check_oci(&default, &made, &config, &store, Seccomp::RuntimeDefault),
        Ok(())
    );
    let mut none = own.clone();
    none["linux"].as_object_mut().unwrap().remove("seccomp");
    assert!(check_oci(&none, &made, &config, &store, Seccomp::RuntimeDefault).is_err());
    // A profile that allows every call it does not name is no default of Podman's.
    let mut open = default.clone();
    open["linux"]["seccomp"]["defaultAction"] = "SCMP_ACT_ALLOW".into();
    assert!(check_oci(&open, &made, &config, &store, Seccomp::RuntimeDefault).is_err());
}

/// Docker's runner is held to the runner's profile rule for rule, however its text is spaced.
#[test]
fn dockers_runner_is_held_to_the_runners_rules_however_they_are_spaced() {
    let with = |profile: String| {
        let mut one = inspected();
        one[0]["HostConfig"]["SecurityOpt"] =
            serde_json::json!(["no-new-privileges", format!("seccomp={profile}")]);
        check_on(&one, &made(), Engine::default())
    };
    assert_eq!(with(SECCOMP_PROFILE.to_owned()), Ok(Seccomp::Runners));
    // A runner made with the earlier profile (#3410 to #3298) is still known, and linked.
    assert_eq!(
        with(EARLIER_SECCOMP_PROFILE.to_owned()),
        Ok(Seccomp::RunnersEarlier)
    );
    let mut more: serde_json::Value = serde_json::from_str(SECCOMP_PROFILE).unwrap();
    more["syscalls"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({"names": ["unshare"], "action": "SCMP_ACT_ALLOW"}));
    assert!(with(more.to_string()).is_err(), "one call more");
    assert!(with("not json".to_owned()).is_err());
}

/// Podman keeps only the path it read the runner's profile from: held to an absolute path to a
/// file of the runner's profile's name.
#[test]
fn podmans_runner_profile_is_named_by_an_absolute_path_to_its_file() {
    for (option, taken) in [
        ("seccomp=/run/purlis/run-x/runner-seccomp.json", true),
        ("seccomp=/run/purlis/run-x/runner-seccomp-engine.json", true),
        ("seccomp=run-x/runner-seccomp.json", false),
        ("seccomp=runner-seccomp.json", false),
        ("seccomp=/run/purlis/run-x/seccomp.json", false),
        ("seccomp=/run/purlis/run-x/runner-seccomp.json.old", false),
    ] {
        let mut one = recorded("podman-rootful", "made");
        one[0]["HostConfig"]["SecurityOpt"] = serde_json::json!(["no-new-privileges", option]);
        assert_eq!(
            checked_on_its_engine("podman-rootful", &one).is_ok(),
            taken,
            "{option}"
        );
    }
}

/// What rootless Podman compiled from the runner's profile is held to exactly what it makes of
/// it for a runner with no capability: one rule more, less or changed is refused.
#[test]
fn rootless_podmans_compiled_profile_is_exactly_the_runners() {
    let own = rootless_now("made-oci");
    let made = rootless_runner("measured-8364", &own);
    let (store, config) = rootless_store(&made);
    let changed = |change: &dyn Fn(&mut serde_json::Value)| {
        let mut oci = own.clone();
        change(&mut oci["linux"]["seccomp"]);
        check_oci(&oci, &made, &config, &store, Seccomp::Runners)
    };
    assert_eq!(changed(&|_| {}), Ok(()));
    fn rules(seccomp: &serde_json::Value) -> Vec<serde_json::Value> {
        seccomp["syscalls"].as_array().unwrap().clone()
    }
    let cases: Vec<(&str, Change)> = vec![
        (
            "one call more",
            Box::new(|seccomp| {
                let mut more = rules(seccomp);
                more.push(serde_json::json!({"names": ["unshare"], "action": "SCMP_ACT_ALLOW"}));
                seccomp["syscalls"] = more.into();
            }),
        ),
        (
            "a rule widened",
            Box::new(|seccomp| {
                seccomp["syscalls"][0]["names"]
                    .as_array_mut()
                    .unwrap()
                    .push("mount".into());
            }),
        ),
        (
            "no pidfd_getfd rule",
            Box::new(|seccomp| {
                let mut less = rules(seccomp);
                less.retain(|rule| rule["names"] != serde_json::json!(["pidfd_getfd"]));
                seccomp["syscalls"] = less.into();
            }),
        ),
        (
            "pidfd_getfd behind a condition",
            Box::new(|seccomp| {
                for rule in seccomp["syscalls"].as_array_mut().unwrap() {
                    if rule["names"] == serde_json::json!(["pidfd_getfd"]) {
                        rule["args"] =
                            serde_json::json!([{"index": 0, "value": 1, "op": "SCMP_CMP_EQ"}]);
                    }
                }
            }),
        ),
        (
            "every call allowed but those named",
            Box::new(|seccomp| seccomp["defaultAction"] = "SCMP_ACT_ALLOW".into()),
        ),
        (
            "another architecture",
            Box::new(|seccomp| seccomp["architectures"] = serde_json::json!(["SCMP_ARCH_AARCH64"])),
        ),
        (
            "no profile",
            Box::new(|seccomp| *seccomp = serde_json::Value::Null),
        ),
    ];
    for (case, change) in cases {
        assert!(changed(&*change).is_err(), "{case} passed");
    }
    let mut none = own.clone();
    none["linux"].as_object_mut().unwrap().remove("seccomp");
    assert!(check_oci(&none, &made, &config, &store, Seccomp::Runners).is_err());
}

/// The runner's profile is the Moby project's default, as pinned beside it byte for byte
/// (`seccomp-moby-v0.2.4.json`), with four rules added after its first: `pidfd_getfd` (#3410),
/// and `clone` and `unshare` making no namespace but a user, a process and a network one, and
/// `chroot`, for the browser engine's sandbox (#3298). The earlier profile is the first alone.
#[test]
fn the_runners_profile_is_the_pinned_upstream_default_and_four_rules() {
    let upstream: serde_json::Value =
        serde_json::from_str(include_str!("seccomp-moby-v0.2.4.json")).expect("JSON");
    // CLONE_NEWNS, CLONE_NEWCGROUP, CLONE_NEWUTS and CLONE_NEWIPC: refused.
    let no_other_namespace =
        serde_json::json!([{"index": 0, "value": 0x0E02_0000u32, "op": "SCMP_CMP_MASKED_EQ"}]);
    let added = [
        serde_json::json!({"names": ["pidfd_getfd"], "action": "SCMP_ACT_ALLOW"}),
        serde_json::json!({
            "names": ["clone"],
            "action": "SCMP_ACT_ALLOW",
            "args": no_other_namespace,
            "excludes": {"arches": ["s390", "s390x"]},
        }),
        // CLONE_NEWTIME too, which `unshare` alone takes (in `clone` it is part of the signal).
        serde_json::json!({
            "names": ["unshare"],
            "action": "SCMP_ACT_ALLOW",
            "args": [{"index": 0, "value": 0x0E02_0080u32, "op": "SCMP_CMP_MASKED_EQ"}],
        }),
        serde_json::json!({"names": ["chroot"], "action": "SCMP_ACT_ALLOW"}),
    ];
    let with = |rules: &[serde_json::Value]| {
        let mut it = upstream.clone();
        let syscalls = it["syscalls"].as_array_mut().unwrap();
        for (at, rule) in rules.iter().enumerate() {
            syscalls.insert(1 + at, rule.clone());
        }
        it
    };
    let ours: serde_json::Value = serde_json::from_str(SECCOMP_PROFILE).expect("JSON");
    assert_eq!(ours, with(&added));
    let earlier: serde_json::Value = serde_json::from_str(EARLIER_SECCOMP_PROFILE).expect("JSON");
    assert_eq!(earlier, with(&added[..1]));
}

/// Rootless Podman's runtime config of a runner made with the earlier profile is held to what
/// Podman makes of that profile, and is not taken as the runner's own (#3298).
#[test]
fn rootless_podmans_runner_of_the_earlier_profile_is_told_apart() {
    let earlier = recorded("podman-rootless", "made-oci");
    let made = rootless_runner("measured-8364", &earlier);
    let (store, config) = rootless_store(&made);
    assert_eq!(
        check_oci(&earlier, &made, &config, &store, Seccomp::RunnersEarlier),
        Ok(())
    );
    assert!(check_oci(&earlier, &made, &config, &store, Seccomp::Runners).is_err());
    let now = rootless_now("made-oci");
    assert!(check_oci(&now, &made, &config, &store, Seccomp::RunnersEarlier).is_err());
}

#[test]
fn a_suspended_runner_is_checked_as_it_was_made_then_started_again() {
    let mut stopped = inspected();
    stopped[0]["State"]["Running"] = false.into();
    let fake = FakeDocker::new(&stopped);
    // The fake's container stays stopped, so the whole check after the start refuses it: it was
    // started once it was checked as made, then stopped again, and not linked.
    // Seen red: with the stop after a refused check taken out, no `stop` is in the log.
    let resumed = fake.docker().resumed(&made());
    assert!(
        resumed.is_err(),
        "a container refused its check after the start is not linked"
    );
    let log = fake.file("log");
    assert!(log.contains(&format!("start {ID}")), "{log}");
    assert!(log.contains(&format!("stop {ID}")), "{log}");
}

#[test]
fn a_suspended_runner_not_as_it_was_made_is_never_started() {
    let mut changed = inspected();
    changed[0]["State"]["Running"] = false.into();
    changed[0]["HostConfig"]["Privileged"] = true.into();
    let fake = FakeDocker::new(&changed);
    // Seen red: with the check before the start taken out, the log holds a `start`.
    let resumed = fake.docker().resumed(&made());
    assert!(
        resumed
            .as_ref()
            .is_err_and(|why| why.0.contains("privileged")),
        "refused as privileged, before any start"
    );
    assert!(!fake.file("log").contains("start "), "{}", fake.file("log"));
}

#[test]
fn a_running_runner_is_only_checked() {
    let fake = FakeDocker::new(&inspected());
    assert_eq!(
        fake.docker()
            .resumed(&made())
            .map(|resumed| resumed.started),
        Ok(false)
    );
    assert!(fake.docker().running(&made()).unwrap());
    assert!(!fake.file("log").contains("start "), "{}", fake.file("log"));
}

/// The earlier profile as `docker inspect` writes it back.
fn earlier_seccomp() -> String {
    let profile: serde_json::Value = serde_json::from_str(EARLIER_SECCOMP_PROFILE).expect("JSON");
    format!("seccomp={profile}")
}

/// D-2050-26: only an image whose `purlis` holds what it starts from namespaces is made with the
/// profile that lets the browser engine make its sandbox; any other gets the earlier profile, and
/// is linked on it.
#[test]
fn a_runner_whose_purlis_holds_nothing_from_namespaces_is_made_with_the_earlier_profile() {
    let mut earlier = inspected();
    earlier[0]["HostConfig"]["SecurityOpt"] =
        serde_json::json!(["no-new-privileges", earlier_seccomp()]);
    let fake = FakeDocker::new(&earlier);
    fake.set("holds", "1");
    let made = fake
        .docker()
        .make(&runner())
        .expect("made on the earlier profile");
    assert_eq!(made.container.as_str(), ID);
    let log = fake.file("log");
    assert!(log.contains("--entrypoint /usr/local/bin/purlis"), "{log}");
    assert!(log.contains(EARLIER_SECCOMP_FILE), "{log}");
    assert!(!log.contains(SECCOMP_FILE), "{log}");

    // An image whose purlis holds them is made with the runner's own; one made so on the earlier
    // profile anyway is refused and removed.
    let fake = FakeDocker::new(&earlier);
    assert!(fake.docker().make(&runner()).is_err());
    let fake = FakeDocker::new(&inspected());
    assert!(fake.docker().make(&runner()).is_ok());
    assert!(fake.file("log").contains(SECCOMP_FILE));
}

/// The id the fake engine gives a probe's container.
const PROBE_ID: &str = "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee";

/// **The probe of an image's `purlis` names no container, and removes only its own** (#3702): a
/// runner may be named `<pool>-probe`, and making pool `<pool>`'s runner leaves it be.
#[test]
fn a_pool_named_after_another_with_probe_survives_that_pools_runner_being_made() {
    let mut earlier = inspected();
    earlier[0]["HostConfig"]["SecurityOpt"] =
        serde_json::json!(["no-new-privileges", earlier_seccomp()]);
    let fake = FakeDocker::new(&earlier);
    fake.set("holds", "1");
    fake.docker()
        .make(&runner())
        .expect("made on the earlier profile");

    let log = fake.file("log");
    assert!(!log.contains("purlis-runner-first-probe"), "{log}");
    let probe = log
        .lines()
        .find(|line| line.contains("_no-namespaces"))
        .expect("the probe ran");
    assert!(!probe.contains("--name"), "{probe}");
    assert!(probe.contains("--cidfile"), "{probe}");
    // Its own container, by the id the engine gave it, and nothing else of a probe's.
    assert!(log.contains(&format!("rm --force {PROBE_ID}\n")), "{log}");
    assert_eq!(
        log.lines().filter(|line| line.starts_with("rm ")).count(),
        1,
        "{log}"
    );
}

/// D-2050-26: a runner on the profile that lets the browser engine make namespaces, whose purlis
/// does not hold its programs from them, is not linked.
#[test]
fn a_runner_on_the_engines_profile_whose_purlis_holds_nothing_is_not_linked() {
    let fake = FakeDocker::new(&inspected());
    assert_eq!(fake.docker().check(&made()), Ok(()));
    fake.set("holds", "1");
    let refused = fake.docker().check(&made()).unwrap_err();
    assert!(
        refused.0.contains("does not hold its programs"),
        "{refused}"
    );
    // A runner on the earlier profile is not asked.
    let mut earlier = inspected();
    earlier[0]["HostConfig"]["SecurityOpt"] =
        serde_json::json!(["no-new-privileges", earlier_seccomp()]);
    let fake = FakeDocker::new(&earlier);
    fake.set("holds", "1");
    assert_eq!(fake.docker().check(&made()), Ok(()));
}

/// What Docker says of a container's latest run is read in seconds (#3492): a time Docker never
/// set (`0001-01-01T00:00:00Z`, a container never started) is no time, and so is one it did not
/// write as RFC 3339.
#[test]
fn a_containers_latest_run_is_read_in_seconds_and_an_unset_time_is_none() {
    let mut running = inspected();
    running[0]["State"]["StartedAt"] = "2026-10-26T09:00:00.123456789Z".into();
    running[0]["State"]["FinishedAt"] = "0001-01-01T00:00:00Z".into();
    assert_eq!(
        ran(&running),
        Ran {
            running: true,
            started: Some(1_793_005_200),
            finished: None,
        }
    );
    let mut stopped = inspected();
    stopped[0]["State"] = serde_json::json!({
        "Running": false,
        "StartedAt": "2026-10-26T11:00:00+02:00",
        "FinishedAt": "yesterday",
    });
    assert_eq!(
        ran(&stopped),
        Ran {
            running: false,
            started: Some(1_793_005_200),
            finished: None,
        }
    );
}

// ---------------------------------------------------------------------------------------------
// Every run that only reads is held to a time (#3546)

/// A `docker` that never answers what only reads (an inspect, an `info`, a list, an exec) and
/// pulls in three seconds, in `dir`, its reads held to `within`. A time of seconds, not
/// milliseconds, where the stand-in must answer in it: a loaded machine starts a shell slowly.
fn a_wedged_docker(dir: &std::path::Path, within: std::time::Duration) -> Docker {
    let pulled = dir.join("pulled").display().to_string();
    stand_in::program(
        dir,
        "docker",
        &format!(
            "#!/bin/sh\ncase \"$1 $2\" in\n  \
             'pull '*) sleep 3; touch '{pulled}' ;;\n  \
             'image inspect') [ -f '{pulled}' ] || exit 1; \
             echo '[{{\"Config\":{{\"User\":\"10001:10001\",\"Env\":[\"HOME=/home/purlis\"]}}}}]' ;;\n  \
             *) exec sleep 30 ;;\nesac\n"
        ),
    );
    Docker::at(dir.join("docker"))
        .with_settings_in(dir.join("settings"))
        .within(within)
}

#[test]
fn every_run_that_only_reads_is_ended_when_its_time_is_up_and_says_it_did_not_answer() {
    let dir = tempfile::tempdir().unwrap();
    let docker = a_wedged_docker(dir.path(), std::time::Duration::from_millis(300));
    let made = made();
    type Read<'a> = Box<dyn Fn() -> Option<String> + 'a>;
    let reads: [(&str, Read<'_>); 5] = [
        ("ran", Box::new(|| docker.ran(&made).err().map(|why| why.0))),
        (
            "running",
            Box::new(|| docker.running(&made).err().map(|why| why.0)),
        ),
        (
            "check",
            Box::new(|| docker.check(&made).err().map(|why| why.0)),
        ),
        (
            "version_of",
            Box::new(|| docker.version_of(&made).err().map(|why| why.0)),
        ),
        (
            "made_suspending",
            Box::new(|| docker.made_suspending(&made).err().map(|why| why.0)),
        ),
    ];
    for (read, asked) in reads {
        let started = std::time::Instant::now();
        let why = asked().unwrap_or_else(|| panic!("{read} answered a docker that never does"));
        assert!(
            started.elapsed() < std::time::Duration::from_secs(10),
            "{read} waited past its time"
        );
        assert!(why.contains("did not answer within"), "{read}: {why}");
    }
}

#[test]
fn a_pull_is_not_held_to_the_time_of_a_read() {
    let dir = tempfile::tempdir().unwrap();
    let docker = a_wedged_docker(dir.path(), std::time::Duration::from_secs(2));
    assert_eq!(
        docker.declared(&image()).unwrap(),
        (User::new(10001, 10001).unwrap(), "/home/purlis".to_owned()),
        "a pull that takes longer than a read may is waited for"
    );
}

/// A `docker` whose every run is logged in `dir/log`, that never answers an inspect or an
/// `info`, and pulls at once.
fn a_docker_that_never_answers_a_read(dir: &std::path::Path) -> Docker {
    let log = dir.join("log").display().to_string();
    stand_in::program(
        dir,
        "docker",
        &format!(
            "#!/bin/sh\necho \"$*\" >> '{log}'\ncase \"$1 $2\" in\n  \
             'pull '*) ;;\n  \
             *) exec sleep 30 ;;\nesac\n"
        ),
    );
    Docker::at(dir.join("docker"))
        .with_settings_in(dir.join("settings"))
        .within(std::time::Duration::from_secs(2))
}

#[test]
fn an_image_inspect_that_does_not_answer_pulls_nothing_and_says_so() {
    let dir = tempfile::tempdir().unwrap();
    let why = a_docker_that_never_answers_a_read(dir.path())
        .declared(&image())
        .unwrap_err();
    assert!(why.0.contains("did not answer within"), "{why}");
    let log = std::fs::read_to_string(dir.path().join("log")).unwrap_or_default();
    assert!(!log.contains("pull"), "{log}");
}

#[test]
fn an_engine_that_does_not_answer_is_not_asked_again_as_a_podman() {
    let dir = tempfile::tempdir().unwrap();
    let why = a_docker_that_never_answers_a_read(dir.path())
        .check(&made())
        .unwrap_err();
    assert!(why.0.contains("did not answer within"), "{why}");
    let log = std::fs::read_to_string(dir.path().join("log")).unwrap_or_default();
    assert_eq!(
        log.lines().filter(|line| line.starts_with("info")).count(),
        1,
        "{log}"
    );
}
