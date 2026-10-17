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

/// **A plan another machine made, held to this one** (#2445): the folders it names are this
/// machine's own for the project, and every service of this machine it gives is one this
/// machine's sandbox for the project gives.
mod made_elsewhere {
    use std::net::IpAddr;
    use std::path::Path;

    use super::super::plan::{Beside, Here, Plan};
    use super::super::reach::{By, Reach};
    use super::plan;

    const ROUTES: &str = "/plane/.charter/app/ssh";

    fn here<'a>(given: &'a [u16], own: &'a [IpAddr]) -> Here<'a> {
        Here {
            routes: Path::new(ROUTES),
            given,
            own,
        }
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
    }

    #[test]
    fn a_plan_whose_ssh_route_is_in_this_machines_folder_fits() {
        assert_eq!(plan(Vec::new()).fits(&here(&[], &[])), Ok(()));
    }

    #[test]
    fn a_plan_giving_a_service_this_machine_does_not_give_does_not_fit() {
        let refused = plan(vec![5432])
            .fits(&here(&[6379], &[]))
            .expect_err("refused");

        assert!(refused.contains("5432"), "{refused}");
    }

    #[test]
    fn a_plan_giving_only_services_this_machine_gives_fits() {
        let fits = listing(
            &["127.0.0.1:5432", "[::1]:5432", "localhost:5432"],
            vec![5432],
        )
        .fits(&here(&[5432, 6379], &[]));

        assert_eq!(fits, Ok(()));
    }

    #[test]
    fn a_plan_listing_a_port_of_this_machine_it_does_not_give_does_not_fit() {
        for entry in ["127.0.0.1:22", "[::1]:22", "[::ffff:127.0.0.1]:22"] {
            let refused = listing(&[entry], Vec::new())
                .fits(&here(&[], &[]))
                .expect_err(entry);

            assert!(refused.contains(entry), "{refused}");
        }
    }

    #[test]
    fn a_plan_listing_another_address_of_this_machine_does_not_fit() {
        let own: [IpAddr; 1] = ["10.1.2.3".parse().expect("an address")];
        for entry in [
            "10.1.2.3:5432",
            "169.254.169.254:80",
            "0.0.0.0:5432",
            "[fe80::1]:22",
        ] {
            let refused = listing(&[entry], vec![5432])
                .fits(&here(&[5432], &own))
                .expect_err(entry);

            assert!(refused.contains(entry), "{refused}");
        }
    }

    #[test]
    fn a_plan_listing_hosts_elsewhere_fits() {
        let fits = listing(
            &["api.anthropic.com", "203.0.113.7:443", "localhost:22"],
            Vec::new(),
        )
        .fits(&here(&[], &[]));

        assert_eq!(fits, Ok(()));
    }

    /// What a session host holds a relayed plan to: the project's own folder for ssh routes on
    /// this machine, and the services on this machine you let every chat of it reach.
    #[test]
    fn a_relayed_plan_fits_the_project_on_this_machine_only_as_its_own_sandbox_gives() {
        let project = tempfile::tempdir().expect("a project");
        let root = project.path();
        super::super::local::grant_port(root, 50861).expect("granted");
        let wired = |given: Vec<u16>| {
            Plan {
                beside: Beside::ProxyOnly {
                    routes: super::super::tunnel::routes_folder(root),
                },
                ..plan(given)
            }
            .wired()
        };

        assert_eq!(
            super::super::hosted::fits_here(root, &wired(vec![50861])),
            Ok(())
        );
        let refused =
            super::super::hosted::fits_here(root, &wired(vec![50862])).expect_err("refused");
        assert!(refused.contains("50862"), "{refused}");
        let elsewhere = Plan {
            beside: Beside::ProxyOnly {
                routes: "/plane/.charter/app/ssh".into(),
            },
            ..plan(Vec::new())
        };
        assert!(super::super::hosted::fits_here(root, &elsewhere.wired()).is_err());
    }
}
