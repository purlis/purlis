// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

const IMAGE: &str = "ghcr.io/purlis/purlis-runner@sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

fn adding(name: &str) -> PoolAdding {
    PoolAdding {
        name: name.into(),
        image: IMAGE.into(),
        cpus: None,
        memory_mib: None,
        processes: None,
        tmp_mib: None,
        suspend_after: None,
        frames: None,
        width: None,
    }
}

#[test]
fn cpus_are_typed_as_a_number_to_the_thousandth() {
    for (typed, millicpus) in [
        ("2", 2000),
        ("0.5", 500),
        ("1.25", 1250),
        (" 3 ", 3000),
        ("0.001", 1),
    ] {
        assert_eq!(millicpus_of(typed), Ok(millicpus), "{typed}");
    }
    for refused in ["", "two", "1.0005", "-1", "1,5", ".5", "1.", "99999999"] {
        assert!(millicpus_of(refused).is_err(), "{refused:?} was taken");
    }
}

#[test]
fn a_pool_takes_the_limits_typed_and_the_defaults_for_the_rest() {
    let pool = pool_of(&PoolAdding {
        cpus: Some("1.5".into()),
        memory_mib: Some(2048),
        ..adding("builds")
    })
    .unwrap();
    assert_eq!(
        row(&pool).limits,
        PoolLimitsShown {
            cpus: "1.5".into(),
            memory_mib: 2048,
            processes: 1024,
            tmp_mib: 1024,
        }
    );
    // An empty CPUs field is the default.
    let pool = pool_of(&PoolAdding {
        cpus: Some(" ".into()),
        ..adding("builds")
    })
    .unwrap();
    assert_eq!(row(&pool).limits.cpus, "2");
}

#[test]
fn a_pool_takes_its_browser_frame_limits_as_typed() {
    let pool = pool_of(&PoolAdding {
        frames: Some(30),
        width: Some(1920),
        ..adding("builds")
    })
    .unwrap();
    assert_eq!(
        pool.link_frames(),
        purlis_session_protocol::browsing::LinkFrames {
            frames: 30,
            width: 1920
        }
    );
}

#[test]
fn a_pool_the_core_would_refuse_is_refused_in_its_words() {
    for (adding, says) in [
        (
            PoolAdding {
                image: "ghcr.io/purlis/purlis-runner:latest".into(),
                ..adding("builds")
            },
            "digest",
        ),
        (adding("Builds"), "cannot name a pool"),
        (
            PoolAdding {
                memory_mib: Some(100),
                ..adding("builds")
            },
            "memory",
        ),
        // #3297: a pool's browser frame limits, each within its bounds.
        (
            PoolAdding {
                frames: Some(61),
                ..adding("builds")
            },
            "frames",
        ),
        (
            PoolAdding {
                width: Some(70_000),
                ..adding("builds")
            },
            "widest browser frame",
        ),
    ] {
        let refused = pool_of(&adding).unwrap_err();
        assert!(refused.contains(says), "{refused}");
    }
}

#[test]
fn the_pools_are_listed_with_a_new_pools_defaults() {
    let config = tempfile::tempdir().unwrap();
    assert_eq!(pools_on(config.path()).unwrap().pools, Vec::new());
    purlis_core::runners::record_pool(config.path(), &pool_of(&adding("builds")).unwrap()).unwrap();
    let shown = pools_on(config.path()).unwrap();
    assert_eq!(shown.pools.len(), 1);
    assert_eq!(shown.pools[0].name, "builds");
    assert_eq!(shown.pools[0].image, IMAGE);
    assert_eq!(shown.defaults.cpus, "2");
    assert_eq!(shown.defaults.memory_mib, 4096);
    assert_eq!(shown.suspend_after, "24h");
    assert_eq!(shown.pools[0].suspend_after, "24h");
}

#[test]
fn a_pool_takes_the_time_typed_before_its_runner_suspends_and_a_day_when_empty() {
    let typed = |after: Option<&str>| {
        pool_of(&PoolAdding {
            suspend_after: after.map(str::to_owned),
            ..adding("builds")
        })
        .map(|pool| row(&pool).suspend_after)
    };
    assert_eq!(typed(Some("12h")).as_deref(), Ok("12h"));
    assert_eq!(typed(Some(" 90m ")).as_deref(), Ok("90m"));
    assert_eq!(typed(Some("120m")).as_deref(), Ok("2h"));
    assert_eq!(typed(Some("")).as_deref(), Ok("24h"));
    assert_eq!(typed(None).as_deref(), Ok("24h"));
    for refused in ["a day", "1d", "10s", "1000h", "-1h"] {
        assert!(typed(Some(refused)).is_err(), "{refused:?} was taken");
    }
}

#[cfg(unix)]
#[test]
fn a_workspace_is_set_to_run_its_chats_only_on_a_pool_this_machine_keeps() {
    let config = tempfile::tempdir().unwrap();
    let plane = std::path::Path::new("/planes/a");
    purlis_core::machine::update(config.path(), |store| store.remember(plane, 1)).unwrap();
    assert_eq!(
        chats_run_on_at(config.path(), plane, "app").unwrap(),
        ChatsRunOn {
            pool: None,
            pools: Vec::new(),
        }
    );
    // A pool this machine does not keep is refused, and nothing is set.
    let refused = set_chats_run_on_at(config.path(), plane, "app", Some("builds")).unwrap_err();
    assert!(refused.contains("builds"), "{refused}");
    assert_eq!(
        chats_run_on_at(config.path(), plane, "app").unwrap().pool,
        None
    );
    // This machine is always one to choose.
    assert_eq!(
        set_chats_run_on_at(config.path(), plane, "app", None)
            .unwrap()
            .pool,
        None
    );
}

#[test]
fn a_runners_version_that_could_not_be_read_says_why_safely() {
    let (runner, why) = version_read(Err(
        "docker exec failed:\n\u{202E}tcp://someone:hidden@engine".to_owned(),
    ));
    assert_eq!(runner, None);
    assert_eq!(why.as_deref(), Some("docker exec failed: tcp://…@engine"));
    assert_eq!(
        version_read(Ok("0.9.0".to_owned())),
        (Some("0.9.0".to_owned()), None)
    );
}

#[test]
fn what_a_removal_left_and_why_a_pool_was_refused_are_said_safely() {
    assert_eq!(
        left_said(Some(
            "container: docker rm failed:\nbusy\u{200B}".to_owned()
        ))
        .as_deref(),
        Some("container: docker rm failed: busy")
    );
    assert_eq!(left_said(None), None);
    assert_eq!(
        refused_said("docker create failed: ssh://someone:hidden@host\u{202E} said no"),
        "docker create failed: ssh://…@host said no"
    );
}

/// How a runner was made to suspend is said as `runners.json` spells a pool's time (#3717).
#[cfg(unix)]
#[test]
fn how_a_runner_was_made_to_suspend_is_said_as_a_pools_time_is() {
    use purlis_core::dockerpool::MadeSuspending;
    assert_eq!(suspends_shown(MadeSuspending::Never), PoolSuspends::Never);
    assert_eq!(
        suspends_shown(MadeSuspending::After(std::time::Duration::from_secs(
            90 * 60
        ))),
        PoolSuspends::After {
            after: "90m".into()
        }
    );
    assert_eq!(
        serde_json::to_value(PoolSuspends::After {
            after: "24h".into()
        })
        .ok(),
        Some(serde_json::json!({"made": "after", "after": "24h"}))
    );
}

/// **A pool's links end once it is gone from this machine**: not while a removal left it kept, and
/// also where what this machine keeps cannot be read.
#[cfg(unix)]
#[test]
fn a_pools_links_end_once_it_is_gone_and_not_while_it_is_kept() {
    use std::os::unix::fs::PermissionsExt;
    let config = tempfile::tempdir().unwrap();
    let path = purlis_core::machine::dir(config.path()).join(purlis_core::machine::RUNNERS_FILE);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let write = |text: String| {
        std::fs::write(&path, text).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
    };
    let image = format!("ghcr.io/purlis/purlis-runner@sha256:{}", "a".repeat(64));
    write(
        serde_json::json!({
            "version": 1,
            "pools": [{ "name": "builds", "kind": "docker", "image": image }],
            "runners": [],
        })
        .to_string(),
    );
    let window = std::sync::Arc::new(crate::hostedsessions::Connection::default());
    let _pool = window.to_a_pool("builds");
    links_end_once_gone(&window, config.path(), "builds");
    assert_eq!(window.removals_of("builds"), 0, "still kept");
    write(serde_json::json!({"version": 1, "pools": [], "runners": []}).to_string());
    links_end_once_gone(&window, config.path(), "builds");
    assert_eq!(window.removals_of("builds"), 1, "gone");
    write("not json".to_owned());
    links_end_once_gone(&window, config.path(), "builds");
    assert_eq!(window.removals_of("builds"), 2, "cannot be read");
}
