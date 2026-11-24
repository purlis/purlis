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
/// project, is refused there, and the sentence says what to do instead.
fn only_itself(args: &WhyArgs, own: u32) -> Option<String> {
    let named: Vec<&str> = [
        args.chat.filter(|chat| *chat != own).map(|_| "--chat"),
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

/// How long the app may take to answer a chat's `why`: it reads the project's files at `HEAD`.
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
    let request = (action.as_str(), (kind.as_str(), id.as_str()));
    let shown = match crate::session::chat_number() {
        Some(own) => {
            if let Some(refused) = only_itself(args, own) {
                return say(refused);
            }
            asked_of_the_app(&action, &kind, &id).map(|why| (why, None))
        }
        None => answered_here(args, request),
    };
    match shown {
        Ok((why, dry)) => {
            let out = if args.json {
                purlis_core::policy::why::json(&why, dry.as_ref())
            } else {
                purlis_core::policy::why::text(&why, dry.as_ref(), (&args.verb, &args.resource))
            };
            print!("{out}");
            std::process::ExitCode::SUCCESS
        }
        Err(why) => say(why),
    }
}

/// **A chat's answer about itself**, from the app that started it: the app alone holds the
/// chat's facts, and no word of this line sets one.
fn asked_of_the_app(
    action: &str,
    kind: &str,
    id: &str,
) -> Result<purlis_core::policy::why::Why, String> {
    use purlis_core::hookwire::{Answer, Ask, WhyAsk};
    let asked = crate::brokered::asked(
        |chat| {
            Ask::Why(WhyAsk {
                chat,
                action: action.to_owned(),
                kind: kind.to_owned(),
                id: id.to_owned(),
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

/// **A person's answer, here**: the policy in force over the project the folder is in, for the
/// context the flags describe, and the dry run where `--policy` names one.
fn answered_here(
    args: &WhyArgs,
    request: (&str, (&str, &str)),
) -> Result<
    (
        purlis_core::policy::why::Why,
        Option<purlis_core::policy::why::DryRun>,
    ),
    String,
> {
    use purlis_core::policy::decide::{Asker, Sealed};
    use purlis_core::policy::levels::Chat;
    let cwd =
        std::env::current_dir().map_err(|e| format!("cannot read the current directory: {e}"))?;
    let root = purlis_core::plane::resolve(&cwd).map_err(|why| why.to_string())?;
    let asker = Asker {
        chat: args.chat.unwrap_or(0),
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
    match &args.policy {
        None => Ok((
            purlis_core::policy::why::in_plane(&root, &asker, request, &sealed, None),
            None,
        )),
        Some(policy) => {
            purlis_core::policy::why::dry_run(&root, &asker, request, &sealed, None, policy)
                .map(|(why, dry)| (why, Some(dry)))
        }
    }
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
        assert_eq!(only_itself(&args(), 7), None);
        assert_eq!(
            only_itself(
                &WhyArgs {
                    chat: Some(7),
                    json: true,
                    ..args()
                },
                7
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
            let refused = only_itself(&asked, 7).unwrap_or_else(|| panic!("{flag} is allowed"));
            assert!(refused.contains(flag), "{refused}");
            assert!(refused.contains("only about itself"), "{refused}");
        }
    }
}
