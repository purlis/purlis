// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **`purlis skill grade --run`: a grade asked for at a terminal** (SK-61, #3241; ruled
//! 2026-10-31). A grade's runner lives in the window, so this asks the session host, as
//! `terminal`, and the host relays it to the window that holds the project. The window answers
//! with the plan a press of Grade would start, and takes the start through that press's one
//! check, so a terminal's yes consents to exactly what a press does.
//!
//! **Three things hold what the yes means.** What is printed is this process's own read of the
//! skill, and the window's digest of its read must match it before anything is asked. The start
//! names the digest of the plan the window answered, profile, model and limits included. And a
//! link that goes (Ctrl-C, a closed terminal) stops the grade: nothing spends with nobody
//! following it.
//!
//! **Refused inside a chat**, before anything else. That is the courtesy; the enforcement is the
//! host's, which refuses a peer inside a chat every scope, and the sandbox's, which denies a chat
//! the `terminal` credential (ADR 0068 §5).

use std::path::Path;

use purlis_core::hookwire::SOCKET_ENV;
use purlis_core::skills::grade::Arm;
use purlis_core::skills::grades;
use purlis_core::skills::verbs::Level;
use purlis_session_protocol::session::{
    GradeAnswer, GradeAsk, GradeLevel, GradeOf, GradePlanTold, GradeStepTold,
};

use crate::memory::Code;
use crate::voice;

/// What the person asked for.
pub(crate) struct Asked {
    pub wire: GradeLevel,
    pub level: Level,
    pub name: String,
    pub profile: Option<String>,
    pub model: Option<String>,
    pub evals: Vec<String>,
    pub yes: bool,
}

/// What a command in a chat is told.
const IN_A_CHAT: &str = "`purlis skill grade --run` spends the person's account, so a chat cannot \
     run it, and nothing was run. Ask the person to run it in a terminal, or to press Grade in \
     the Skill studio";

/// What a terminal is told with no session host to ask.
const NO_HOST: &str = "No purlis session host is running, so nothing was run. Open purlis, or \
     start a host with `purlis serve`, then open this project in the purlis window: a grade runs \
     there";

/// Whether this runs inside a chat, as every write of `purlis skill` asks: its number, its hook
/// socket, or its sandbox.
fn in_a_chat(env: &dyn Fn(&str) -> Option<String>) -> bool {
    let set = |name: &str| env(name).is_some_and(|value| !value.is_empty());
    set(purlis_core::hookwire::CHAT_ENV)
        || set(SOCKET_ENV)
        || set(purlis_core::active::SESSION_ID_ENV)
        || purlis_core::sandbox::chat_is_sandboxed_in(env)
}

/// The grade `asked` of the project at `root`: the exit code.
pub(crate) fn run(root: &Path, asked: &Asked) -> Code {
    if in_a_chat(&purlis_core::envvar::var) {
        voice::err(IN_A_CHAT);
        return 1;
    }
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(why) => {
            voice::err(format!("the grade could not start: {why}"));
            return 1;
        }
    };
    runtime.block_on(asking(root, asked))
}

#[cfg(not(unix))]
async fn asking(_root: &Path, _asked: &Asked) -> Code {
    voice::err("the session host runs on macOS and Linux only, so nothing was run");
    1
}

/// A link to this user's session host, as `terminal`, or what to say: that none answers, or why
/// the one that answers refused this terminal.
#[cfg(unix)]
async fn linked() -> Result<purlis_session_protocol::link::Link, String> {
    use purlis_session_protocol::auth::{Credential, Scope};
    use purlis_session_protocol::{link, session};
    let no_host = || NO_HOST.to_owned();
    let config = purlis_core::machine::config_root().ok_or_else(no_host)?;
    let credential = Credential::read(&purlis_core::machine::charterd(&config), Scope::Terminal)
        .map_err(|_| no_host())?;
    let stream = tokio::net::UnixStream::connect(purlis_core::hostsocket::here())
        .await
        .map_err(|_| no_host())?;
    link::connect(stream, session::speaks(), Scope::Terminal, &credential)
        .await
        .map_err(|why| format!("The session host refused this terminal ({why}). Nothing was run."))
}

#[cfg(unix)]
async fn asking(root: &Path, asked: &Asked) -> Code {
    use purlis_session_protocol::session::{self, Client};
    let link = match linked().await {
        Ok(link) => link,
        Err(why) => {
            voice::err(why);
            return 1;
        }
    };
    let (client, _views, mut events) = Client::new(link);
    let refused = |why: &str| {
        voice::err(why.to_owned());
        1
    };
    let Ok(project) = purlis_core::scaffold::planefile::id_to_hold_by(root) else {
        return refused("This project has no id the session host can name it by.");
    };
    let path = root.display().to_string();
    let of = GradeOf {
        level: asked.wire.clone(),
        skill: asked.name.clone(),
        profile: asked.profile.clone(),
        model: asked.model.clone(),
        evals: asked.evals.clone(),
    };
    let answer = client
        .ask_grade(&project, &path, GradeAsk::Plan { of: of.clone() })
        .await;
    let plan = match answer {
        Ok(GradeAnswer::Plan(plan)) => plan,
        Ok(GradeAnswer::Refused { why }) => return refused(&format!("{why} Nothing was run.")),
        Ok(GradeAnswer::Started { .. }) => return refused("The window answered out of turn."),
        Err(why) => return refused(&refused_words(&why)),
    };
    let planned = match grades::planned(root, &asked.level, &asked.name, None) {
        Ok(planned) => planned,
        Err(why) => return refused(&why),
    };
    print!("{}", crate::skill::plan_said(&planned, root));
    // What was printed is what the window read, or the yes would be for something else.
    if plan.graded != planned.consent {
        return refused(
            "The skill changed while its plan was read, so nothing was run: run this again.",
        );
    }
    println!("{}", terms_said(&plan));
    if let Err(why) = can_start(&plan) {
        return refused(&why);
    }
    let Some(consent) = plan.consent.clone() else {
        return refused("There is nothing to grade, so nothing was run.");
    };
    if !asked.yes && !said_yes().await {
        voice::info("Nothing was run.");
        return 1;
    }
    // Only now: nothing reads what it is pushed while the question waits. Before the start, so no
    // step of it is missed.
    if let Err(why) = client.subscribe(session::Since::new()).await {
        return refused(&format!("The session host refused this terminal: {why}."));
    }
    let started = client
        .ask_grade(&project, &path, GradeAsk::Start { of, consent })
        .await;
    let grade = match started {
        Ok(GradeAnswer::Started { grade }) => grade,
        Ok(GradeAnswer::Refused { why }) => return refused(&why),
        Ok(GradeAnswer::Plan(_)) => return refused("The window answered out of turn."),
        Err(why) => return refused(&refused_words(&why)),
    };
    voice::info("Grading. Ctrl-C stops it; the Skill studio shows it too.");
    let last = loop {
        match events.recv().await {
            Some(session::Told::Pushed(session::Pushed::GradeStep { step }))
                if step.grade == grade =>
            {
                if step.ended.is_some() {
                    break Some(step);
                }
                println!("{}", step_said(&step));
            }
            // Steps may have gone unread. Leaving ends the link, which stops the grade.
            Some(session::Told::FellBehind { .. }) => {
                return refused(
                    "This terminal fell behind the session host and lost track of the grade, so \
                     the grade was stopped.",
                );
            }
            Some(_) => {}
            None => break None,
        }
    };
    let Some(last) = last else {
        return refused("The session host closed, so the grade ended with it.");
    };
    if let Some(said) = &last.said {
        voice::info(said.clone());
    }
    let record = grades::grades_of(root, &asked.name, Some(&asked.level));
    if let Some(graded) = record.iter().find(|one| one.grade == grade) {
        print!("{}", scorecard_said(graded));
    }
    Code::from(last.ended != Some(session::GradeEndTold::Whole))
}

/// A refusal in the host's words, or that the host is older than this command.
#[cfg(unix)]
fn refused_words(why: &purlis_session_protocol::session::CallError) -> String {
    match why.refusal() {
        Some(refusal) if refusal.code == purlis_session_protocol::session::UNKNOWN_COMMAND => {
            "The running purlis is older than this command and cannot run a grade from a \
             terminal: update it, or press Grade in the Skill studio. Nothing was run."
                .to_owned()
        }
        Some(refusal) => format!("{}.", refusal.why),
        None => format!("The session host did not answer ({why}), so nothing was run."),
    }
}

/// Why this plan cannot start: the window's own words, said before a yes is asked for.
fn can_start(plan: &GradePlanTold) -> Result<(), String> {
    if let Some(why) = &plan.refused {
        return Err(format!("{why} Nothing was run."));
    }
    if plan.running.is_some() {
        return Err(
            "A grade is running in this project already: stop it, or wait for it to end. \
             Nothing was run."
                .to_owned(),
        );
    }
    Ok(())
}

/// What the plan's terms are, as the window would start it: what the yes covers.
fn terms_said(plan: &GradePlanTold) -> String {
    let word = |word: &Option<String>, none: &str| word.clone().unwrap_or_else(|| none.to_owned());
    format!(
        "It runs {} chats on profile {}, model {}, under the {} sandbox, and stops once they \
         have spent {} tokens or ${:.2}.",
        plan.chats,
        word(&plan.profile, "none"),
        word(&plan.model, "the profile's own"),
        word(&plan.sandbox, "unknown"),
        plan.token_cap,
        dollars(plan.limit_micro_usd),
    )
}

/// Millionths of a dollar, in dollars.
fn dollars(micro: u64) -> f64 {
    micro as f64 / 1_000_000.0
}

/// One step, in a line.
fn step_said(step: &GradeStepTold) -> String {
    let spent = match (step.cost_micro_usd, step.tokens) {
        (Some(cost), _) => format!(", ${:.2} so far", dollars(cost)),
        (None, Some(tokens)) => format!(", {tokens} tokens so far"),
        (None, None) => String::new(),
    };
    format!("  {} of {} chats{spent}", step.ran, step.of)
}

/// Whether the person answers yes: read off the runtime. Anything else, the end of its input
/// included, is a no.
async fn said_yes() -> bool {
    use std::io::Write as _;
    print!("Run this grade? It spends your account. [y/N] ");
    let _ = std::io::stdout().flush();
    tokio::task::spawn_blocking(|| {
        let mut line = String::new();
        std::io::stdin().read_line(&mut line).ok()?;
        Some(is_yes(&line))
    })
    .await
    .ok()
    .flatten()
    .unwrap_or(false)
}

fn is_yes(line: &str) -> bool {
    matches!(line.trim().to_ascii_lowercase().as_str(), "y" | "yes")
}

/// **The grade's scorecard**: each test in each arm, with the chats that asked something.
fn scorecard_said(graded: &grades::Graded) -> String {
    let mut text = String::from("\nScorecard:\n");
    for one in &graded.report.triggers {
        let arm = match one.arm {
            Arm::With => "with",
            Arm::Without => "without",
            Arm::Previous => "previous",
            Arm::Router => "router",
            Arm::BuiltInRouter => "built-in router",
        };
        let verdict = match one.pass {
            Some(true) => "pass",
            Some(false) => "fail",
            None => "not run",
        };
        let asked = match one.asked {
            0 => String::new(),
            asked => format!(", {asked} asked you something"),
        };
        text.push_str(&format!(
            "  {verdict:<7} {arm:<8} {}/{} started it{asked}  {}\n",
            one.triggered,
            one.runs,
            purlis_core::skills::review::escaped_line(&one.query),
        ));
    }
    text
}

#[cfg(test)]
mod tests;
