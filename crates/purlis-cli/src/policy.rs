// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **This chat's `[policy]`, as the app answers it** (rulings Q308, Q308a; the review of #6992).
//!
//! Only the app can verify what the person accepted, so a hook, a git hook or `purlis secret
//! exec` asks it over the hook socket ([`Ask::Policy`]) the first time a family's mode is
//! needed, and runs every family enforced where the app does not answer
//! ([`purlis_core::enforcement::in_force`]). A log line goes to the app too ([`Ask::PolicyLog`]):
//! a process inside the chat's wrap cannot write the log, and one outside it is not trusted to.

use std::time::Duration;

use purlis_core::enforcement::{self, Family, Resolved};
use purlis_core::hookwire::{Answer, Ask, PolicyAsk, PolicyLogAsk};

/// How long a process waits for the app's answer: the app answers from what it holds.
const ANSWERED_WITHIN: Duration = Duration::from_millis(500);

/// The app's answer about this chat's policy, or none: then every family is enforced.
fn asked() -> Option<Resolved> {
    match crate::brokered::asked(|chat| Ask::Policy(PolicyAsk { chat }), ANSWERED_WITHIN) {
        Ok(Ok(Answer::Policy { spelled })) => Some(Resolved::parse(&spelled)),
        _ => None,
    }
}

/// One log line handed to the app; dropped where nobody takes it.
fn logged(family: Family, rule: &str, host: Option<&str>) {
    let _ = crate::brokered::asked(
        |chat| {
            Ask::PolicyLog(PolicyLogAsk {
                chat,
                family: family.key().to_owned(),
                rule: rule.to_owned(),
                host: host.map(str::to_owned),
            })
        },
        ANSWERED_WITHIN,
    );
}

/// **Asks the app, and logs through it**, for every command of this process. First thing in
/// `main`: nothing is asked until a family's mode is needed.
pub fn from_the_app() {
    enforcement::ask_with(asked);
    enforcement::log_to(Box::new(logged));
}

/// `purlis policy`: the project policy (M75, ADR 0127).
#[derive(clap::Subcommand, Debug)]
pub enum PolicyCommand {
    /// Print a policy. With `--default`, the shipped default compiled into this build, which a
    /// project's `policy.cedar` sits on top of.
    Show {
        /// The shipped default, as this build holds it.
        #[arg(long, required = true)]
        default: bool,
    },
    /// Say why an action on a resource would be allowed, asked or refused, and run nothing:
    /// the deciding rules with their level and owner or file and line, the mode where it changed
    /// the answer, any lock, a grant that decided, the permits a forbid overrode, and the nearest
    /// level that could loosen it. In a chat it answers for that chat only.
    Why(WhyArgs),
}

/// What `purlis policy why` is asked.
#[derive(clap::Args, Debug)]
pub struct WhyArgs {
    /// The action, a verb of the policy's schema: `merge`, `exec`, `read`, …
    pub verb: String,
    /// What it is done to, as the schema's resource names it: `acme/web#main` for a branch.
    /// Where an action takes more than one kind, name it first: `Repo:acme/web`.
    pub resource: String,
    /// Ask as this chat's number (a chat's own facts are the app's: here they come from the
    /// flags that follow).
    #[arg(long)]
    pub chat: Option<u32>,
    /// Ask for a chat of this workspace.
    #[arg(long)]
    pub workspace: Option<String>,
    /// Ask for a chat started as this persona.
    #[arg(long)]
    pub persona: Option<String>,
    /// Ask for a chat nobody is at.
    #[arg(long)]
    pub unattended: bool,
    /// Ask for a chat at this sandbox level.
    #[arg(long, value_parser = ["off", "floor", "full"])]
    pub sandbox: Option<String>,
    /// Ask for a chat this deep in a dispatch chain.
    #[arg(long)]
    pub depth: Option<i64>,
    /// Answer in JSON.
    #[arg(long)]
    pub json: bool,
    /// Answer as this policy would: a `policy.cedar` file laid over the project, or a branch of
    /// it. Adds what differs from the policy in force, as the loosening press shows it.
    #[arg(long, value_name = "FILE|BRANCH")]
    pub policy: Option<String>,
}

/// Runs `purlis policy …`.
pub fn run(command: &PolicyCommand) -> std::process::ExitCode {
    match command {
        // `--default` is required: the policy in force is `why`'s.
        PolicyCommand::Show { default: _ } => {
            print!("{}", purlis_core::policy::DEFAULT);
            std::process::ExitCode::SUCCESS
        }
        PolicyCommand::Why(args) => why(args),
    }
}

/// **What a chat may not name** (Q313.1, Q318.7, Q310.8): `purlis policy why` is a way to read the
/// policy for another context, and a chat may read it only for itself. A chat's own facts are the
/// app's, so every flag that would set one, or name another chat, or lay another policy over the
/// project, is refused there, and the sentence says what to do instead. `own` is the chat number
/// the environment names, where it names one: a chat that stripped its environment has none, so
/// no `--chat` is its own.
fn only_itself(args: &WhyArgs, own: Option<u32>) -> Option<String> {
    let named: Vec<&str> = [
        args.chat
            .filter(|chat| Some(*chat) != own)
            .map(|_| "--chat"),
        args.workspace.as_ref().map(|_| "--workspace"),
        args.persona.as_ref().map(|_| "--persona"),
        args.unattended.then_some("--unattended"),
        args.sandbox.as_ref().map(|_| "--sandbox"),
        args.depth.map(|_| "--depth"),
        args.policy.as_ref().map(|_| "--policy"),
    ]
    .into_iter()
    .flatten()
    .collect();
    (!named.is_empty()).then(|| {
        format!(
            "a chat may ask `purlis policy why` only about itself, and {} asks about something \
             else: leave the flags off to ask about this chat, or run it in a terminal outside \
             any chat",
            named.join(", ")
        )
    })
}

/// Who asks `purlis policy why`, and so whose answer it is.
#[derive(Debug, PartialEq, Eq)]
enum Route {
    /// A chat, about itself: the app answers from its record of the chat.
    Itself,
    /// The person, in a terminal outside any chat: the app answers where it can.
    Person,
}

/// **Whether the caller is a chat, and so what it may ask.** `in_a_chat` is the app's word by
/// ancestry, never the environment's alone ([`crate::brokered::in_a_chat`]: a process that
/// unsets `$CHARTER_CHAT` and `$CHARTER_SESSION_ID` is still in its chat, and an app that cannot
/// answer leaves it one), so no flag a chat may not name reaches the person's route.
fn route(args: &WhyArgs, in_a_chat: bool, own: Option<u32>) -> Result<Route, String> {
    if !in_a_chat {
        return Ok(Route::Person);
    }
    match only_itself(args, own) {
        Some(refused) => Err(refused),
        None => Ok(Route::Itself),
    }
}

/// How long the app may take to answer a `why`: it reads the project's files at `HEAD`.
const WHY_IS_ANSWERED_WITHIN: Duration = Duration::from_secs(10);

/// `purlis policy why`.
fn why(args: &WhyArgs) -> std::process::ExitCode {
    let say = |sentence: String| {
        crate::voice::err(sentence);
        std::process::ExitCode::FAILURE
    };
    let (action, kind, id) = match purlis_core::policy::why::request_of(&args.verb, &args.resource)
    {
        Ok(request) => request,
        Err(why) => return say(why),
    };
    let route = match route(
        args,
        crate::brokered::in_a_chat(),
        crate::session::chat_number(),
    ) {
        Ok(route) => route,
        Err(refused) => return say(refused),
    };
    let shown = match route {
        Route::Itself => asked_of_the_app(&action, &kind, &id),
        Route::Person => here().and_then(|cwd| as_the_person(args, (&action, &kind, &id), &cwd)),
    };
    match shown {
        Ok(why) => {
            let out = if args.json {
                purlis_core::policy::why::json(&why)
            } else {
                purlis_core::policy::why::text(&why, (&args.verb, &args.resource))
            };
            print!("{out}");
            std::process::ExitCode::SUCCESS
        }
        Err(why) => say(why),
    }
}

/// The folder this command runs in, as the app is told where it ran.
fn here() -> Result<std::path::PathBuf, String> {
    std::env::current_dir().map_err(|e| format!("cannot read the current directory: {e}"))
}

/// **A chat's answer about itself**, from the app that started it: the app alone holds the
/// chat's facts, and no word of this line sets one.
fn asked_of_the_app(
    action: &str,
    kind: &str,
    id: &str,
) -> Result<purlis_core::policy::why::Why, String> {
    use purlis_core::hookwire::{Answer, Ask, WhyAsk};
    let cwd = here().ok().map(|cwd| cwd.display().to_string());
    let asked = crate::brokered::asked(
        |chat| {
            Ask::Why(WhyAsk {
                chat,
                action: action.to_owned(),
                kind: kind.to_owned(),
                id: id.to_owned(),
                cwd,
            })
        },
        WHY_IS_ANSWERED_WITHIN,
    );
    match asked {
        Ok(Ok(Answer::Why(why))) => Ok(*why),
        Ok(Ok(Answer::No { why })) => Err(why),
        Ok(Ok(_)) => Err("the app answered something else, so nothing is known".to_owned()),
        Ok(Err(e)) => Err(format!("the app did not answer ({e})")),
        Err(not_taken) => Err(not_taken.why().to_owned()),
    }
}

/// What the app said to the person's ask.
enum Heard {
    /// Its answer.
    Said(Box<purlis_core::policy::why::Why>),
    /// It refused, and why: its words stand.
    Refused(String),
    /// No app answered (nothing listens, or it ended the connection unanswered), and why.
    Silent(String),
}

/// **The person's ask to the app of the plane they stand in**, on its socket, with no chat's
/// token: the app answers it only for a process in none of its chats.
fn asked_as_the_person(ask: purlis_core::hookwire::PersonWhyAsk, cwd: &std::path::Path) -> Heard {
    use purlis_core::hookwire::{Answer, Ask, Asking};
    let socket = purlis_core::hookwire::reach::socket_here(cwd);
    let mut asking = match Asking::on(&socket, None) {
        Ok(asking) => asking,
        Err(e) => {
            return Heard::Silent(format!("nothing listens at {} ({e})", socket.display()));
        }
    };
    match asking.ask(&Ask::PersonWhy(ask), WHY_IS_ANSWERED_WITHIN) {
        Ok(Answer::Why(why)) => Heard::Said(why),
        // An app older than the ask ends the connection with the sentence it says to a line it
        // cannot read: no app that knows this question answered.
        Ok(Answer::No { why }) if why == purlis_core::hookwire::UNKNOWN_LINE => {
            Heard::Silent("the app is older than this question".to_owned())
        }
        Ok(Answer::No { why }) => Heard::Refused(why),
        Ok(_) => Heard::Silent("the app answered something else".to_owned()),
        Err(e) => Heard::Silent(format!("the app did not answer ({e})")),
    }
}

/// **The person's answer**: the app's, which reads the sealed side (the accepted copy and floor,
/// grants and nevers, and so the dry run's diff against what they accepted). Where no app
/// answers, the answer is made here from the committed project alone and **says so** ([`why::SEALED_UNREAD`]),
/// and `--chat N` is refused: a chat's facts are the app's, and no word of this line describes one.
fn as_the_person(
    args: &WhyArgs,
    (action, kind, id): (&str, &str, &str),
    cwd: &std::path::Path,
) -> Result<purlis_core::policy::why::Why, String> {
    if let Some(chat) = args.chat
        && (args.workspace.is_some()
            || args.persona.is_some()
            || args.unattended
            || args.sandbox.is_some()
            || args.depth.is_some())
    {
        return Err(format!(
            "--chat {chat} asks as that chat, whose facts are the app's own record: leave \
             --workspace, --persona, --unattended, --sandbox and --depth off"
        ));
    }
    let heard = asked_as_the_person(
        purlis_core::hookwire::PersonWhyAsk {
            action: action.to_owned(),
            kind: kind.to_owned(),
            id: id.to_owned(),
            cwd: Some(cwd.display().to_string()),
            chat: args.chat,
            workspace: args.workspace.clone(),
            persona: args.persona.clone(),
            unattended: args.unattended,
            sandbox: args.sandbox.clone(),
            depth: args.depth,
            policy: args.policy.clone(),
        },
        cwd,
    );
    match heard {
        Heard::Said(why) => Ok(*why),
        Heard::Refused(why) => Err(why),
        Heard::Silent(reason) => match args.chat {
            Some(chat) => Err(format!(
                "no purlis app answered ({reason}), and chat {chat}'s facts are the app's: start \
                 the app and run this again, or leave --chat off and name the context with \
                 --workspace, --persona, --unattended, --sandbox and --depth"
            )),
            None => answered_here(args, (action, (kind, id)), cwd),
        },
    }
}

/// **A person's answer, here**, where no app answered: the project as committed over the folder's
/// plane, for the context the flags describe, and the dry run where `--policy` names one. This
/// process cannot read the sealed store (`Sealed::of` is the app's), so the answer says it was
/// not read.
fn answered_here(
    args: &WhyArgs,
    request: (&str, (&str, &str)),
    cwd: &std::path::Path,
) -> Result<purlis_core::policy::why::Why, String> {
    use purlis_core::policy::decide::{Asker, Sealed};
    use purlis_core::policy::levels::Chat;
    use purlis_core::policy::why;
    let root = purlis_core::plane::resolve(cwd).map_err(|why| why.to_string())?;
    let asker = Asker {
        chat: 0,
        of: Chat {
            workspace: args.workspace.as_deref(),
            persona: args.persona.as_deref(),
        },
        sandbox: args.sandbox.as_deref().unwrap_or("floor"),
        attended: !args.unattended,
        harness_mode: "default",
        depth: args.depth.unwrap_or(0),
        via: "hook",
        start: None,
    };
    let sealed = Sealed::of(&root);
    let mut answered = match &args.policy {
        None => why::in_plane(&root, &asker, request, &sealed, None),
        Some(policy) => why::dry_run(&root, &asker, request, &sealed, None, policy)?,
    };
    answered.sealed_unread = Some(why::SEALED_UNREAD.to_owned());
    Ok(answered)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args() -> WhyArgs {
        WhyArgs {
            verb: "merge".to_owned(),
            resource: "acme/web#main".to_owned(),
            chat: None,
            workspace: None,
            persona: None,
            unattended: false,
            sandbox: None,
            depth: None,
            json: false,
            policy: None,
        }
    }

    #[test]
    fn a_chat_may_ask_policy_why_only_about_itself() {
        // Its own question, with or without its own number, is answered.
        assert_eq!(only_itself(&args(), Some(7)), None);
        assert_eq!(
            only_itself(
                &WhyArgs {
                    chat: Some(7),
                    json: true,
                    ..args()
                },
                Some(7)
            ),
            None
        );
        // Every way to ask about something else is refused, and named.
        for (flag, asked) in [
            (
                "--chat",
                WhyArgs {
                    chat: Some(8),
                    ..args()
                },
            ),
            (
                "--workspace",
                WhyArgs {
                    workspace: Some("web".into()),
                    ..args()
                },
            ),
            (
                "--persona",
                WhyArgs {
                    persona: Some("builder".into()),
                    ..args()
                },
            ),
            (
                "--unattended",
                WhyArgs {
                    unattended: true,
                    ..args()
                },
            ),
            (
                "--sandbox",
                WhyArgs {
                    sandbox: Some("off".into()),
                    ..args()
                },
            ),
            (
                "--depth",
                WhyArgs {
                    depth: Some(1),
                    ..args()
                },
            ),
            (
                "--policy",
                WhyArgs {
                    policy: Some("main".into()),
                    ..args()
                },
            ),
        ] {
            let refused =
                only_itself(&asked, Some(7)).unwrap_or_else(|| panic!("{flag} is allowed"));
            assert!(refused.contains(flag), "{refused}");
            assert!(refused.contains("only about itself"), "{refused}");
        }
    }

    #[test]
    fn a_chat_that_stripped_its_environment_may_still_ask_only_about_itself() {
        // `env -u CHARTER_CHAT -u CHARTER_SESSION_ID`: the environment names no chat (`own` is
        // none), and the app, by the caller's ancestry, says it is in one. Every flag a chat may
        // not name is refused, `--chat` included: no number is its own.
        for asked in [
            WhyArgs {
                chat: Some(7),
                ..args()
            },
            WhyArgs {
                workspace: Some("web".into()),
                ..args()
            },
            WhyArgs {
                policy: Some("main".into()),
                ..args()
            },
        ] {
            let refused = route(&asked, true, None).expect_err("a chat's flags are refused");
            assert!(refused.contains("only about itself"), "{refused}");
        }
        // Its own plain question is answered, by the app, for the chat the app names.
        assert_eq!(route(&args(), true, None), Ok(Route::Itself));
        // Only a caller the app says is in no chat is the person's.
        assert_eq!(
            route(
                &WhyArgs {
                    workspace: Some("web".into()),
                    ..args()
                },
                false,
                None
            ),
            Ok(Route::Person)
        );
    }

    #[test]
    fn chat_n_from_outside_a_chat_is_the_apps_record_and_never_an_invented_answer() {
        // Outside a chat, no app: `--chat 7` is the app's record, so it is refused with a reason
        // rather than answered from invented facts. (No hook socket stands in this folder.)
        let folder = tempfile::tempdir().expect("a folder");
        let refused = as_the_person(
            &WhyArgs {
                chat: Some(7),
                ..args()
            },
            ("merge", "Branch", "acme/web#main"),
            folder.path(),
        );
        let refused = refused.expect_err("no app, no chat's facts");
        // Either no app answered, or one did and does not have chat 7 open: never an answer.
        assert!(refused.contains("chat 7"), "{refused}");
    }
}
