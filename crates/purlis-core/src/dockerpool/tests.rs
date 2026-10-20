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
    let args = runner().create_args();
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

#[test]
fn a_runner_container_keeps_the_default_seccomp_profile_and_is_never_privileged() {
    let args = runner().create_args();
    assert!(!args.iter().any(|a| a.contains("privileged")), "{args:?}");
    assert!(!args.iter().any(|a| a.contains("seccomp")), "{args:?}");
    assert!(!args.iter().any(|a| a.contains("apparmor")), "{args:?}");
    assert!(!args.iter().any(|a| a == "--cap-add"), "{args:?}");
}

#[test]
fn a_runner_container_runs_the_pinned_image_by_digest_as_its_session_host() {
    let args = runner().create_args();
    let at = args
        .iter()
        .position(|a| a == &format!("ghcr.io/purlis/purlis-runner@{DIGEST}"))
        .expect("the image is named by its digest");
    assert_eq!(&args[at + 1..], ["serve"]);
}

#[test]
fn a_runner_container_holds_its_home_on_a_volume_of_its_own_and_no_folder_of_this_machine() {
    let args = runner().create_args();
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

#[test]
fn a_runner_container_has_a_read_only_root_and_a_tmp_that_runs_no_program() {
    let args = runner().create_args();
    assert!(args.iter().any(|a| a == "--read-only"), "{args:?}");
    assert!(
        has_pair(&args, "--tmpfs", "/tmp:rw,nosuid,nodev,noexec,size=1024m"),
        "{args:?}"
    );
}

#[test]
fn a_runner_container_is_held_to_its_pools_limits_or_the_defaults() {
    let args = runner().create_args();
    for (flag, value) in [
        ("--cpus", "2"),
        ("--memory", "4096m"),
        ("--memory-swap", "4096m"),
        ("--pids-limit", "1024"),
    ] {
        assert!(has_pair(&args, flag, value), "{flag} {value}: {args:?}");
    }
    let limits = Limits::new(1500, 8192, 256, 512).unwrap();
    let limited = runner().limited(limits);
    let args = limited.create_args();
    for (flag, value) in [
        ("--cpus", "1.500"),
        ("--memory", "8192m"),
        ("--memory-swap", "8192m"),
        ("--pids-limit", "256"),
        ("--tmpfs", "/tmp:rw,nosuid,nodev,noexec,size=512m"),
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
    let args = runner().create_args();
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
        &"x".repeat(41),
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
        "HostConfig": {
            "Privileged": false,
            "SecurityOpt": ["no-new-privileges"],
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
                c["HostConfig"]["SecurityOpt"] =
                    serde_json::json!(["no-new-privileges", "apparmor=unconfined"])
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
                    serde_json::json!(["no-new-privileges", "label=disable"])
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
            "#!/bin/sh\necho \"$*\" >> '{log}'\ncase \"$1 $2\" in\n  \
             'run '*) [ -f '{run_fails}' ] && \
             {{ echo 'the runtime refused it' >&2; exit 125; }}; cat '{id}' ;;\n  \
             'container inspect') cat '{inspect}' ;;\n  \
             'volume inspect') cat '{volume}' ;;\n  \
             'volume ls') cat '{volumes}' 2>/dev/null || true ;;\n  \
             'info '*) cat '{info}' ;;\n  \
             'image inspect') cat '{image}' ;;\n  \
             'exec '*) cat > '{input}'; cat '{paired}' ;;\n  \
             'container ls') cat '{containers}' 2>/dev/null || true ;;\n  \
             *' rm') exit $(cat '{rm_fails}'-\"$1\" 2>/dev/null || cat '{rm_fails}' 2>/dev/null || echo 0) ;;\nesac\n",
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
        Docker(self.dir.path().join("docker"))
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
        Ok(found) => assert_eq!(Some(found.0), installed),
    }
    stand_in::program(on_path.path(), "docker", "#!/bin/sh\n");
    std::fs::create_dir_all(home.path().join(".orbstack/bin")).unwrap();
    stand_in::program(&home.path().join(".orbstack/bin"), "docker", "#!/bin/sh\n");
    // The path comes last: an installer's place is taken before it.
    let found = Docker::on_this_machine(Some(&path), Some(home.path())).unwrap();
    assert_ne!(found.0, on_path.path().join("docker"));
    assert_eq!(
        found.0,
        installed.unwrap_or_else(|| home.path().join(".orbstack/bin/docker"))
    );
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
