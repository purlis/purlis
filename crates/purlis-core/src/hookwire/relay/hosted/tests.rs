// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A `secret exec` of a chat its host composed, run there on the values its desktop sends
//! (#2047): the host's half and the window's, over JSON as over the link.

use std::io::{BufRead, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};

use super::super::tests::{LINK, Shared, a_window, the_windows};
use super::super::*;
use super::InCluster;

/// The chat its host composed, with the tool `kubectl` armed.
const COMPOSED: u32 = 21;

/// A project whose chat [`COMPOSED`] its host composed in a sandbox as `devops`, with tool
/// `kubectl` handed `KUBECONFIG` from vault `team` as a file.
fn a_composed_chat() -> tempfile::TempDir {
    a_composed_chat_reaching(None)
}

/// [`a_composed_chat`], whose confinement here allows live what `live` reads (#3407).
fn a_composed_chat_reaching(live: Option<crate::hostedchats::Live>) -> tempfile::TempDir {
    use crate::sandbox::tools::{Handed as As, Ready, Tool};
    let tmp = tempfile::tempdir().unwrap();
    let mut confines = crate::sandbox::Confines::default();
    confines.tools.push(Ready {
        tool: Tool::one("kubectl", "team", "KUBECONFIG", As::File, "kubeconfig"),
        program: std::path::PathBuf::from("/opt/kube/kubectl"),
    });
    crate::hostedchats::record(
        tmp.path(),
        COMPOSED,
        crate::hostedchats::Composed {
            persona: Some("devops".into()),
            folder: tmp.path().to_path_buf(),
            harness: None,
            confines,
            live: live.map(crate::hostedchats::Reached),
        },
    );
    tmp
}

/// What chat [`COMPOSED`] asks to run with kubectl's own vault and hand-over, with claims of its
/// own that its host clears.
fn kubectl_asked() -> brokered::Ask {
    brokered::Ask {
        chat: COMPOSED,
        secret_exec: brokered::Wanted {
            vault: "team".into(),
            file: vec!["KUBECONFIG=kubeconfig".into()],
            command: vec!["/opt/kube/kubectl".into(), "get".into(), "pods".into()],
            ..Default::default()
        },
        runs_as: Some("sh".into()),
        waited: true,
        run: Some("the-chats-own".into()),
    }
}

/// What the window heard of each `secret exec`: the tool its host runs it as, whether it waited,
/// and whether its run was numbered by the host.
type ExecsHeard = Arc<Mutex<Vec<(Option<String>, bool, bool)>>>;

/// How the desktop's window answers a `secret exec` its chat's host runs: the values it sends,
/// or none.
type Deciding =
    Arc<dyn Fn(&brokered::Ask, Box<dyn BufRead + Send>) -> Option<brokered::Handed> + Send + Sync>;

/// The values of each key `ask` hands over, `v-<key>`, for its run.
fn its_values(ask: &brokered::Ask) -> brokered::Handed {
    let keys = ask
        .secret_exec
        .file
        .iter()
        .filter_map(|spec| spec.split_once('=').map(|(_, key)| key.to_owned()));
    brokered::Handed::of(
        ask.chat,
        ask.runs_as.as_deref().unwrap_or_default(),
        ask.run.as_deref().unwrap_or_default(),
        keys.map(|key| (key.clone(), format!("v-{key}"))),
    )
}

/// A desktop whose window decides each `secret exec` its chat's host runs with `deciding`,
/// only while `there`: its host's hearing, what the window heard, what ran on the host, the
/// host, and every number a line was handed on as.
struct Desktop {
    host: Hearing,
    heard: ExecsHeard,
    ran: Arc<Mutex<Vec<String>>>,
    /// What each run here decided a connection to [`LIVE_HOST`] by.
    decided: Arc<Mutex<Vec<crate::sandbox::reach::Decision>>>,
    relaying: Arc<Relaying>,
    numbers: Arc<Mutex<Vec<u64>>>,
}

/// A host the person may allow the composed chat live.
const LIVE_HOST: &str = "pypi.example.com";

fn a_desktop(there: &Arc<AtomicBool>, deciding: Deciding) -> Desktop {
    a_desktop_minting(there, deciding, Arc::new(|_, _| None))
}

/// [`a_desktop`], whose host asks `mints` of the cluster first.
fn a_desktop_minting(there: &Arc<AtomicBool>, deciding: Deciding, mints: InCluster) -> Desktop {
    a_desktop_saying(there, deciding, mints, None)
}

/// [`a_desktop_minting`], whose window writes `says` to each run it sends no values.
fn a_desktop_saying(
    there: &Arc<AtomicBool>,
    deciding: Deciding,
    mints: InCluster,
    says: Option<String>,
) -> Desktop {
    let (window, holding) = a_window();
    let heard: ExecsHeard = Arc::default();
    let mut hearing = the_windows(&window, &holding);
    hearing.secret_exec = {
        let heard = Arc::clone(&heard);
        Box::new(move |ask, reader, mut writer| {
            heard
                .lock()
                .unwrap()
                .push((ask.runs_as.clone(), ask.waited, ask.run.is_some()));
            if ask.runs_as.is_none() {
                let _ = writeln!(writer, "ran in the window");
                return;
            }
            if let Some(handed) = deciding(&ask, reader) {
                assert!(hand_values(handed).is_ok(), "sent to the host");
            } else if let Some(says) = &says {
                let _ = writeln!(writer, "{says}");
            }
        })
    };
    let heeding = Arc::new(Heeding::new(hearing));
    heeding.linked(LINK);
    let host: Arc<Mutex<Option<Arc<Relaying>>>> = Arc::default();
    let ran: Arc<Mutex<Vec<String>>> = Arc::default();
    let decided: Arc<Mutex<Vec<crate::sandbox::reach::Decision>>> = Arc::default();
    let numbers: Arc<Mutex<Vec<u64>>> = Arc::default();
    let relaying = Relaying::new({
        let (host, there, numbers) = (Arc::clone(&host), Arc::clone(there), Arc::clone(&numbers));
        Box::new(move |id, heard| {
            if !there.load(Ordering::SeqCst) {
                return false;
            }
            numbers.lock().unwrap().push(id);
            let heard: Heard =
                serde_json::from_str(&serde_json::to_string(heard).unwrap()).unwrap();
            let host = Arc::clone(&host);
            let reply: Reply = Arc::new(move |replied: Replied| {
                let replied: Replied =
                    serde_json::from_str(&serde_json::to_string(&replied).unwrap()).unwrap();
                let relaying = host.lock().unwrap().clone().unwrap();
                relaying.replied(id, replied);
            });
            let heeding = Arc::clone(&heeding);
            if Heeding::waits(&heard) {
                if heeding.admit(LINK, id, &heard, &reply) {
                    std::thread::spawn(move || heeding.hear(LINK, id, heard, reply));
                }
            } else {
                heeding.hear(LINK, id, heard, reply);
            }
            true
        })
    })
    .running_handed_with({
        let (ran, decided) = (Arc::clone(&ran), Arc::clone(&decided));
        Arc::new(move |asker, wanted, handed, reader, mut writer, _told| {
            if let Some(confines) = &asker.confines {
                decided.lock().unwrap().push(confines.decides().decide(
                    LIVE_HOST,
                    443,
                    &[443],
                    &[],
                ));
            }
            let keys: Vec<&str> = handed.keys().collect();
            ran.lock().unwrap().push(format!(
                "chat {} as {:?} ran {} with {keys:?}",
                asker.chat,
                asker.persona.as_deref().unwrap_or_default(),
                wanted.command.join(" ")
            ));
            for line in reader.lines() {
                writeln!(writer, "read {}", line.unwrap()).unwrap();
            }
        })
    })
    .minting_in_cluster_with(mints);
    *host.lock().unwrap() = Some(Arc::clone(&relaying));
    // The chat is there: its token is held.
    let tokens = Arc::new(ChatTokens::default());
    tokens.issue(COMPOSED).expect("issued");
    tokens.issue(MINTING).expect("issued");
    Desktop {
        host: relaying.hearing(tokens),
        heard,
        ran,
        decided,
        relaying,
        numbers,
    }
}

/// A window that sends each run its values at once.
fn sends_its_values() -> Deciding {
    Arc::new(|ask, _reader| Some(its_values(ask)))
}

/// Runs `ask` through `host`'s hearing on a thread: what the asker was written so far, the
/// asker's end of the connection, and when the exec ends.
fn an_exec_through(
    host: Hearing,
    ask: brokered::Ask,
) -> (
    Arc<Mutex<Vec<u8>>>,
    std::os::unix::net::UnixStream,
    mpsc::Receiver<()>,
) {
    an_exec_on(Arc::new(host), ask)
}

/// [`an_exec_through`], on a host that takes more than one.
fn an_exec_on(
    host: Arc<Hearing>,
    ask: brokered::Ask,
) -> (
    Arc<Mutex<Vec<u8>>>,
    std::os::unix::net::UnixStream,
    mpsc::Receiver<()>,
) {
    let written = Arc::new(Mutex::new(Vec::new()));
    let (theirs, ours) = std::os::unix::net::UnixStream::pair().unwrap();
    let (ended, done) = mpsc::channel();
    std::thread::spawn({
        let written = Arc::clone(&written);
        move || {
            (host.secret_exec)(
                ask,
                Box::new(std::io::BufReader::new(theirs)),
                Box::new(Shared(written)),
            );
            let _ = ended.send(());
        }
    });
    (written, ours, done)
}

fn said(written: &Arc<Mutex<Vec<u8>>>) -> String {
    String::from_utf8_lossy(&written.lock().unwrap()).into_owned()
}

/// Waits until `until` holds, at most seconds, saying `what` where it never does.
fn soon(what: &str, until: impl Fn() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !until() {
        assert!(Instant::now() < deadline, "{what}");
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// The one frame `written` holds.
fn the_frame(written: &Arc<Mutex<Vec<u8>>>) -> brokered::Frame {
    serde_json::from_str(said(written).trim()).unwrap()
}

#[test]
fn a_composed_chats_tool_runs_on_its_host_on_the_values_its_desktop_sent() {
    let project = a_composed_chat();
    let there = Arc::new(AtomicBool::new(true));
    let desktop = a_desktop(&there, sends_its_values());
    desktop.relaying.of_project_at(project.path());

    let (written, mut asker, done) = an_exec_through(desktop.host, kubectl_asked());
    soon("it never ran", || !desktop.ran.lock().unwrap().is_empty());
    writeln!(asker, "typed").unwrap();
    asker.shutdown(std::net::Shutdown::Write).unwrap();

    assert_eq!(done.recv_timeout(Duration::from_secs(10)), Ok(()));
    assert_eq!(
        *desktop.heard.lock().unwrap(),
        [(Some("kubectl".to_owned()), false, true)],
        "the window is told the tool its host runs it as and the run's number, and nothing the \
         chat claimed itself"
    );
    assert_eq!(
        *desktop.ran.lock().unwrap(),
        [r#"chat 21 as "devops" ran /opt/kube/kubectl get pods with ["kubeconfig"]"#],
        "run here, as the chat's host recorded it, on the values sent"
    );
    assert_eq!(
        said(&written),
        "read typed\n",
        "its input reached the run here"
    );
}

/// What the composed chat's confinement here allows live now, as a test sets it.
type LiveNow = Arc<Mutex<Vec<purlis_session_protocol::session::Listed>>>;

/// A reader of `now`, as the host's own hold would read the chat's confinement.
fn reading(now: &LiveNow) -> crate::hostedchats::Live {
    let now = Arc::clone(now);
    Arc::new(move || purlis_session_protocol::session::ReachNow {
        asked: Vec::new(),
        allowed_live: now.lock().unwrap().clone(),
    })
}

/// A window that, while the person is asked, runs `meanwhile`, then sends the run its values.
fn sends_its_values_after(meanwhile: impl Fn() + Send + Sync + 'static) -> Deciding {
    Arc::new(move |ask, _reader| {
        meanwhile();
        Some(its_values(ask))
    })
}

/// #3407: a run is held to the chat's confinement as it is when its values come, not as it was
/// when the command asked: a host the person took back while they were asked is not reached.
#[test]
fn a_host_taken_back_while_the_person_is_asked_is_not_reached_by_the_run() {
    let now: LiveNow = Arc::new(Mutex::new(vec![purlis_session_protocol::session::Listed {
        host: LIVE_HOST.into(),
        layer: purlis_session_protocol::session::Layer::Chat,
    }]));
    let project = a_composed_chat_reaching(Some(reading(&now)));
    let there = Arc::new(AtomicBool::new(true));
    let desktop = a_desktop(
        &there,
        sends_its_values_after({
            let now = Arc::clone(&now);
            move || now.lock().unwrap().clear()
        }),
    );
    desktop.relaying.of_project_at(project.path());

    let (_written, asker, done) = an_exec_through(desktop.host, kubectl_asked());
    soon("it never ran", || !desktop.ran.lock().unwrap().is_empty());
    asker.shutdown(std::net::Shutdown::Write).unwrap();

    assert_eq!(done.recv_timeout(Duration::from_secs(10)), Ok(()));
    assert_eq!(
        *desktop.decided.lock().unwrap(),
        [crate::sandbox::reach::Decision::Ask],
        "taken back before its values came"
    );
}

/// #3407: and a host the person allowed while they were asked is reached by that run.
#[test]
fn a_host_allowed_while_the_person_is_asked_is_reached_by_the_run() {
    let now: LiveNow = Arc::default();
    let project = a_composed_chat_reaching(Some(reading(&now)));
    let there = Arc::new(AtomicBool::new(true));
    let desktop = a_desktop(
        &there,
        sends_its_values_after({
            let now = Arc::clone(&now);
            move || {
                now.lock()
                    .unwrap()
                    .push(purlis_session_protocol::session::Listed {
                        host: LIVE_HOST.into(),
                        layer: purlis_session_protocol::session::Layer::Chat,
                    });
            }
        }),
    );
    desktop.relaying.of_project_at(project.path());

    let (_written, asker, done) = an_exec_through(desktop.host, kubectl_asked());
    soon("it never ran", || !desktop.ran.lock().unwrap().is_empty());
    asker.shutdown(std::net::Shutdown::Write).unwrap();

    assert_eq!(done.recv_timeout(Duration::from_secs(10)), Ok(()));
    assert_eq!(
        *desktop.decided.lock().unwrap(),
        [crate::sandbox::reach::Decision::Allowed(
            crate::sandbox::reach::By::Chat
        )]
    );
}

/// A chat its host forgot while the person was asked runs nothing on the values that come.
#[test]
fn a_chat_forgotten_while_the_person_is_asked_runs_nothing() {
    let project = a_composed_chat();
    let root = project.path().to_path_buf();
    let there = Arc::new(AtomicBool::new(true));
    let desktop = a_desktop(
        &there,
        sends_its_values_after(move || crate::hostedchats::forget(&root, COMPOSED)),
    );
    desktop.relaying.of_project_at(project.path());

    let (written, _asker, done) = an_exec_through(desktop.host, kubectl_asked());

    assert_eq!(done.recv_timeout(Duration::from_secs(10)), Ok(()));
    assert!(desktop.ran.lock().unwrap().is_empty(), "nothing ran");
    assert!(said(&written).contains("is gone"), "{}", said(&written));
}

#[test]
fn values_sent_for_another_run_are_refused_and_nothing_runs() {
    let project = a_composed_chat();
    let there = Arc::new(AtomicBool::new(true));
    let desktop = a_desktop(
        &there,
        Arc::new(|ask, _reader| {
            let mut handed = its_values(ask);
            handed.run = "another-run".into();
            Some(handed)
        }),
    );
    desktop.relaying.of_project_at(project.path());

    let (written, _asker, done) = an_exec_through(desktop.host, kubectl_asked());

    assert_eq!(done.recv_timeout(Duration::from_secs(10)), Ok(()));
    assert!(
        matches!(the_frame(&written), brokered::Frame::Refused { why, .. } if why.contains("not for this run")),
        "{}",
        said(&written)
    );
    assert!(desktop.ran.lock().unwrap().is_empty(), "nothing ran");
}

#[test]
fn values_a_window_sends_late_for_a_host_before_this_one_run_nothing_under_the_same_number() {
    // A window still deciding for a host that has since started again sends its values for the
    // old run; the new host numbers its lines afresh, and a run of its own that took the same
    // number is still not run on them.
    let project = a_composed_chat();
    let there = Arc::new(AtomicBool::new(true));
    let (holding, held) = mpsc::channel::<()>();
    let held = Arc::new(Mutex::new(held));
    let desktop = a_desktop(
        &there,
        Arc::new(move |_ask, _reader| {
            let _ = held.lock().unwrap().recv_timeout(Duration::from_secs(10));
            None
        }),
    );
    desktop.relaying.of_project_at(project.path());
    let first = desktop.relaying.next_number();
    let again = Relaying::new(Box::new(|_, _| false));
    assert_ne!(
        first,
        again.next_number(),
        "a host started again numbers its lines afresh"
    );

    let (written, _asker, done) = an_exec_through(desktop.host, kubectl_asked());
    soon("never handed on", || {
        !desktop.numbers.lock().unwrap().is_empty()
    });
    let id = desktop.numbers.lock().unwrap()[0];
    let late = brokered::Handed::of(
        COMPOSED,
        "kubectl",
        "the-run-of-the-host-before",
        [("kubeconfig".to_owned(), "v-old".to_owned())],
    );
    desktop.relaying.replied(id, Replied::Handed(late));

    assert_eq!(done.recv_timeout(Duration::from_secs(10)), Ok(()));
    let _ = holding.send(());
    assert!(
        matches!(the_frame(&written), brokered::Frame::Refused { why, .. } if why.contains("not for this run")),
        "{}",
        said(&written)
    );
    assert!(desktop.ran.lock().unwrap().is_empty(), "nothing ran");
}

#[test]
fn with_the_desktop_away_a_composed_chats_exec_waits_says_so_and_runs_once_it_is_back() {
    let project = a_composed_chat();
    let there = Arc::new(AtomicBool::new(false));
    let desktop = a_desktop(&there, sends_its_values());
    desktop.relaying.of_project_at(project.path());

    let (written, asker, done) = an_exec_through(desktop.host, kubectl_asked());
    soon("never told it waits", || {
        said(&written).contains("waits for them")
    });
    assert_eq!(
        the_frame(&written),
        brokered::Frame::Note(exec_waits_said("kubectl")),
        "said on its stderr, so the chat carries on"
    );
    assert!(
        desktop.ran.lock().unwrap().is_empty(),
        "nothing ran while away"
    );
    there.store(true, Ordering::SeqCst);
    soon("it never ran once the desktop was back", || {
        !desktop.ran.lock().unwrap().is_empty()
    });
    asker.shutdown(std::net::Shutdown::Write).unwrap();

    assert_eq!(done.recv_timeout(Duration::from_secs(10)), Ok(()));
    assert_eq!(
        *desktop.heard.lock().unwrap(),
        [(Some("kubectl".to_owned()), true, true)],
        "handed on as one that waited"
    );
}

#[test]
fn a_composed_chats_exec_whose_asker_hung_up_while_away_is_never_handed_on() {
    let project = a_composed_chat();
    let there = Arc::new(AtomicBool::new(false));
    let desktop = a_desktop(&there, sends_its_values());
    desktop.relaying.of_project_at(project.path());

    let (_written, asker, done) = an_exec_through(desktop.host, kubectl_asked());
    drop(asker);
    assert_eq!(done.recv_timeout(Duration::from_secs(10)), Ok(()));
    there.store(true, Ordering::SeqCst);

    assert!(desktop.heard.lock().unwrap().is_empty(), "never handed on");
    assert!(desktop.ran.lock().unwrap().is_empty(), "nothing ran");
}

#[test]
fn an_asker_that_hangs_up_while_the_window_decides_is_told_to_it_and_nothing_runs() {
    let project = a_composed_chat();
    let there = Arc::new(AtomicBool::new(true));
    let told_gone = Arc::new(AtomicBool::new(false));
    let desktop = a_desktop(&there, {
        let told_gone = Arc::clone(&told_gone);
        Arc::new(move |_ask, mut reader| {
            // A run its host runs carries no input to the window: its end is the asker's.
            let mut rest = String::new();
            let _ = reader.read_to_string(&mut rest);
            told_gone.store(true, Ordering::SeqCst);
            None
        })
    });
    desktop.relaying.of_project_at(project.path());

    let (_written, asker, done) = an_exec_through(desktop.host, kubectl_asked());
    soon("never handed on", || {
        !desktop.heard.lock().unwrap().is_empty()
    });
    drop(asker);

    soon("the window was never told", || {
        told_gone.load(Ordering::SeqCst)
    });
    assert_eq!(done.recv_timeout(Duration::from_secs(10)), Ok(()));
    assert!(desktop.ran.lock().unwrap().is_empty(), "nothing ran");
}

#[test]
fn values_the_link_could_not_carry_are_said_to_the_asker() {
    let project = a_composed_chat();
    let there = Arc::new(AtomicBool::new(true));
    let (holding, held) = mpsc::channel::<()>();
    let held = Arc::new(Mutex::new(held));
    let desktop = a_desktop(
        &there,
        Arc::new(move |_ask, _reader| {
            let _ = held.lock().unwrap().recv_timeout(Duration::from_secs(10));
            None
        }),
    );
    desktop.relaying.of_project_at(project.path());
    let (written, _asker, done) = an_exec_through(desktop.host, kubectl_asked());
    soon("never handed on", || {
        !desktop.numbers.lock().unwrap().is_empty()
    });
    let id = desktop.numbers.lock().unwrap()[0];

    desktop.relaying.replied(
        id,
        Replied::NotTaken {
            why: "too large for a frame".into(),
        },
    );

    assert_eq!(done.recv_timeout(Duration::from_secs(10)), Ok(()));
    let _ = holding.send(());
    assert!(
        matches!(the_frame(&written), brokered::Frame::Refused { why, .. } if why.contains("could not cross the link")),
        "{}",
        said(&written)
    );
}

#[test]
fn a_command_its_host_armed_no_tool_for_is_refused_there_and_never_handed_on() {
    let project = a_composed_chat();
    let there = Arc::new(AtomicBool::new(true));
    let desktop = a_desktop(&there, sends_its_values());
    desktop.relaying.of_project_at(project.path());
    let mut ask = kubectl_asked();
    ask.secret_exec.command = vec!["/bin/sh".into(), "-c".into(), "env".into()];

    let (written, _asker, done) = an_exec_through(desktop.host, ask);

    assert_eq!(done.recv_timeout(Duration::from_secs(10)), Ok(()));
    assert!(
        matches!(the_frame(&written), brokered::Frame::Refused { why, .. } if why.contains("declared tools")),
        "{}",
        said(&written)
    );
    assert!(
        desktop.heard.lock().unwrap().is_empty(),
        "the window was asked nothing"
    );
    assert!(desktop.ran.lock().unwrap().is_empty());
}

#[test]
fn a_chat_its_host_did_not_compose_runs_in_the_window_and_its_own_claims_are_cleared() {
    let project = tempfile::tempdir().unwrap();
    let there = Arc::new(AtomicBool::new(true));
    let desktop = a_desktop(&there, sends_its_values());
    desktop.relaying.of_project_at(project.path());

    let (written, asker, done) = an_exec_through(desktop.host, kubectl_asked());
    drop(asker);

    assert_eq!(done.recv_timeout(Duration::from_secs(10)), Ok(()));
    assert_eq!(*desktop.heard.lock().unwrap(), [(None, false, false)]);
    assert!(desktop.ran.lock().unwrap().is_empty());
    assert_eq!(said(&written), "ran in the window\n");
}

#[test]
fn values_are_sent_once_for_an_exec_and_never_from_another_thread() {
    let reply: Reply = Arc::new(|_| {});
    let sent = handing(reply, || {
        let one = hand_values(brokered::Handed::default());
        let again = hand_values(brokered::Handed::default());
        (one.is_ok(), again.is_ok())
    });
    assert_eq!(sent, (true, false), "once");
    assert!(
        hand_values(brokered::Handed::default()).is_err(),
        "nothing once the exec is heard"
    );
}

#[test]
fn a_waiting_exec_counts_within_the_bounds_logins_wait_in() {
    let relaying = Relaying::new(Box::new(|_, _| false));
    for _ in 0..MOST_WAITING_FOR_A_CHAT {
        assert!(relaying.admit(COMPOSED));
    }
    assert!(!relaying.admit(COMPOSED), "one chat's bound");
    relaying.release(COMPOSED);
    assert!(
        relaying.admit(COMPOSED),
        "room again once one waits no more"
    );
}

/// The chat its host composed, with the minting tool `gh` armed (#4207).
const MINTING: u32 = 22;

/// [`a_composed_chat`], whose chat [`MINTING`] has tool `gh`, a GitHub App's.
fn a_composed_minting_chat() -> tempfile::TempDir {
    use crate::sandbox::tools::{Handed as As, Ready, Tool};
    let tmp = tempfile::tempdir().unwrap();
    let mut tool = Tool::one("gh", "forge", "GH_TOKEN", As::Env, "app-key");
    tool.mint = Some(crate::sandbox::tools::mint::Mint {
        app: "Iv23liExample".into(),
    });
    let mut confines = crate::sandbox::Confines::default();
    confines.tools.push(Ready {
        tool,
        program: std::path::PathBuf::from("/opt/gh/gh"),
    });
    crate::hostedchats::record(
        tmp.path(),
        MINTING,
        crate::hostedchats::Composed {
            persona: Some("devops".into()),
            folder: tmp.path().to_path_buf(),
            harness: None,
            confines,
            live: None,
        },
    );
    tmp
}

fn gh_asked() -> brokered::Ask {
    brokered::Ask {
        chat: MINTING,
        secret_exec: brokered::Wanted {
            vault: "forge".into(),
            env: vec!["GH_TOKEN=app-key".into()],
            command: vec!["/opt/gh/gh".into(), "pr".into(), "list".into()],
            ..Default::default()
        },
        runs_as: None,
        waited: false,
        run: None,
    }
}

/// On an opted-in pool the run goes with the cluster's token: nobody is asked, the desktop may be
/// away, and the window hears nothing of it (the person's ruling on #3807, Q1).
#[test]
fn a_minting_tool_told_of_the_port_runs_on_its_token_with_the_desktop_away() {
    let project = a_composed_minting_chat();
    let there = Arc::new(AtomicBool::new(false));
    let desktop = a_desktop_minting(
        &there,
        sends_its_values(),
        Arc::new(|_, tool| {
            assert_eq!(tool, "gh");
            Some(Ok((
                "app-key".to_owned(),
                zeroize::Zeroizing::new("minted-here".to_owned()),
            )))
        }),
    );
    desktop.relaying.of_project_at(project.path());
    let (_written, asker, done) = an_exec_through(desktop.host, gh_asked());
    soon("it never ran", || !desktop.ran.lock().unwrap().is_empty());
    asker.shutdown(std::net::Shutdown::Write).unwrap();
    assert_eq!(done.recv_timeout(Duration::from_secs(10)), Ok(()));
    assert!(
        desktop.heard.lock().unwrap().is_empty(),
        "the window is asked nothing"
    );
    let ran = desktop.ran.lock().unwrap();
    assert!(ran[0].contains("[\"app-key\"]"), "{ran:?}");
}

/// **A refusal never becomes a token without the person** (#4207): the line goes to the window,
/// whose gate and Ask decide.
#[test]
fn a_mint_the_port_refuses_is_handed_to_the_window_with_the_refusal_said() {
    let project = a_composed_minting_chat();
    let there = Arc::new(AtomicBool::new(true));
    let desktop = a_desktop_minting(
        &there,
        Arc::new(|_, _| None),
        Arc::new(|_, _| Some(Err("the claim or its pool is stopped".to_owned()))),
    );
    desktop.relaying.of_project_at(project.path());
    let (written, asker, done) = an_exec_through(desktop.host, gh_asked());
    soon("the refusal was never said", || {
        said(&written).contains("the claim or its pool is stopped")
    });
    soon("the window never heard it", || {
        !desktop.heard.lock().unwrap().is_empty()
    });
    asker.shutdown(std::net::Shutdown::Write).unwrap();
    let _ = done.recv_timeout(Duration::from_secs(10));
    assert!(
        desktop.ran.lock().unwrap().is_empty(),
        "nothing ran on no token"
    );
    assert_eq!(
        desktop.heard.lock().unwrap()[0],
        (Some("gh".to_owned()), false, true),
        "handed on as any run on a pool not opted in is"
    );
}

/// A host told of no port, as on a pool not opted in, changes nothing.
#[test]
fn a_host_told_of_no_port_hands_a_minting_tool_to_the_window_saying_nothing_of_it() {
    let project = a_composed_minting_chat();
    let there = Arc::new(AtomicBool::new(true));
    let desktop = a_desktop_minting(&there, Arc::new(|_, _| None), Arc::new(|_, _| None));
    desktop.relaying.of_project_at(project.path());
    let (written, asker, done) = an_exec_through(desktop.host, gh_asked());
    soon("the window never heard it", || {
        !desktop.heard.lock().unwrap().is_empty()
    });
    assert!(!said(&written).contains("cluster"), "{}", said(&written));
    asker.shutdown(std::net::Shutdown::Write).unwrap();
    let _ = done.recv_timeout(Duration::from_secs(10));
    assert!(desktop.ran.lock().unwrap().is_empty());
}

/// A chat that keeps running its tool while the port refuses it asks the operator once for now
/// (#4332), and each run still reaches the window, whose gate and Ask decide.
#[test]
fn a_chats_refused_mints_are_paced_and_each_still_goes_to_the_window() {
    let project = a_composed_minting_chat();
    let there = Arc::new(AtomicBool::new(true));
    let asked = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let desktop = a_desktop_minting(
        &there,
        Arc::new(|_, _| None),
        Arc::new({
            let asked = Arc::clone(&asked);
            move |_, _| {
                asked.fetch_add(1, Ordering::SeqCst);
                Some(Err("the claim or its pool is stopped".to_owned()))
            }
        }),
    );
    desktop.relaying.of_project_at(project.path());
    let host = Arc::new(desktop.host);
    for run in 1..=3 {
        let (written, asker, done) = an_exec_on(Arc::clone(&host), gh_asked());
        soon("the window never heard it", || {
            desktop.heard.lock().unwrap().len() == run
        });
        assert!(
            said(&written).contains("the claim or its pool is stopped"),
            "a paced run says the operator's words too: {}",
            said(&written)
        );
        asker.shutdown(std::net::Shutdown::Write).unwrap();
        let _ = done.recv_timeout(Duration::from_secs(10));
    }
    assert_eq!(
        asked.load(Ordering::SeqCst),
        1,
        "the operator is asked once"
    );
}

/// Once the window has refused such a run too, the next is refused here, saying so, with the person
/// not asked again (#4332).
#[test]
fn a_run_the_window_refused_is_not_asked_of_the_person_again_at_once() {
    let project = a_composed_minting_chat();
    let there = Arc::new(AtomicBool::new(true));
    let desktop = a_desktop_minting(
        &there,
        Arc::new(|_, _| None),
        Arc::new(|_, _| Some(Err("the claim or its pool is stopped".to_owned()))),
    );
    desktop.relaying.of_project_at(project.path());
    let paced = (project.path().to_path_buf(), MINTING, "gh".to_owned());
    desktop
        .relaying
        .asks_paced
        .refused(&paced, "refused by the person");
    let (written, asker, done) = an_exec_through(desktop.host, gh_asked());
    soon("the asker was never told", || {
        said(&written).contains("does not ask again")
    });
    asker.shutdown(std::net::Shutdown::Write).unwrap();
    let _ = done.recv_timeout(Duration::from_secs(10));
    assert!(desktop.heard.lock().unwrap().is_empty(), "nobody is asked");
    assert!(desktop.ran.lock().unwrap().is_empty());
}

/// Runs started together cost the operator one question, and the person one Ask: those that come
/// while one is under way wait for what comes of it (#4374).
#[test]
fn runs_started_together_ask_once_and_the_rest_go_by_what_came_of_it() {
    let project = a_composed_minting_chat();
    let there = Arc::new(AtomicBool::new(true));
    let asked = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let refused = serde_json::to_string(&brokered::Frame::Refused {
        why: "refused by the person".to_owned(),
        code: 1,
    })
    .unwrap();
    let desktop = a_desktop_saying(
        &there,
        Arc::new(|_, _| None),
        Arc::new({
            let asked = Arc::clone(&asked);
            move |_, _| {
                asked.fetch_add(1, Ordering::SeqCst);
                // Long enough for every run below to have come while it is under way.
                std::thread::sleep(Duration::from_millis(300));
                Some(Err("the claim or its pool is stopped".to_owned()))
            }
        }),
        Some(refused),
    );
    desktop.relaying.of_project_at(project.path());
    let host = Arc::new(desktop.host);
    let runs: Vec<_> = (0..4)
        .map(|_| an_exec_on(Arc::clone(&host), gh_asked()))
        .collect();
    soon("the others were never told", || {
        runs.iter()
            .filter(|(written, ..)| said(written).contains("does not ask again"))
            .count()
            == 3
    });
    assert_eq!(
        asked.load(Ordering::SeqCst),
        1,
        "the operator is asked once"
    );
    assert_eq!(
        desktop.heard.lock().unwrap().len(),
        1,
        "the person is asked once"
    );
    for (_, asker, _) in &runs {
        let _ = asker.shutdown(std::net::Shutdown::Write);
    }
}

/// An asker that hangs up while it waits stops waiting, and takes no turn (#4374).
#[test]
fn a_run_waiting_for_its_turn_that_hangs_up_asks_nothing() {
    let project = a_composed_minting_chat();
    let there = Arc::new(AtomicBool::new(true));
    let desktop = a_desktop_minting(
        &there,
        Arc::new(|_, _| None),
        Arc::new(|_, _| Some(Err("the claim or its pool is stopped".to_owned()))),
    );
    desktop.relaying.of_project_at(project.path());
    let paced = (project.path().to_path_buf(), MINTING, "gh".to_owned());
    let held = desktop
        .relaying
        .ask_turns
        .taken_within(&paced, Duration::ZERO)
        .expect("free");
    let (_, asker, done) = an_exec_through(desktop.host, gh_asked());
    asker.shutdown(std::net::Shutdown::Write).unwrap();
    done.recv_timeout(Duration::from_secs(10))
        .expect("it stopped waiting");
    drop(held);
    assert!(desktop.heard.lock().unwrap().is_empty(), "nobody is asked");
}

/// A chat that went leaves no pace for what starts again under its number (#4374).
#[test]
fn a_chat_that_went_leaves_no_pace_for_its_number() {
    let project = a_composed_minting_chat();
    let there = Arc::new(AtomicBool::new(true));
    let desktop = a_desktop_minting(
        &there,
        Arc::new(|_, _| None),
        Arc::new(|_, _| Some(Err("the claim or its pool is stopped".to_owned()))),
    );
    let paced = (project.path().to_path_buf(), MINTING, "gh".to_owned());
    desktop.relaying.port_paced.refused(&paced, "no");
    desktop.relaying.asks_paced.refused(&paced, "no");
    desktop.relaying.chat_went(MINTING);
    assert_eq!(desktop.relaying.port_paced.waits(&paced), None);
    assert_eq!(desktop.relaying.asks_paced.waits(&paced), None);
}

/// The window says the person is being asked before they answer: only its refusal paces.
#[test]
fn the_windows_note_that_it_asks_is_no_refusal() {
    use super::is_a_refusal;
    let line = |frame: &brokered::Frame| serde_json::to_string(frame).unwrap();
    let note = line(&brokered::Frame::Note("asked".to_owned()));
    let refusal = line(&brokered::Frame::Refused {
        why: "no".to_owned(),
        code: 1,
    });
    assert!(!is_a_refusal(&note));
    assert!(is_a_refusal(&refusal));
    assert!(is_a_refusal(&format!("{note}\n{refusal}\n")));
    assert!(!is_a_refusal("not a frame"));
}
