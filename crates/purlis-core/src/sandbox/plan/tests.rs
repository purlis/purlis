// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A chat's confinement, planned in one process and started, or held, in another (#1999).

use std::sync::{Arc, Mutex};

use super::{Beside, Plan};
use crate::sandbox::egress::{PLAIN_PORTS, TUNNEL_PORTS};
use crate::sandbox::reach::{By, Reach};
use crate::sandbox::{Board, Confinement, HeldElsewhere, Placed, localport};

fn plan(given: Vec<u16>) -> Plan {
    Plan {
        reach: Reach::of(vec![
            ("api.anthropic.com".to_owned(), By::Open),
            ("grafana.example.com".to_owned(), By::Persona),
        ])
        .written(),
        tunnel_ports: TUNNEL_PORTS.to_vec(),
        plain_ports: PLAIN_PORTS.to_vec(),
        any_port: true,
        beside: Beside::ProxyOnly {
            routes: "/plane/.charter/app/ssh".into(),
        },
        given,
    }
}

/// A port a chat was given as a service on this Mac is never a port its proxy, or another
/// chat's, listens on: a plan whose given port purlis holds now starts nothing.
#[test]
fn a_plan_whose_given_port_purlis_listens_on_now_starts_nothing() {
    assert!(localport::hold_elsewhere(&[50811]));

    let started = plan(vec![50811]).start(super::Telling::default());

    assert!(started.is_err());
    localport::release(50811);
}

/// #2590: a plan the host composed for a chat it starts by its names, started leaving out a
/// service the window allowed the chat alone, asks about that service when the chat reaches for
/// it, by its loopback address, and holds the connection meanwhile.
#[test]
fn a_composed_plan_started_leaving_a_service_out_asks_about_it() {
    use std::io::Write;
    let asks = Arc::new(crate::sandbox::asks::Asks::timed(
        Arc::new(|_| {}),
        std::time::Duration::from_secs(30),
        std::time::Duration::from_millis(20),
    ));
    let routes = tempfile::tempdir().expect("a folder");
    let composed = Plan {
        beside: Beside::ProxyOnly {
            routes: routes.path().into(),
        },
        ..plan(Vec::new())
    };

    let confinement = composed
        .start_leaving_out(
            super::Telling {
                asks: Some(Arc::clone(&asks)),
                ..super::Telling::default()
            },
            vec![5432],
        )
        .expect("started");
    let mut client =
        std::net::TcpStream::connect(("127.0.0.1", confinement.placed().http)).expect("its proxy");
    client
        .write_all(b"CONNECT localhost:5432 HTTP/1.1\r\nHost: localhost:5432\r\n\r\n")
        .expect("asked");
    for _ in 0..1000 {
        if asks.holding() == 1 {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }

    assert_eq!(
        asks.holding(),
        1,
        "the connection is held while the person is asked"
    );
    assert_eq!(asks.asked(), ["127.0.0.1:5432"]);
}

/// What the session host holds of a chat's proxy, as the window reaches it.
#[derive(Debug, Default)]
struct Holder {
    withdrawn: Mutex<Vec<u16>>,
}

#[derive(Debug)]
struct Holding(Arc<Holder>);

impl HeldElsewhere for Holding {
    fn id(&self) -> &str {
        "01HOSTED"
    }

    fn withdraw_port(&self, port: u16) -> Result<(), String> {
        self.0.withdrawn.lock().expect("held").push(port);
        Ok(())
    }

    fn board(&self) -> Option<Arc<dyn Board>> {
        None
    }
}

fn placed(http: u16, socks: u16, tmp: Option<&std::path::Path>) -> Placed {
    Placed {
        http,
        socks,
        tmp: tmp.map(std::path::Path::to_path_buf),
        routed: None,
        ssh: None,
        left_out: Vec::new(),
        host_name: None,
    }
}

#[test]
fn a_proxy_held_elsewhere_is_named_by_its_ports_which_are_purlis_own_while_it_is_held() {
    let holder = Arc::new(Holder::default());

    let confinement = Confinement::held_elsewhere(
        placed(50821, 50822, None),
        Box::new(Holding(Arc::clone(&holder))),
    )
    .expect("held");

    assert_eq!(confinement.proxy_ports(), [50821, 50822]);
    assert_eq!(confinement.proxy_url(), "http://127.0.0.1:50821");
    assert_eq!(confinement.held_as(), Some("01HOSTED"));
    assert!(localport::is_own(50821) && localport::is_own(50822));
    drop(confinement);
    assert!(!localport::is_own(50821) && !localport::is_own(50822));
}

#[test]
fn a_proxy_held_elsewhere_on_a_port_a_chat_was_given_is_refused() {
    assert_eq!(localport::still_ports(&[50831]), [50831]);

    let held = Confinement::held_elsewhere(
        placed(50831, 50832, None),
        Box::new(Holding(Arc::default())),
    );

    assert!(held.is_err());
    assert!(!localport::is_own(50832), "nothing is kept of a refusal");
}

/// What holds a proxy elsewhere whose proxy asks live.
#[derive(Debug)]
struct Asking;

impl HeldElsewhere for Asking {
    fn id(&self) -> &str {
        "01HOSTED"
    }

    fn withdraw_port(&self, _port: u16) -> Result<(), String> {
        Ok(())
    }

    fn board(&self) -> Option<Arc<dyn Board>> {
        Some(Arc::new(crate::sandbox::asks::Asks::new(Arc::new(|_| {}))))
    }
}

#[test]
fn a_proxy_held_elsewhere_says_which_services_its_runner_left_out_and_what_comes_of_it() {
    let asking = Confinement::held_elsewhere(
        Placed {
            left_out: vec![5432],
            host_name: Some("runner-7".into()),
            ..placed(50851, 50852, None)
        },
        Box::new(Asking),
    )
    .expect("held");
    assert_eq!(asking.left_out(), [5432]);
    assert_eq!(asking.runner(), Some("runner-7"));
    assert_eq!(
        asking.said_of_what_was_left_out().as_deref(),
        Some(
            "The runner runner-7 does not give this chat the service on port 5432 that you \
             allowed for it alone. You are asked when the chat reaches for it there."
        )
    );

    let silent = Confinement::held_elsewhere(
        Placed {
            left_out: vec![5432, 6379],
            ..placed(50853, 50854, None)
        },
        Box::new(Holding(Arc::default())),
    )
    .expect("held");
    assert_eq!(
        silent.said_of_what_was_left_out().as_deref(),
        Some(
            "The runner this chat runs on does not give this chat the services on ports 5432 \
             and 6379 that you allowed for it alone. The chat cannot reach them there."
        )
    );

    let alone = Confinement::held_elsewhere(
        placed(50855, 50856, None),
        Box::new(Holding(Arc::default())),
    )
    .expect("held");
    assert!(alone.left_out().is_empty());
    assert_eq!(alone.said_of_what_was_left_out(), None);
}

/// **A proxy on another machine listens on that machine's ports** (#2446): a runner's ports
/// are never judged against, nor taken as, this machine's.
#[test]
fn a_proxy_held_on_another_machine_takes_none_of_this_machine_s_ports() {
    assert_eq!(localport::still_ports(&[50861]), [50861]);

    let confinement = Confinement::held_on_another_machine(
        placed(50861, 50862, None),
        Box::new(Holding(Arc::default())),
    );

    assert_eq!(confinement.proxy_ports(), [50861, 50862]);
    assert!(!localport::is_own(50861) && !localport::is_own(50862));
    // A listener of purlis's here may take the other one while it is held.
    assert!(localport::hold_elsewhere(&[50862]));
    drop(confinement);
    assert!(localport::is_own(50862), "its end lets go of nothing here");
    localport::release(50862);
}

#[test]
fn a_port_withdrawn_from_a_chat_held_elsewhere_reaches_its_holder() {
    let holder = Arc::new(Holder::default());
    let confinement = Confinement::held_elsewhere(
        placed(50841, 50842, None),
        Box::new(Holding(Arc::clone(&holder))),
    )
    .expect("held");

    confinement.withdraw_port(5432).expect("withdrawn");

    assert_eq!(*holder.withdrawn.lock().expect("held"), [5432]);
}

#[test]
fn what_a_chat_held_elsewhere_keeps_is_written_in_the_folder_its_holder_made() {
    let made = tempfile::tempdir().expect("a folder");
    let mut confinement = Confinement::held_elsewhere(
        placed(50851, 50852, Some(made.path())),
        Box::new(Holding(Arc::default())),
    )
    .expect("held");

    let kept = confinement
        .keep_own("kubeconfig", "kind: Config")
        .expect("kept");

    assert_eq!(kept, made.path().join("kubeconfig"));
    assert_eq!(confinement.tmp(), Some(made.path()));
    assert_eq!(std::fs::read_to_string(kept).expect("read"), "kind: Config");
}

/// The window answers a board in this process and one the session host holds alike.
#[test]
fn a_board_here_answers_as_a_board() {
    let board: Arc<dyn Board> = Arc::new(crate::sandbox::asks::Asks::new(Arc::new(|_| {})));
    let host = crate::sandbox::hosts::Host::parse("pypi.example.com").expect("a host");

    assert!(board.allow(&host, By::Chat).is_empty());

    assert_eq!(
        board.allowed_live(),
        [("pypi.example.com".to_owned(), By::Chat)]
    );
    board.forget(&host, By::Chat);
    assert!(board.allowed_live().is_empty());
    assert!(!board.asks_about(&host));
}

/// **A plan another machine made, held to the runner that starts it** (#2445, ruled
/// 2026-10-17): the folders it names are the runner's own for the project, every service of the
/// runner it gives is one the runner's sandbox for the project gives, and an internal address
/// is reached only where the runner's policy lists it.
mod made_elsewhere {
    use std::net::IpAddr;
    use std::path::Path;

    use super::super::{Beside, Here, Plan};
    use super::plan;
    use crate::sandbox::hosts::Network;
    use crate::sandbox::internal::Internal;
    use crate::sandbox::reach::{By, Reach};

    const ROUTES: &str = "/plane/.charter/app/ssh";

    fn ip(text: &str) -> IpAddr {
        text.parse().expect("an address")
    }

    /// How the runner resolves a name, in these tests: a few names, fixed.
    fn resolve(name: &str, _port: u16) -> Vec<IpAddr> {
        match name {
            "localhost" => vec![ip("127.0.0.1")],
            "db.corp.example" => vec![ip("203.0.113.40"), ip("10.0.0.9")],
            "metadata.google.internal" => vec![ip("169.254.169.254")],
            "api.anthropic.com" => vec![ip("203.0.113.7")],
            _ => Vec::new(),
        }
    }

    fn policy(hosts: &[&str]) -> Internal {
        Internal::listing(
            hosts
                .iter()
                .map(|host| crate::sandbox::internal::Entry::parse(host).expect("an entry"))
                .collect(),
        )
    }

    fn held_to<'a>(given: &'a [u16], own: &'a [Network], internal: &'a Internal) -> Here<'a> {
        Here {
            routes: Path::new(ROUTES),
            given,
            own,
            internal,
            resolve: &resolve,
        }
    }

    fn here<'a>(given: &'a [u16], own: &'a [Network]) -> Here<'a> {
        static NONE: std::sync::LazyLock<Internal> = std::sync::LazyLock::new(Internal::default);
        held_to(given, own, &NONE)
    }

    fn listing(entries: &[&str], given: Vec<u16>) -> Plan {
        Plan {
            reach: Reach::of(
                entries
                    .iter()
                    .map(|entry| ((*entry).to_owned(), By::You))
                    .collect(),
            )
            .written(),
            ..plan(given)
        }
    }

    #[test]
    fn a_plan_whose_ssh_route_is_in_another_folder_does_not_fit() {
        let elsewhere = Plan {
            beside: Beside::ProxyOnly {
                routes: "/tmp".into(),
            },
            ..plan(Vec::new())
        };

        let refused = elsewhere.fits(&here(&[], &[])).expect_err("refused");

        assert!(refused.contains("/tmp"), "{refused}");
        assert!(refused.contains("the runner"), "{refused}");
    }

    #[test]
    fn a_plan_whose_ssh_route_is_in_the_runners_folder_fits() {
        assert_eq!(plan(Vec::new()).fits(&here(&[], &[])), Ok(()));
    }

    #[test]
    fn a_plan_giving_a_service_the_runner_does_not_give_does_not_fit() {
        let refused = plan(vec![5432])
            .fits(&here(&[6379], &[]))
            .expect_err("refused");

        assert!(refused.contains("5432"), "{refused}");
    }

    #[test]
    fn a_plan_giving_only_services_the_runner_gives_fits() {
        let fits = listing(
            &["127.0.0.1:5432", "[::1]:5432", "localhost:5432"],
            vec![5432],
        )
        .fits(&here(&[5432, 6379], &[]));

        assert_eq!(fits, Ok(()));
    }

    #[test]
    fn a_plan_listing_a_port_of_the_runner_it_does_not_give_does_not_fit() {
        for entry in [
            "127.0.0.1:22",
            "[::1]:22",
            "[::ffff:127.0.0.1]:22",
            "127.0.0.2:5432",
            "127.255.255.254:5432",
        ] {
            let refused = listing(&[entry], vec![5432])
                .fits(&here(&[5432], &[]))
                .expect_err(entry);

            assert!(refused.contains(entry), "{refused}");
        }
    }

    #[test]
    fn a_plan_listing_an_internal_address_its_policy_does_not_list_does_not_fit() {
        let own = [Network::of(ip("203.0.113.9"), 24)];
        for entry in [
            "203.0.113.9:5432",
            "10.96.0.1:443",
            "192.168.1.20",
            "100.64.0.10:6443",
            "[fd00::1]:5432",
            "169.254.169.254:80",
            "0.0.0.0:5432",
            "[fe80::1]:22",
            "203.0.113.200:443",
        ] {
            let refused = listing(&[entry], vec![5432])
                .fits(&here(&[5432], &own))
                .expect_err(entry);

            assert!(refused.contains(entry), "{refused}");
        }
    }

    #[test]
    fn a_plan_listing_an_internal_address_its_policy_lists_fits() {
        let internal = policy(&["10.96.0.1:443", "192.168.1.20"]);

        let fits = listing(&["10.96.0.1:443", "192.168.1.20:5432"], Vec::new()).fits(&held_to(
            &[],
            &[],
            &internal,
        ));

        assert_eq!(fits, Ok(()));
    }

    #[test]
    fn a_plan_listing_a_name_the_runner_resolves_to_an_internal_address_does_not_fit() {
        let refused = listing(&["db.corp.example:5432"], Vec::new())
            .fits(&here(&[], &[]))
            .expect_err("refused");

        assert!(refused.contains("db.corp.example:5432"), "{refused}");
        assert!(refused.contains("10.0.0.9"), "{refused}");
    }

    #[test]
    fn a_name_its_policy_lists_fits_whatever_it_resolves_to_but_a_metadata_service() {
        let internal = policy(&["*.corp.example", "*.google.internal"]);
        let here = held_to(&[], &[], &internal);

        assert_eq!(
            listing(&["db.corp.example:5432"], Vec::new()).fits(&here),
            Ok(())
        );
        assert!(
            listing(&["metadata.google.internal"], Vec::new())
                .fits(&here)
                .is_err()
        );
    }

    #[test]
    fn a_plan_listing_public_hosts_and_names_under_a_domain_fits() {
        let fits = listing(
            &[
                "api.anthropic.com",
                "203.0.113.7:443",
                "localhost:22",
                "*.corp.example",
                "unresolved.example",
            ],
            Vec::new(),
        )
        .fits(&here(&[], &[]));

        assert_eq!(fits, Ok(()));
    }

    /// A plan giving the service on `port` as the window gives one the person allowed for this
    /// chat alone: its port, and the loopback entries for it, at the chat's layer.
    fn for_this_chat_alone(port: u16, given: Vec<u16>) -> Plan {
        let mut listed = vec![("api.anthropic.com".to_owned(), By::Open)];
        listed.extend(
            crate::sandbox::localport::reach_entries(port)
                .into_iter()
                .map(|entry| (entry, By::Chat)),
        );
        Plan {
            reach: Reach::of(listed).written(),
            ..plan(given)
        }
    }

    #[test]
    fn a_plan_giving_a_service_allowed_for_this_chat_alone_fits() {
        let fits = for_this_chat_alone(5432, vec![5432]).fits(&here(&[6379], &[]));

        assert_eq!(fits, Ok(()));
    }

    #[test]
    fn the_runner_starts_a_plan_without_the_services_allowed_for_this_chat_alone() {
        let planned = Plan {
            given: vec![5432, 6379],
            reach: Reach::of(vec![
                ("api.anthropic.com".to_owned(), By::Open),
                ("127.0.0.1:5432".to_owned(), By::Chat),
                ("[::1]:5432".to_owned(), By::Chat),
                ("localhost:5432".to_owned(), By::Chat),
                ("127.0.0.1:6379".to_owned(), By::You),
            ])
            .written(),
            ..plan(Vec::new())
        };

        let (started, left_out) = planned.as_the_runner_starts_it(&[6379]);

        assert_eq!(left_out, [5432]);
        assert_eq!(started.given, [6379]);
        assert_eq!(
            started.reach.listed,
            [
                ("api.anthropic.com".to_owned(), By::Open),
                ("127.0.0.1:6379".to_owned(), By::You),
            ]
        );
    }

    #[test]
    fn the_services_a_plan_gives_for_this_chat_alone_are_those_listed_at_its_layer_alone() {
        let mut planned = for_this_chat_alone(5432, vec![5432, 6379, 8080]);
        planned.reach.listed.extend(
            crate::sandbox::localport::reach_entries(6379)
                .into_iter()
                .map(|entry| (entry, By::You)),
        );

        assert_eq!(planned.given_for_this_chat_alone(), [5432]);
        assert!(plan(vec![5432]).given_for_this_chat_alone().is_empty());
    }

    #[test]
    fn a_service_the_runner_gives_is_kept_whatever_layer_gives_it() {
        let planned = for_this_chat_alone(5432, vec![5432]);

        let (started, left_out) = planned.as_the_runner_starts_it(&[5432]);

        assert!(left_out.is_empty());
        assert_eq!(started, planned);
    }

    #[test]
    fn a_service_given_at_another_layer_as_well_still_does_not_fit() {
        let mut planned = for_this_chat_alone(5432, vec![5432]);
        planned
            .reach
            .listed
            .push(("127.0.0.1:5432".to_owned(), By::You));

        let refused = planned.fits(&here(&[], &[])).expect_err("refused");

        assert!(refused.contains("5432"), "{refused}");
        assert!(planned.as_the_runner_starts_it(&[]).1.is_empty());
    }

    #[test]
    fn a_service_given_with_no_entry_of_the_chats_still_does_not_fit() {
        let refused = plan(vec![5432]).fits(&here(&[], &[])).expect_err("refused");

        assert!(refused.contains("5432"), "{refused}");
        assert!(plan(vec![5432]).as_the_runner_starts_it(&[]).1.is_empty());
    }

    #[test]
    fn a_chat_entry_on_a_port_the_plan_does_not_give_still_does_not_fit() {
        let planned = for_this_chat_alone(5432, Vec::new());

        let refused = planned.fits(&here(&[], &[])).expect_err("refused");

        assert!(refused.contains("5432"), "{refused}");
    }

    /// What a session host holds a relayed plan to: the project's own folder for ssh routes on
    /// the runner, and the services on it you let every chat of it reach.
    #[test]
    fn a_relayed_plan_fits_the_project_on_the_runner_only_as_its_own_sandbox_gives() {
        let project = tempfile::tempdir().expect("a project");
        let root = project.path();
        crate::sandbox::local::grant_port(root, 50861).expect("granted");
        let wired = |given: Vec<u16>| {
            Plan {
                beside: Beside::ProxyOnly {
                    routes: crate::sandbox::tunnel::routes_folder(root),
                },
                ..plan(given)
            }
            .wired()
        };
        let fits = |plan: &purlis_session_protocol::session::ConfinePlan| {
            crate::sandbox::hosted::fits_with(root, plan, Some(Vec::new()), &resolve)
        };

        assert_eq!(fits(&wired(vec![50861])), Ok(()));
        let refused = fits(&wired(vec![50862])).expect_err("refused");
        assert!(refused.contains("50862"), "{refused}");
        assert!(
            refused.starts_with("the runner does not start the chat's confinement"),
            "{refused}"
        );
        let elsewhere = Plan {
            beside: Beside::ProxyOnly {
                routes: ROUTES.into(),
            },
            ..plan(Vec::new())
        };
        assert!(fits(&elsewhere.wired()).is_err());
    }

    /// The runner could not tell its own addresses from another's: nothing fits.
    #[test]
    fn a_relayed_plan_does_not_fit_where_the_runner_cannot_read_its_own_addresses() {
        let project = tempfile::tempdir().expect("a project");
        let root = project.path();
        let wired = Plan {
            beside: Beside::Wrapped,
            ..plan(Vec::new())
        }
        .wired();

        let refused =
            crate::sandbox::hosted::fits_with(root, &wired, None, &resolve).expect_err("refused");

        assert!(refused.contains("its own addresses"), "{refused}");
    }
}

/// #2019: a port a chat was given for a listen grant alone that gets no door is withdrawn from
/// its proxy, so the grant leaves it no plain connect there; a port it holds a grant to connect
/// to besides is left.
#[cfg(unix)]
#[test]
fn a_listen_grant_s_port_that_gets_no_door_is_withdrawn_from_the_chat_s_proxy() {
    let holder = Arc::new(Holder::default());
    let mut confinement = Confinement::held_elsewhere(
        placed(50871, 50872, None),
        Box::new(Holding(Arc::clone(&holder))),
    )
    .expect("held");
    // No folder can be made where a file stands, so no door opens.
    let base = tempfile::tempdir().expect("a folder");
    std::fs::write(base.path().join("file"), "x").expect("a file");
    let folder = base.path().join("file").join("0123456789abcdef");
    let widened = crate::sandbox::Widened {
        ports: vec![5173, 8080],
        listens: vec![5173, 8080],
        listen_only: vec![5173],
        ..crate::sandbox::Widened::default()
    };
    assert!(confinement.serve(&folder, &widened).is_err());
    assert_eq!(*holder.withdrawn.lock().expect("held"), [5173]);
    assert_eq!(confinement.served(), None);
}
