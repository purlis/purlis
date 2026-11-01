// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;
use crate::profilestore::{self, Homes, MachineSettings};

fn forget(name: &str) -> Asked {
    Asked::ForgetOffThePools {
        name: name.to_owned(),
    }
}

fn no_keep(_: &str, _: &str) -> Result<Option<String>, String> {
    panic!("no runner is asked to keep anything")
}

fn no_approve(_: &str, _: &str, _: &str) -> Result<Option<String>, String> {
    panic!("the person is asked nothing")
}

fn keep(name: &str) -> Asked {
    Asked::KeepOnAPool {
        name: name.to_owned(),
        pool: "builds".to_owned(),
    }
}

fn a_profile_on(config: &Path, dir: &Path, name: &str) {
    let store = Homes::at(config, &dir.join("data"));
    let home = dir.join("home");
    std::fs::create_dir_all(&home).unwrap();
    let env = |_: &str| -> Option<String> { None };
    profilestore::create(
        &store,
        name,
        "claude",
        &MachineSettings {
            home: &home,
            env: &env,
        },
        1,
    )
    .expect("made");
}

/// **No line takes a live profile off a runner** (#4141): what is still among this machine's
/// profiles is refused, and so is what is not a name.
#[test]
fn the_window_forgets_only_a_name_this_machine_no_longer_has() {
    let dir = tempfile::tempdir().unwrap();
    let config = dir.path().join("config");
    a_profile_on(&config, dir.path(), "work");
    let never = |_: &str| -> Result<Option<String>, String> { panic!("no runner is asked") };

    assert!(matches!(
        answer(&config, forget("work"), never, no_keep, no_approve),
        Said::Refused { why } if why.contains("still a profile")
    ));
    assert!(matches!(
        answer(&config, forget("../up"), never, no_keep, no_approve),
        Said::Refused { .. }
    ));
    assert_eq!(
        answer(
            &config,
            forget("gone"),
            |name| Ok(Some(format!("{name} kept"))),
            no_keep,
            no_approve
        ),
        Said::Asked {
            note: Some("gone kept".to_owned())
        }
    );
}

/// What stopped the window asking at all is a refusal, never "they were asked".
#[test]
fn a_window_that_could_not_ask_says_so() {
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(
        answer(
            dir.path(),
            forget("gone"),
            |_| Err("no engine".to_owned()),
            no_keep,
            no_approve
        ),
        Said::Refused {
            why: "no engine".to_owned()
        }
    );
}

/// **A keep goes only to one of this machine's own profiles** (#4112): what it does not have, and
/// what is not a name, reach no runner; one it has is passed on with its pool.
#[test]
fn the_window_keeps_only_a_profile_this_machine_has() {
    let dir = tempfile::tempdir().unwrap();
    let config = dir.path().join("config");
    a_profile_on(&config, dir.path(), "work");
    let no_forget = |_: &str| -> Result<Option<String>, String> { panic!("nothing is forgotten") };

    for name in ["gone", "../up"] {
        assert!(
            matches!(
                answer(&config, keep(name), no_forget, no_keep, no_approve),
                Said::Refused { .. }
            ),
            "{name}"
        );
    }
    assert_eq!(
        answer(
            &config,
            keep("work"),
            no_forget,
            |name, pool| { Ok(Some(format!("{name} on {pool}"))) },
            no_approve
        ),
        Said::Asked {
            note: Some("work on builds".to_owned())
        }
    );
    assert_eq!(
        answer(
            &config,
            keep("work"),
            no_forget,
            |_, _| Err("not yours".to_owned()),
            no_approve
        ),
        Said::Refused {
            why: "not yours".to_owned()
        }
    );
}

/// **A profile the runner keeps asks the window nothing** (#4112).
#[test]
fn a_profile_already_kept_asks_no_window() {
    assert_eq!(
        kept_first("builds", "work", || Ok(Some(1)), |_| panic!("no window")),
        Ok(1)
    );
}

/// **One it does not keep is kept through the window, then asked again**, and the asks say so.
#[test]
fn a_profile_not_kept_is_kept_through_the_window_and_asked_again() {
    let asks = std::cell::Cell::new(0);
    let standing = || {
        asks.set(asks.get() + 1);
        Ok((asks.get() > 1).then_some(7))
    };
    let said = kept_first("builds", "work", standing, |asked| {
        assert_eq!(asked, &keep("work"));
        Answered::Said(Said::Asked { note: None })
    });
    assert_eq!(said, Ok(7));
    assert_eq!(asks.get(), 2);
}

/// **What stopped it keeping is said**, nothing having started: no window, no answer, a refusal,
/// and a runner that still keeps none after the window asked.
#[test]
fn a_profile_that_could_not_be_kept_says_why() {
    let never = || Ok::<Option<u8>, String>(None);
    let tried = |answered: Answered| kept_first("builds", "work", never, |_| answered).unwrap_err();

    assert!(tried(Answered::NoWindow).contains("none is running"));
    assert!(tried(Answered::NoAnswer).contains("did not say in time"));
    assert!(
        tried(Answered::Said(Said::Refused {
            why: "not yours".to_owned()
        }))
        .contains("not yours")
    );
    assert!(tried(Answered::Said(Said::Asked { note: None })).contains("even after"));
}

/// With no pool there is nothing to reach and nothing to say, and no window is asked.
#[test]
fn a_machine_with_no_pool_says_nothing_and_asks_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let said = reach_the_pools(dir.path(), "work", |_| panic!("nothing to ask"));
    assert_eq!(said, "");
}

mod on_a_socket {
    use super::*;

    /// **The window's answer comes back to the command**: the note that names a runner before
    /// 1.52 among it, and a refusal in its words.
    #[test]
    fn the_command_hears_what_the_window_said() {
        let dir = tempfile::tempdir().unwrap();
        let socket = dir.path().join("purlis").join("window.sock");
        serve_on(&socket, |asked| match asked {
            Asked::ForgetOffThePools { name } if name == "work" => Said::Asked {
                note: Some("Pool builds keeps it".to_owned()),
            },
            _ => Said::Refused {
                why: "no".to_owned(),
            },
        })
        .expect("listening");
        let within = Duration::from_secs(5);

        assert_eq!(
            ask_on(&socket, &forget("work"), within),
            Answered::Said(Said::Asked {
                note: Some("Pool builds keeps it".to_owned())
            })
        );
        assert_eq!(
            ask_on(&socket, &forget("other"), within),
            Answered::Said(Said::Refused {
                why: "no".to_owned()
            })
        );
    }

    /// Nothing listening is no window, a window's old socket included, and the next window
    /// takes that socket over. One that answers is left alone.
    #[test]
    fn an_old_socket_is_no_window_and_a_live_one_is_left_alone() {
        let dir = tempfile::tempdir().unwrap();
        let socket = dir.path().join("purlis").join("window.sock");
        let within = Duration::from_millis(200);
        assert_eq!(ask_on(&socket, &forget("work"), within), Answered::NoWindow);

        crate::hostsocket::make_the_folder(&socket).unwrap();
        drop(std::os::unix::net::UnixListener::bind(&socket).unwrap());
        assert_eq!(ask_on(&socket, &forget("work"), within), Answered::NoWindow);

        let refused = |_: Asked| Said::Refused {
            why: "the first".to_owned(),
        };
        serve_on(&socket, refused).expect("an old socket is taken over");
        assert_eq!(
            serve_on(&socket, refused).unwrap_err().kind(),
            std::io::ErrorKind::AddrInUse
        );
    }

    /// A window that hears and never answers is said as that, not as no window.
    #[test]
    fn a_window_that_does_not_answer_in_time_is_said_as_that() {
        let dir = tempfile::tempdir().unwrap();
        let socket = dir.path().join("silent.sock");
        let _listening = std::os::unix::net::UnixListener::bind(&socket).unwrap();
        assert_eq!(
            ask_on(&socket, &forget("work"), Duration::from_millis(100)),
            Answered::NoAnswer
        );
    }

    /// The window's socket sits in the host socket's folder, which every sandbox denies, and
    /// is told apart for each config home as the host's is.
    #[test]
    fn the_windows_socket_is_beside_the_hosts() {
        assert_eq!(
            beside(Path::new("/run/purlis/purlisd.sock")),
            Path::new("/run/purlis/purlisd-window.sock")
        );
        assert_eq!(
            beside(Path::new("/run/purlis/purlisd-abc.sock")),
            Path::new("/run/purlis/purlisd-abc-window.sock")
        );
    }
}

fn sign_in(name: &str, pool: &str, config: &Path) -> Asked {
    Asked::ApproveSignIn {
        name: name.to_owned(),
        pool: pool.to_owned(),
        code: "A1B2C3".to_owned(),
        config: config.to_path_buf(),
    }
}

/// A `runners.json` under `config` holding the cluster pool `team`.
fn a_pool_on(config: &Path) {
    use std::os::unix::fs::PermissionsExt;
    let folder = crate::machine::dir(config);
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::set_permissions(&folder, std::fs::Permissions::from_mode(0o700)).unwrap();
    let file = folder.join(crate::machine::RUNNERS_FILE);
    let held = serde_json::json!({
        "version": 1,
        "pools": [{
            "name": "team", "kind": "cluster", "kubeconfig": "/nowhere/.kube/config",
            "context": "team", "pool": "default", "namespace": "ada",
            "operatorKey": format!("ed25519:{}", "ab".repeat(32)),
            "cluster": "3e9a4b2c-7d10-4f5e-8a6b-9c0d1e2f3a4b",
            "holds": ["/opt/homebrew/bin/kubectl"],
        }],
        "runners": [],
    });
    std::fs::write(&file, held.to_string()).unwrap();
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o600)).unwrap();
}

/// **The person is asked only of what the window's own files name**: a profile and a pool of this
/// machine's. Their answer is what comes back, a no as a refusal with its reason.
#[test]
fn the_person_is_asked_only_of_a_profile_and_a_pool_this_machine_has() {
    let dir = tempfile::tempdir().unwrap();
    let config = dir.path().join("config");
    a_profile_on(&config, dir.path(), "work");
    a_pool_on(&config);
    let no_forget = |_: &str| -> Result<Option<String>, String> { panic!("nothing is forgotten") };

    for (name, pool) in [("nobody", "team"), ("../up", "team"), ("work", "elsewhere")] {
        assert!(
            matches!(
                answer(
                    &config,
                    sign_in(name, pool, &config),
                    no_forget,
                    no_keep,
                    no_approve
                ),
                Said::Refused { .. }
            ),
            "{name} on {pool}"
        );
    }
    let yes = answer(
        &config,
        sign_in("work", "team", &config),
        no_forget,
        no_keep,
        |name, pool, code| {
            assert_eq!((name, pool, code), ("work", "team", "A1B2C3"));
            Ok(None)
        },
    );
    assert_eq!(yes, Said::Asked { note: None });

    // A command reading another config home, and a code this purlis would not write.
    let elsewhere = dir.path().join("elsewhere");
    std::fs::create_dir_all(&elsewhere).unwrap();
    let another = answer(
        &config,
        sign_in("work", "team", &elsewhere),
        no_forget,
        no_keep,
        no_approve,
    );
    assert!(matches!(another, Said::Refused { why } if why.contains("another config home")));
    let Asked::ApproveSignIn {
        name,
        pool,
        config: theirs,
        ..
    } = sign_in("work", "team", &config)
    else {
        unreachable!()
    };
    let bad_code = answer(
        &config,
        Asked::ApproveSignIn {
            name,
            pool,
            code: "<b>\u{202e}".to_owned(),
            config: theirs,
        },
        no_forget,
        no_keep,
        no_approve,
    );
    assert!(matches!(bad_code, Said::Refused { .. }));

    let no = answer(
        &config,
        sign_in("work", "team", &config),
        no_forget,
        no_keep,
        |_, _, _| Err("you said no".to_owned()),
    );
    assert_eq!(
        no,
        Said::Refused {
            why: "you said no".to_owned()
        }
    );
}

/// **Only the window's yes goes on**: no window, no answer in time and a no each stop it, and say
/// nothing was started.
#[test]
fn a_sign_in_goes_on_only_on_the_windows_yes() {
    let asked = std::cell::Cell::new(None);
    let config = Path::new("/the/config");
    let yes = approved(config, "team", "work", "A1B2C3", |what| {
        asked.set(Some(what.clone()));
        Answered::Said(Said::Asked { note: None })
    });
    assert_eq!(yes, Ok(()));
    assert_eq!(asked.take(), Some(sign_in("work", "team", config)));

    for (answered, says) in [
        (Answered::NoWindow, "none is running"),
        (Answered::NoAnswer, "did not answer in time"),
        (
            Answered::Said(Said::Refused {
                why: "you said no".to_owned(),
            }),
            "you said no",
        ),
    ] {
        let said = approved(config, "team", "work", "A1B2C3", |_| answered).unwrap_err();
        assert!(said.contains(says), "{said}");
        assert!(said.contains("othing was started"), "{said}");
    }
}

/// **A code reads as one**, so the window draws nothing else, and two differ.
#[test]
fn a_new_code_reads_as_a_code() {
    let code = a_new_code();
    assert!(a_code(&code), "{code}");
    assert_ne!(code, a_new_code());
}

/// **What waits long hears its peer hang up**, and not before.
#[test]
fn a_peer_that_closes_reads_as_hung_up() {
    let (near, far) = UnixStream::pair().unwrap();
    assert!(!hung_up(&far));
    drop(near);
    assert!(hung_up(&far));
}

/// A peer that dribbles is cut off at its whole line's deadline, however often it writes.
#[test]
fn a_line_has_one_deadline_however_often_its_peer_writes() {
    let (mut near, far) = UnixStream::pair().unwrap();
    let writing = std::thread::spawn(move || {
        while near.write_all(b"x").is_ok() {
            std::thread::sleep(Duration::from_millis(10));
        }
    });

    let began = Instant::now();
    assert_eq!(line_of(&far, Duration::from_millis(100)), None);
    assert!(
        began.elapsed() < Duration::from_secs(30),
        "well before a line's most"
    );
    drop(far);
    writing.join().unwrap();
}
