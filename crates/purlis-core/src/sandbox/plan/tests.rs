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
