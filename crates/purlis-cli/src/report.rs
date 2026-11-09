// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! `charter report bug|feature`: the command line over [`purlis_core::report`].
//!
//! Every run drafts, scrubs and prints the draft. Only the reporter's yes files it: `y` at a
//! prompt on a terminal, or `--yes <digest>` naming the digest the preview printed, so a chat
//! can run this safely — what it would send is on screen before anything is sent.
//!
//! **Filed with purlis's own sign-in, never `gh`** (#806). Everything after reading the words
//! is the Reports module's ([`Reports`], #5686): it drafts, scrubs, routes and files, and this
//! only prints its outcome. Only the person in the app's window files as the reporter: a
//! command, which a chat may be running, holds no sign-in (ADR 0070 ruling 5). So here the draft
//! is shown and its digest checked, and the reporter files it through the prefilled link, or from
//! the app.
//!
//! **In a chat the app started, the words go to the person's Inbox** (#5694, spec #5683): the
//! command hands them to the app over the chat's hook socket, and the app holds the draft for the
//! person only with their yes to Agent drafts. The person reads it, edits it and files it there.
//! Nothing is filed from a chat.

use crate::clierror::CliError;
use std::io::{IsTerminal, Read, Write};
use std::path::PathBuf;
use std::process::ExitCode;

use purlis_core::forge::Caller;
use purlis_core::report::{self, Draft, Evidence, Kind, Outcome, Refusal, Reports, Route};

#[derive(clap::Subcommand)]
pub enum ReportCommand {
    /// Draft a bug report on purlis's own tracker. Shows the draft; files nothing without
    /// your yes.
    Bug {
        #[command(flatten)]
        text: TextArgs,
        /// Draft the newest panic the app saved: where it panicked, its message and its
        /// purlis version. Your own words, if given, go in front of it.
        #[arg(long)]
        panic: bool,
    },
    /// Draft a feature request on purlis's own tracker. Shows the draft; files nothing
    /// without your yes.
    #[command(alias = "gap")]
    Feature {
        #[command(flatten)]
        text: TextArgs,
    },
}

#[derive(clap::Args)]
pub struct TextArgs {
    /// What happened, or what purlis should do. The first line is the title.
    text: Option<String>,
    /// Read the description from this file, so backticks and `$` reach it as written.
    #[arg(long, value_name = "PATH", conflicts_with_all = ["text", "stdin"])]
    from_file: Option<PathBuf>,
    /// Read the description from standard input.
    #[arg(long, conflicts_with = "text")]
    stdin: bool,
    /// The issue's title, instead of the description's first line.
    #[arg(long)]
    title: Option<String>,
    /// File the draft, if its digest is this one: the digest the preview of the same draft
    /// printed. Anything else files nothing.
    #[arg(long, value_name = "DIGEST")]
    yes: Option<String>,
    /// The purlis command or tool that failed, for the report's first line.
    #[arg(long, value_name = "COMMAND")]
    call: Option<String>,
    /// The exit code the failed call ended with.
    #[arg(
        long,
        value_name = "CODE",
        requires = "call",
        allow_hyphen_values = true
    )]
    exit: Option<i32>,
    /// In a chat: add the words to the draft purlis made of a failure of its own in this chat,
    /// the one its note named, rather than draft a report of their own (#5697).
    #[arg(long, value_name = "DRAFT", conflicts_with_all = ["call", "title", "yes"])]
    adds: Option<String>,
}

pub fn run(command: &ReportCommand) -> ExitCode {
    // No working directory means no plane, and a report needs none.
    let plane = std::env::current_dir()
        .ok()
        .and_then(|cwd| purlis_core::plane::resolve(&cwd).ok());
    let reports = Reports::here(plane.as_deref());
    let source = Caller::command();
    let refused = |said: &str| {
        eprintln!("charter report: {said}");
        ExitCode::FAILURE
    };
    let (evidence, args) = match evidence(command) {
        Ok(made) => made,
        Err(said) => return refused(&said),
    };
    if let Evidence::PersonDraft { kind, text, title } = &evidence
        && let Some(code) = to_the_inbox(*kind, text, title.as_deref(), args)
    {
        return code;
    }
    match reports.take(evidence.clone(), &source) {
        Outcome::Drafted { draft, route } => {
            decide(&reports, &source, evidence, &draft, route, args)
        }
        Outcome::Refused(Refusal::Unusable(said)) => refused(&said),
        Outcome::Refused(refusal) => refused(&format!("the draft was not made: {refusal:?}")),
        // A person's words are drafted or refused: anything else is a state this command has no
        // arm for (#5862).
        other => CliError::internal(format!(
            "charter report: a draft was asked for and the Reports module answered {other:?}"
        ))
        .exit(),
    }
}

/// What the command line hands the Reports module: the reporter's words, or a saved panic.
fn evidence(command: &ReportCommand) -> Result<(Evidence, &TextArgs), String> {
    let (kind, args, panic) = match command {
        ReportCommand::Bug { text, panic } => (Kind::Bug, text, *panic),
        ReportCommand::Feature { text } => (Kind::Feature, text, false),
    };
    let words = words(args)?;
    let saved = report::panic_log()
        .and_then(|log| std::fs::read_to_string(log).ok())
        .and_then(|log| report::latest_panic(&log));
    let evidence = if panic {
        let Some(saved) = saved else {
            return Err(format!(
                "there is no saved panic to report (purlis looked in {})",
                report::panic_log().map_or("no log directory".into(), |p| p.display().to_string())
            ));
        };
        Evidence::Panic {
            record: saved,
            words,
        }
    } else {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        if kind == Kind::Bug
            && let Some(saved) = saved.as_ref().filter(|p| p.is_recent(now))
        {
            eprintln!(
                "charter report: the app saved a panic (at {}). `charter report bug --panic` \
                 drafts it as a bug.",
                saved.place.rsplit(['/', '\\']).next().unwrap_or("")
            );
        }
        Evidence::PersonDraft {
            kind,
            text: words.unwrap_or_default(),
            title: args.title.clone(),
        }
    };
    Ok((evidence, args))
}

/// How long the app may take to hold a draft: a scrub and one small write.
const HOLDING_TAKES_AT_MOST: std::time::Duration = std::time::Duration::from_secs(10);

/// In a chat the app started, hands the words to the app for the person's Inbox and says what
/// became of them; `None` where this is no such chat, so the command drafts here as before.
fn to_the_inbox(kind: Kind, text: &str, title: Option<&str>, args: &TextArgs) -> Option<ExitCode> {
    use crate::brokered::NotTaken;
    use purlis_core::hookwire::{Answer, Ask, ReportDraftAsk};
    let ask = |chat| {
        Ask::ReportDraft(Box::new(ReportDraftAsk {
            chat,
            feature: kind == Kind::Feature,
            text: text.to_owned(),
            title: title.map(str::to_owned),
            call: args
                .call
                .as_ref()
                .map(|call| (call.clone(), args.exit.unwrap_or(1))),
            caught: None,
            adds: args.adds.clone(),
        }))
    };
    let said = match crate::brokered::asked(ask, HOLDING_TAKES_AT_MOST) {
        Err(NotTaken::NotAChat) if args.adds.is_some() => Err(ADDS_ONLY_IN_A_CHAT.to_owned()),
        Err(NotTaken::NotAChat) => return None,
        Err(_) => Err(NOT_HELD.to_owned()),
        Ok(Ok(Answer::Said { lines, code })) => Ok((lines, code)),
        Ok(Ok(Answer::No { why })) if why == purlis_core::hookwire::UNKNOWN_LINE => {
            Err(NOT_HELD.to_owned())
        }
        Ok(Ok(Answer::No { why })) => Err(why),
        Ok(Ok(_)) => Err(NOT_HELD.to_owned()),
        Ok(Err(e)) => Err(format!(
            "the app took the draft and did not answer ({e}), so it may be in the person's \
             Inbox already. Nothing was sent."
        )),
    };
    Some(match said {
        Ok((lines, code)) => {
            for line in lines {
                crate::speak(line);
            }
            if args.yes.is_some() {
                eprintln!(
                    "charter report: a chat files no report, so --yes did nothing: the person \
                     files it from their Inbox."
                );
            }
            ExitCode::from(code)
        }
        Err(why) => {
            eprintln!("charter report: {why}");
            ExitCode::FAILURE
        }
    })
}

/// Why `--adds` did nothing outside a chat the app started.
const ADDS_ONLY_IN_A_CHAT: &str = "--adds adds to a draft purlis made in a chat the app started, \
                                   and this is no such chat. Nothing was changed.";

/// Why a chat's draft did not reach the Inbox where no app could take it.
const NOT_HELD: &str = "the purlis app that started this chat did not take the draft: it is not \
                        running, or older than this command. Nothing was sent.";

/// The reporter's own words, from wherever they were given.
fn words(args: &TextArgs) -> Result<Option<String>, String> {
    if let Some(path) = &args.from_file {
        return std::fs::read_to_string(path)
            .map(Some)
            .map_err(|e| format!("cannot read {}: {e}", path.display()));
    }
    if args.stdin {
        let mut text = String::new();
        std::io::stdin()
            .read_to_string(&mut text)
            .map_err(|e| format!("cannot read standard input: {e}"))?;
        return Ok(Some(text));
    }
    Ok(args.text.clone())
}

fn decide(
    reports: &Reports,
    source: &Caller,
    evidence: Evidence,
    draft: &Draft,
    route: Route,
    args: &TextArgs,
) -> ExitCode {
    let digest = draft.digest();
    print!("{}", draft.preview());
    if let Some(given) = &args.yes {
        return send(reports, source, evidence, given);
    }
    if let Route::ByHand { why, link, whole } = route {
        println!("Nothing was sent: {why}.");
        print_link(&link, whole);
        return ExitCode::SUCCESS;
    }
    let query = draft.query();
    if !query.is_empty() {
        println!(
            "Searching {} for possible duplicates of: {query}",
            report::UPSTREAM
        );
    }
    match reports.duplicates(draft, source) {
        Ok(hits) if hits.is_empty() => println!("Possible duplicates: none found."),
        Ok(hits) => {
            println!("Possible duplicates — if one is the same, add to it on GitHub instead:");
            for hit in hits {
                println!(
                    "  #{} {} ({}) {}",
                    hit.number,
                    purlis_core::shown::readable(&hit.title, 100),
                    hit.state.to_lowercase(),
                    purlis_core::shown::readable(&hit.url, 200)
                );
            }
        }
        Err(e) => println!("Possible duplicates: purlis could not search ({e})."),
    }
    println!(
        "It would be filed on {} under your own account, with purlis's sign-in.",
        report::UPSTREAM
    );
    if !args.stdin && std::io::stdin().is_terminal() && std::io::stderr().is_terminal() {
        eprint!("File it? [y/N] ");
        let _ = std::io::stderr().flush();
        let mut answer = String::new();
        let _ = std::io::stdin().read_line(&mut answer);
        if matches!(answer.trim(), "y" | "Y" | "yes") {
            return send(reports, source, evidence, &digest);
        }
        eprintln!("Nothing was sent.");
        return ExitCode::SUCCESS;
    }
    println!(
        "Nothing was sent. To file exactly this draft, run the same command again with \
         --yes {digest}"
    );
    ExitCode::SUCCESS
}

/// The prefilled link that files the draft in the reporter's browser.
fn print_link(url: &str, whole: bool) {
    println!(
        "To file it yourself, open this link in your browser{}:\n{url}",
        if whole {
            ""
        } else {
            ", and paste the body from the draft above"
        }
    );
}

/// Files the draft whose digest is `shown`, and says what became of it.
fn send(reports: &Reports, source: &Caller, evidence: Evidence, shown: &str) -> ExitCode {
    match reports.file(evidence, source, shown) {
        Outcome::Sent { address } => {
            println!("Filed: {address}");
            ExitCode::SUCCESS
        }
        Outcome::Refused(Refusal::NotTheShownDraft { digest }) => {
            eprintln!(
                "charter report: `--yes {shown}` is not the draft above (its digest is {digest}), \
                 so nothing was sent. Read the draft, then pass its digest."
            );
            ExitCode::FAILURE
        }
        Outcome::Refused(Refusal::NotFiled { why, link, whole }) => {
            eprintln!(
                "charter report: purlis did not file it: {why}\n\
                 Open this link to file it in the browser instead{}:\n{link}",
                if whole {
                    ""
                } else {
                    ", and paste the body from the draft above"
                }
            );
            ExitCode::FAILURE
        }
        Outcome::Refused(refusal) => {
            eprintln!("charter report: purlis did not file it: {refusal:?}");
            ExitCode::FAILURE
        }
        // A filing is sent or refused: anything else is a state this command has no arm for
        // (#5862).
        other => CliError::internal(format!(
            "charter report: a draft was filed and the Reports module answered {other:?}"
        ))
        .exit(),
    }
}
