// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A skill change at the person's terminal on an org's project (SK-11a, #6030), over the session
//! protocol on in-memory links: `terminal` asks the host to audit it, and the host hands the
//! binary's [`Record`] the change it was told and the project's folder as this machine names it,
//! answering only once the binary has made it durable. No other scope's ask reaches the binary,
//! and a host that keeps no event log refuses in words.

#![cfg(unix)]

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use purlis_host::standin::StandIns;
use purlis_host::{Compose, ConfinesNothing, Ended, HearsNothing, Hosting, Record, ToCompose};
use purlis_session_protocol::auth::{Credentials, Scope};
use purlis_session_protocol::link;
use purlis_session_protocol::session::{self, Answer, Client, Command, SkillAct, SkillChangeAudit};
use purlis_terminal::session::Spec;
use tokio::io::duplex;

static HELD: std::sync::LazyLock<Credentials> =
    std::sync::LazyLock::new(|| Credentials::mint().unwrap());

struct Echo;

impl Compose for Echo {
    fn compose(&self, starting: &ToCompose<'_>) -> Result<Spec, String> {
        Ok(Spec::new("/bin/echo", starting.size))
    }
}

/// The binary's keeping: each terminal's skill change it was asked to audit, or the refusal it
/// gives.
#[derive(Clone, Default)]
struct Kept {
    audited: Arc<Mutex<Vec<(PathBuf, SkillChangeAudit)>>>,
    refuses: Option<&'static str>,
}

impl Record for Kept {
    fn record(&self, _entry: serde_json::Value) -> Result<(), String> {
        Ok(())
    }

    fn ended(&self, _ended: &Ended<'_>) {}

    fn audit_skill_change(
        &self,
        project_path: &std::path::Path,
        change: &SkillChangeAudit,
    ) -> Result<(), String> {
        if let Some(why) = self.refuses {
            return Err(why.to_owned());
        }
        self.audited
            .lock()
            .unwrap()
            .push((project_path.to_owned(), change.clone()));
        Ok(())
    }
}

fn a_host(kept: &Kept) -> Hosting {
    Hosting::with(Echo, StandIns::default(), HearsNothing, ConfinesNothing)
        .recording_with(kept.clone())
}

async fn a_link(hosting: &Hosting, scope: Scope) -> Client {
    let (a, b) = duplex(256 * 1024);
    let (client, served) = tokio::join!(
        link::connect(a, session::speaks(), scope, HELD.of(scope)),
        link::serve_any(b, session::speaks(), &HELD)
    );
    tokio::spawn(session::serve(served.unwrap(), hosting.clone(), None));
    let (client, _views, _events) = Client::new(client.unwrap());
    client
}

fn an_install() -> SkillChangeAudit {
    SkillChangeAudit {
        act: SkillAct::Install,
        level: "project".into(),
        skills: vec!["notify".into()],
        source: "https://github.com/acme/skills".into(),
        pin: "0123456789abcdef0123456789abcdef01234567".into(),
    }
}

fn asked(project: &std::path::Path) -> Command {
    Command::AuditSkillChange {
        project_path: project.display().to_string(),
        change: an_install(),
    }
}

#[tokio::test]
async fn a_terminals_skill_change_reaches_the_binarys_audit_with_its_project_as_this_machine_names_it()
 {
    let project = tempfile::tempdir().unwrap();
    let kept = Kept::default();
    let hosting = a_host(&kept);
    let terminal = a_link(&hosting, Scope::Terminal).await;

    // Named through a `..`, as a terminal might: the binary is told the folder itself.
    let roundabout = project
        .path()
        .join("..")
        .join(project.path().file_name().unwrap());
    let answer = terminal.call(asked(&roundabout)).await.unwrap();

    assert_eq!(answer, Answer::Done);
    assert_eq!(
        *kept.audited.lock().unwrap(),
        vec![(project.path().canonicalize().unwrap(), an_install())]
    );
}

#[tokio::test]
async fn no_link_but_the_terminals_asks_for_a_skill_change_to_be_audited() {
    let project = tempfile::tempdir().unwrap();
    let kept = Kept::default();
    let hosting = a_host(&kept);

    // Every other scope's row is the protocol's own test; these are the ones a host admits.
    for scope in [
        Scope::LocalUi,
        Scope::FleetMcp,
        Scope::Approval,
        Scope::Editor,
    ] {
        let other = a_link(&hosting, scope).await;

        let refused = other.call(asked(project.path())).await.unwrap_err();

        let refusal = refused.refusal().expect("refused by the host");
        assert_eq!(refusal.code, session::NOT_ALLOWED, "{scope}");
    }
    assert!(kept.audited.lock().unwrap().is_empty(), "nothing audited");
}

#[tokio::test]
async fn a_change_the_binary_could_not_make_durable_is_refused_in_its_words() {
    let project = tempfile::tempdir().unwrap();
    let kept = Kept {
        refuses: Some("this project holds no [skill_policy], so nothing is audited"),
        ..Kept::default()
    };
    let hosting = a_host(&kept);
    let terminal = a_link(&hosting, Scope::Terminal).await;

    let refused = terminal.call(asked(project.path())).await.unwrap_err();

    let refusal = refused.refusal().expect("refused by the host");
    assert_eq!(refusal.code, session::NOT_RECORDED);
    assert!(refusal.why.contains("[skill_policy]"), "{}", refusal.why);
}

#[tokio::test]
async fn a_folder_that_is_not_there_is_refused_before_the_binary_hears_of_it() {
    let kept = Kept::default();
    let hosting = a_host(&kept);
    let terminal = a_link(&hosting, Scope::Terminal).await;

    let refused = terminal
        .call(asked(std::path::Path::new("/no/such/project/here")))
        .await
        .unwrap_err();

    let refusal = refused.refusal().expect("refused by the host");
    assert_eq!(refusal.code, session::NOT_RECORDED);
    assert!(kept.audited.lock().unwrap().is_empty(), "nothing audited");
}

#[tokio::test]
async fn a_host_that_keeps_no_event_log_refuses_a_terminals_skill_change_in_words() {
    let project = tempfile::tempdir().unwrap();
    let hosting = Hosting::with(Echo, StandIns::default(), HearsNothing, ConfinesNothing);
    let terminal = a_link(&hosting, Scope::Terminal).await;

    let refused = terminal.call(asked(project.path())).await.unwrap_err();

    let refusal = refused.refusal().expect("refused by the host");
    assert_eq!(refusal.code, session::NOT_RECORDED);
    assert!(refusal.why.contains("event log"), "{}", refusal.why);
}
