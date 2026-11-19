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
    let answered = kept_after(config.path(), "lab", "ssh://sam@lab", failed, |()| true);
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
        "ssh://sam@lab",
        Err(Refused::Other("no".to_owned())),
        |()| true,
    );
    kept_after(config.path(), "other", "ssh://sam@lab", Ok(()), |()| true).expect("ok");
    assert!(read_choices(config.path()).unwrap().declined.is_some());
    // An add that made nothing forgets nothing.
    kept_after(config.path(), "lab", "ssh://sam@lab", Ok(()), |()| false).expect("ok");
    assert!(read_choices(config.path()).unwrap().declined.is_some());
    kept_after(config.path(), "lab", "ssh://sam@lab", Ok(()), |()| true).expect("ok");
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
    let _ = kept_after::<()>(config.path(), "lab", "", Err(why), |()| true);
    let kept = read_choices(config.path()).unwrap().declined.unwrap();
    assert!(kept.reason.chars().count() <= REASON_MOST);
}

#[test]
fn forget_kept_clears_whatever_refusal_is_kept() {
    let config = config();
    keep(config.path(), "lab", "", &Refused::Other("no".to_owned())).unwrap();
    forget_kept(config.path()).unwrap();
    assert!(read_choices(config.path()).unwrap().declined.is_none());
    // Nothing kept is nothing to do.
    forget_kept(config.path()).unwrap();
}

#[test]
fn making_pool_b_clears_nothing_of_pool_a() {
    let config = config();
    keep(
        config.path(),
        "a",
        "ssh://sam@lab",
        &Refused::Other("no room".to_owned()),
    )
    .unwrap();
    forget(config.path(), "b", "ssh://sam@lab").unwrap();
    let kept = read_choices(config.path())
        .unwrap()
        .declined
        .expect("a stays");
    assert_eq!(kept.pool, "a");
}

#[test]
fn a_pool_of_the_same_name_made_elsewhere_clears_nothing() {
    let config = config();
    keep(
        config.path(),
        "a",
        "ssh://sam@lab",
        &Refused::Other("no room".to_owned()),
    )
    .unwrap();
    forget(config.path(), "a", "cluster:prod").unwrap();
    assert!(read_choices(config.path()).unwrap().declined.is_some());
    forget(config.path(), "a", "ssh://sam@lab").unwrap();
    assert!(read_choices(config.path()).unwrap().declined.is_none());
}

#[test]
fn a_refusal_kept_by_an_older_build_has_no_place_and_only_forget_kept_clears_it() {
    let config = config();
    let mut choices = read_choices(config.path()).unwrap();
    choices.declined = Some(serde_json::from_str(r#"{"pool":"a","reason":"no"}"#).unwrap());
    write_choices(config.path(), choices).unwrap();
    forget(config.path(), "a", "ssh://sam@lab").unwrap();
    assert!(read_choices(config.path()).unwrap().declined.is_some());
    forget_kept(config.path()).unwrap();
    assert!(read_choices(config.path()).unwrap().declined.is_none());
}
