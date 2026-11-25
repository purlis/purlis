// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The outage rules, as arithmetic on what a kept link told and when** (#7188, the ruling of
//! 2026-11-18), and the kept link that records it. Shared by the fault suite
//! (`a_live_remote_chat_survives_the_faults_that_define_trust`) and the systemd runner's test
//! (`an_ssh_runner_host_outlives_the_sign_out_and_a_restart`, case 5: the machine reboot).

#![cfg(unix)]

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use purlis_core::sshroute::Reach;
use purlis_core::sshroute::connect::RemoteState;
use purlis_host::{CommandConnector, NotChecked};
use purlis_session_protocol::hostlink::{self, Handshake, Identity, Linked, Liveness, Pinned};
use purlis_session_protocol::session::Client;

use super::remote_rig::bridge_args;

/// **Nothing silent for more than 30 s**: from the fault, the link says why it is down at least
/// this soon.
pub const QUIET_LIMIT: Duration = Duration::from_secs(30);
/// **The 3-minute limit**: nothing the person starts waits longer than this before it works or
/// fails.
pub const START_LIMIT: Duration = Duration::from_secs(180);

/// What a kept link told, and when.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Told {
    Up,
    Down {
        why: String,
        key_changed: bool,
        gone: bool,
        refused: bool,
        host_key_changed: bool,
    },
}

/// The first time after `fault_at` that the link said it was down, as an offset from it, with
/// what it said; `None` if it never did.
pub fn first_down_after(
    told: &[(Duration, Told)],
    fault_at: Duration,
) -> Option<(Duration, String)> {
    told.iter().find_map(|(at, what)| match what {
        Told::Down { why, .. } if *at >= fault_at => Some((*at - fault_at, why.clone())),
        _ => None,
    })
}

/// **Rule: nothing silent.** The link told it was down, with a reason, within `limit` of the
/// fault.
pub fn told_within(told: &[(Duration, Told)], fault_at: Duration, limit: Duration) -> Duration {
    let (after, why) = first_down_after(told, fault_at)
        .unwrap_or_else(|| panic!("the link never said it was down: {told:?}"));
    assert!(
        after <= limit,
        "the link was silent for {after:?} (limit {limit:?}) before it said: {why}"
    );
    assert!(!why.trim().is_empty(), "the link was down with no reason");
    after
}

/// The first time after `after` that the link was up, as an offset from `after`.
pub fn first_up_after(told: &[(Duration, Told)], after: Duration) -> Option<Duration> {
    told.iter().find_map(|(at, what)| match what {
        Told::Up if *at >= after => Some(*at - after),
        _ => None,
    })
}

/// A kept link as the desktop keeps it, recording each thing it tells with its time, and the
/// client of the link that is up.
pub struct Kept {
    told: Arc<Mutex<Vec<(Duration, Told)>>>,
    client: Arc<Mutex<Option<Arc<Client>>>>,
    began: Instant,
    /// The task keeping the link; abort it to end the kept link, as the app going away does.
    pub task: tokio::task::JoinHandle<()>,
}

impl Kept {
    /// Starts keeping a link to the host, as the window does, and waits up to `within` for its
    /// first (over a slow link it takes longer than the usual 90 s). Times are measured from
    /// `began`.
    pub async fn start(
        reach: &Reach,
        binary: &str,
        identity: Identity,
        runner: Pinned,
        began: Instant,
        within: Duration,
    ) -> Kept {
        let (_folder, args) = bridge_args(reach, binary);
        // Before each dial after the first, as the app's check before a forward is (#7240): a
        // machine whose key changed ends ssh before the link's first word, which the link can
        // only read as a peer that is not a purlis host and retry. The check tells it as the
        // changed key it is, which the keeper never retries.
        let dials = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let held = reach.clone();
        let connector = CommandConnector::checked(
            reach.ssh.clone(),
            args,
            Arc::new(move || {
                if dials.fetch_add(1, std::sync::atomic::Ordering::SeqCst) == 0 {
                    return Ok(());
                }
                purlis_core::sshroute::connect::key_held(
                    &held.ssh,
                    &held.route,
                    &held.pool,
                    &held.key,
                    &held.settings_in,
                )
                .map_err(|finding| {
                    let why = purlis_core::poolsetup::connect_said(&finding);
                    match finding.remote_state() {
                        RemoteState::HostKeyChanged => NotChecked::HostKeyChanged(why),
                        _ => NotChecked::Unreachable(why),
                    }
                })
            }),
        );
        let mut links = hostlink::keep_linked(
            connector,
            Handshake::THIS_BUILD,
            identity,
            runner,
            Liveness::default(),
        );
        let told = Arc::new(Mutex::new(Vec::new()));
        let client = Arc::new(Mutex::new(None));
        let (told_in, client_in) = (Arc::clone(&told), Arc::clone(&client));
        let task = tokio::spawn(async move {
            // The folder holds the pinned known_hosts for as long as the link is kept.
            let _folder = _folder;
            let mut alive = Vec::new();
            while let Some(linked) = links.next().await {
                let at = began.elapsed();
                match linked {
                    Linked::Up(link) => {
                        let (client, views, events) = Client::new(*link);
                        alive.push((views, events));
                        *client_in.lock().unwrap() = Some(Arc::new(client));
                        links.moved();
                        told_in.lock().unwrap().push((at, Told::Up));
                    }
                    Linked::Down(down) => {
                        *client_in.lock().unwrap() = None;
                        told_in.lock().unwrap().push((
                            at,
                            Told::Down {
                                why: down.why,
                                key_changed: down.key_changed,
                                gone: down.gone,
                                refused: down.refused,
                                host_key_changed: down.host_key_changed,
                            },
                        ));
                    }
                }
            }
        });
        let kept = Kept {
            told,
            client,
            began,
            task,
        };
        kept.until_up(within, "the first link").await;
        kept
    }

    pub fn told(&self) -> Vec<(Duration, Told)> {
        self.told.lock().unwrap().clone()
    }

    pub fn now(&self) -> Duration {
        self.began.elapsed()
    }

    pub fn client(&self) -> Option<Arc<Client>> {
        self.client.lock().unwrap().clone()
    }

    /// Waits until a link is up, up to `within`.
    pub async fn until_up(&self, within: Duration, what: &str) -> Arc<Client> {
        let deadline = Instant::now() + within;
        loop {
            if let Some(client) = self.client() {
                return client;
            }
            assert!(
                Instant::now() < deadline,
                "never linked within {within:?}: {what}; told {:?}",
                self.told()
            );
            tokio::time::sleep(Duration::from_millis(250)).await;
        }
    }

    /// Waits until a link answers a call, up to `within`: after a fault is taken off, the client
    /// of the link that was up before it may be what is left, and a dead one answers nothing.
    pub async fn until_working(&self, within: Duration, what: &str) -> Arc<Client> {
        let deadline = Instant::now() + within;
        loop {
            if let Some(client) = self.client()
                && let Ok(Ok(_)) = tokio::time::timeout(Duration::from_secs(5), client.list()).await
            {
                return client;
            }
            assert!(
                Instant::now() < deadline,
                "no link answered within {within:?}: {what}; told {:?}",
                self.told()
            );
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
    }

    /// Waits until the link says it is down, up to `within` of `from`.
    pub async fn until_down(&self, from: Duration, within: Duration) {
        let deadline = Instant::now() + within + Duration::from_secs(1);
        while first_down_after(&self.told(), from).is_none() {
            assert!(
                Instant::now() < deadline,
                "the link never said it was down within {within:?}: {:?}",
                self.told()
            );
            tokio::time::sleep(Duration::from_millis(250)).await;
        }
    }
}

#[cfg(test)]
mod rules {
    use super::*;

    fn down(why: &str) -> Told {
        Told::Down {
            why: why.to_owned(),
            key_changed: false,
            gone: false,
            refused: false,
            host_key_changed: false,
        }
    }

    #[test]
    fn a_link_that_says_why_it_is_down_within_the_limit_passes() {
        let told = [
            (Duration::from_secs(0), Told::Up),
            (Duration::from_secs(12), down("no answer")),
        ];
        assert_eq!(
            told_within(&told, Duration::from_secs(5), QUIET_LIMIT),
            Duration::from_secs(7)
        );
    }

    #[test]
    #[should_panic(expected = "silent for 40s")]
    fn a_link_that_says_nothing_for_longer_than_the_limit_fails() {
        let told = [(Duration::from_secs(45), down("late"))];
        told_within(&told, Duration::from_secs(5), QUIET_LIMIT);
    }

    #[test]
    #[should_panic(expected = "never said it was down")]
    fn a_link_that_never_says_it_is_down_fails() {
        told_within(
            &[(Duration::ZERO, Told::Up)],
            Duration::from_secs(1),
            QUIET_LIMIT,
        );
    }

    #[test]
    #[should_panic(expected = "no reason")]
    fn a_link_that_is_down_with_no_reason_fails() {
        told_within(
            &[(Duration::from_secs(2), down("  "))],
            Duration::ZERO,
            QUIET_LIMIT,
        );
    }

    #[test]
    fn a_down_before_the_fault_is_not_the_faults() {
        let told = [
            (Duration::from_secs(1), down("earlier")),
            (Duration::from_secs(20), down("the fault's")),
        ];
        assert_eq!(
            first_down_after(&told, Duration::from_secs(10)),
            Some((Duration::from_secs(10), "the fault's".to_owned()))
        );
    }

    #[test]
    fn the_first_up_after_a_restore_is_measured_from_it() {
        let told = [
            (Duration::from_secs(1), Told::Up),
            (Duration::from_secs(30), down("x")),
            (Duration::from_secs(95), Told::Up),
        ];
        assert_eq!(
            first_up_after(&told, Duration::from_secs(60)),
            Some(Duration::from_secs(35))
        );
        assert_eq!(first_up_after(&told, Duration::from_secs(100)), None);
    }
}
