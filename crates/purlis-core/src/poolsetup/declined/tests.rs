// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

fn config() -> tempfile::TempDir {
    tempfile::tempdir().expect("a folder")
}

#[test]
fn a_failed_add_keeps_the_pool_refused_with_its_reason() {
    let config = config();
    let failed: Result<(), Refused> = Err(Refused::Other("the disk on lab is full".to_owned()));
    let answered = kept_after(config.path(), "lab", failed, |()| true);
    assert!(answered.is_err(), "the refusal is handed back as it was");
    let kept = read_choices(config.path()).expect("choices");
    let status = status_of(kept.declined.as_ref()).expect("the pool is kept");
    assert_eq!(status.pool, "lab");
    assert_eq!(
        status.state,
        PoolState::Refused {
            reason: "the disk on lab is full".to_owned()
        }
    );
}

#[test]
fn an_add_that_worked_forgets_the_refusal_of_that_pool_only() {
    let config = config();
    let _ = kept_after::<()>(
        config.path(),
        "lab",
        Err(Refused::Other("no".to_owned())),
        |()| true,
    );
    kept_after(config.path(), "other", Ok(()), |()| true).expect("ok");
    assert!(read_choices(config.path()).unwrap().declined.is_some());
    // An add that made nothing forgets nothing.
    kept_after(config.path(), "lab", Ok(()), |()| false).expect("ok");
    assert!(read_choices(config.path()).unwrap().declined.is_some());
    kept_after(config.path(), "lab", Ok(()), |()| true).expect("ok");
    assert!(read_choices(config.path()).unwrap().declined.is_none());
}

#[test]
fn the_state_is_spelled_as_the_status_bar_reads_it() {
    let said = serde_json::to_value(PoolStatus {
        pool: "lab".to_owned(),
        state: PoolState::Refused {
            reason: "no room".to_owned(),
        },
    })
    .unwrap();
    assert_eq!(
        said,
        serde_json::json!({"pool": "lab", "state": "refused", "reason": "no room"})
    );
    let said = serde_json::to_value(PoolStatus {
        pool: "lab".to_owned(),
        state: PoolState::HostKeyChanged,
    })
    .unwrap();
    assert_eq!(said["state"], "host-key-changed");
}

#[test]
fn a_long_reason_is_cut_not_kept_whole() {
    let config = config();
    let why = Refused::Other("x".repeat(5000));
    let _ = kept_after::<()>(config.path(), "lab", Err(why), |()| true);
    forget_kept(config.path()).unwrap();
    assert!(read_choices(config.path()).unwrap().declined.is_none());
    let why = Refused::Other("x".repeat(5000));
    let _ = kept_after::<()>(config.path(), "lab", Err(why), |()| true);
    let kept = read_choices(config.path()).unwrap().declined.unwrap();
    assert!(kept.reason.chars().count() <= REASON_MOST);
}
