// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;
use crate::profilestore::{self, Homes, MachineSettings};

fn forget(name: &str) -> Asked {
    Asked::ForgetOffThePools {
        name: name.to_owned(),
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
    let never = |_: &str| -> Option<String> { panic!("no runner is asked") };

    assert!(matches!(
        answer(&config, forget("work"), never),
        Said::Refused { why } if why.contains("still a profile")
    ));
    assert!(matches!(
        answer(&config, forget("../up"), never),
        Said::Refused { .. }
    ));
    assert_eq!(
        answer(&config, forget("gone"), |name| Some(format!("{name} kept"))),
        Said::Asked {
            note: Some("gone kept".to_owned())
        }
    );
}

/// With no pool there is nothing to reach and nothing to say, and no window is asked.
#[test]
fn a_machine_with_no_pool_says_nothing_and_asks_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let said = reach_the_pools(dir.path(), "work", |_| panic!("nothing to ask"));
    assert_eq!(said, "");
}

#[cfg(unix)]
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
            Asked::ForgetOffThePools { .. } => Said::Refused {
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
