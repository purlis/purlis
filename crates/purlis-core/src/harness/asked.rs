// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Each source's own ask, read into the one [`Ask`] (HP-5, research track 04 §5.1).
//!
//! A source is a harness's own channel for asking: Claude Code's and Codex's
//! `PermissionRequest` hook at level 2, the Codex app-server, opencode's permission request,
//! and ACP's `session/request_permission` at level 3. Each reader takes the source's payload as
//! the source sent it and keeps its options **as the source gave them**, in its order and its
//! words, so that an answer goes back as one of them (HP-6, HP-16). Nothing here decides
//! anything; a payload missing a field reads as an ask that says less, never as no ask.

use serde_json::Value;

use super::model::{
    Action, Ask, Channel, Choice, ChoiceKind, ChoiceScope, Deadline, Risk, Summary,
};

pub mod edit;
pub mod question;

/// Claude Code's `PermissionRequest` hook payload, on a hook armed with `hook_timeout`.
///
/// Its options are allow and deny for this call, and between them one for each
/// `permission_suggestions` entry charter can say truthfully ([`suggestion`]). The hook answers
/// with a `decision` naming one.
pub fn claude_permission_request(payload: &Value, hook_timeout: std::time::Duration) -> Ask {
    if payload["tool_name"].as_str() == Some(question::TOOL) {
        return question::claude(payload, hook_timeout);
    }
    let mut options = vec![choice(
        "allow",
        "Allow",
        ChoiceKind::Allow,
        ChoiceScope::Once,
    )];
    let suggestions = payload["permission_suggestions"].as_array();
    options.extend(
        suggestions
            .into_iter()
            .flatten()
            .enumerate()
            .filter_map(|(n, entry)| suggestion(n, entry)),
    );
    options.push(choice(
        "deny",
        "Deny",
        ChoiceKind::Reject,
        ChoiceScope::Once,
    ));
    let action = tool_action(payload);
    let writes = matches!(action, Action::Edit { .. })
        .then(|| edit::claude(payload))
        .flatten();
    Ask {
        writes,
        ..hooked(action, options, hook_timeout)
    }
}

/// The option one of Claude Code's `permission_suggestions` offers, read by its `type`, or
/// `None` where charter cannot say what choosing it does.
///
/// Offered: `addRules` that allow or deny, `setMode` naming the mode, and `addDirectories`
/// naming the directories; each says whether it lasts for the session or is saved, and where.
/// Never offered: `removeRules`, `replaceRules` and `removeDirectories`, which rewrite standing
/// rules (removing a deny rule loosens them) rather than answer this call; an `ask` rule, which
/// is no answer; and any type or behaviour charter has not read. Dropping an option fails
/// towards asking: allow and deny for this call are always there.
fn suggestion(n: usize, entry: &Value) -> Option<Choice> {
    let (scope, lasting) = match entry["destination"].as_str()? {
        "session" => (ChoiceScope::Session, " for this session".to_owned()),
        saved => (ChoiceScope::Always, format!(" from now on ({saved})")),
    };
    let (kind, what) = match entry["type"].as_str()? {
        "addRules" => {
            let rules = listed(&entry["rules"], |rule| {
                let tool = rule["toolName"].as_str()?;
                Some(match rule["ruleContent"].as_str() {
                    Some(content) => format!("{tool}({content})"),
                    None => tool.to_owned(),
                })
            })?;
            match entry["behavior"].as_str()? {
                "allow" => (ChoiceKind::Allow, format!("Allow {rules}")),
                "deny" => (ChoiceKind::Reject, format!("Deny {rules}")),
                _ => return None,
            }
        }
        "setMode" => {
            let mode = entry["mode"].as_str().filter(|mode| !mode.is_empty())?;
            (
                ChoiceKind::Allow,
                format!("Allow, and switch to {mode} mode"),
            )
        }
        "addDirectories" => {
            let directories = listed(&entry["directories"], |dir| dir.as_str().map(str::to_owned))?;
            (
                ChoiceKind::Allow,
                format!("Allow, and give access to {directories}"),
            )
        }
        _ => return None,
    };
    Some(choice(
        &format!("suggestion:{n}"),
        &format!("{what}{lasting}"),
        kind,
        scope,
    ))
}

/// Each entry of `list` read by `read`, joined with "and", or `None` when the list is empty or
/// any entry is unreadable: a label that left one out would not say what is granted.
fn listed(list: &Value, read: impl Fn(&Value) -> Option<String>) -> Option<String> {
    let all = list
        .as_array()?
        .iter()
        .map(read)
        .collect::<Option<Vec<String>>>()?;
    match all.split_last()? {
        (last, []) => Some(last.clone()),
        (last, rest) => Some(format!("{} and {last}", rest.join(", "))),
    }
}

/// Codex's `PermissionRequest` hook payload, on a hook armed with `hook_timeout`. Its hook
/// answers allow or deny for this call and nothing longer.
pub fn codex_permission_request(payload: &Value, hook_timeout: std::time::Duration) -> Ask {
    let options = vec![
        choice("allow", "Allow", ChoiceKind::Allow, ChoiceScope::Once),
        choice("deny", "Deny", ChoiceKind::Reject, ChoiceScope::Once),
    ];
    hooked(tool_action(payload), options, hook_timeout)
}

/// A Codex app-server approval request: its `method` and `params`, or `None` for a method
/// other than the two approval requests this reads, whose decisions are other words (v1's
/// `execCommandApproval`, for one). Its four decisions are the options, and it waits for its
/// answer, so the ask has no deadline.
///
/// A file change with a `grantRoot` asks for writes anywhere under that root for the rest of
/// the session, not for one file, and its summary says so.
pub fn codex_app_server(method: &str, params: &Value) -> Option<Ask> {
    let root = params["grantRoot"]
        .as_str()
        .filter(|_| method == "item/fileChange/requestApproval");
    let action = match method {
        "item/fileChange/requestApproval" => match root {
            Some(path) => Action::Edit {
                path: path.to_owned(),
            },
            None => tool("apply_patch", &params["reason"]),
        },
        "item/commandExecution/requestApproval" => match params["command"].as_str() {
            Some(line) => Action::Command {
                line: line.to_owned(),
            },
            None => tool(method, params),
        },
        _ => return None,
    };
    let options = vec![
        choice("accept", "Accept", ChoiceKind::Allow, ChoiceScope::Once),
        choice(
            "acceptForSession",
            "Accept for this session",
            ChoiceKind::Allow,
            ChoiceScope::Session,
        ),
        choice("decline", "Decline", ChoiceKind::Reject, ChoiceScope::Once),
        choice(
            "cancel",
            "Cancel the turn",
            ChoiceKind::Cancel,
            ChoiceScope::Once,
        ),
    ];
    let mut ask = asking(action, options);
    if let Some(root) = root {
        ask.summary = Summary::of(&format!("Allow writes anywhere under {root}"));
    }
    Some(Ask {
        channel: Channel::CodexAppServer {
            thread: text(&params["threadId"]),
            turn: text(&params["turnId"]),
            item: text(&params["itemId"]),
        },
        ..ask
    })
}

/// opencode's `permission.asked` event properties. It offers once, always (for the rest of the
/// session) and reject, answered on its server, which waits, so the ask has no deadline.
pub fn opencode_permission(props: &Value) -> Ask {
    let kind = props["permission"].as_str().unwrap_or_default();
    let first = props["patterns"]
        .as_array()
        .and_then(|all| all.first())
        .and_then(Value::as_str);
    let action = match (kind, first) {
        ("bash", Some(line)) => Action::Command {
            line: line.to_owned(),
        },
        ("edit", Some(path)) => Action::Edit {
            path: path.to_owned(),
        },
        _ => tool(kind, &props["patterns"]),
    };
    let options = vec![
        choice("once", "Allow once", ChoiceKind::Allow, ChoiceScope::Once),
        choice(
            "always",
            &always_label(&props["always"]),
            ChoiceKind::Allow,
            ChoiceScope::Session,
        ),
        choice("reject", "Reject", ChoiceKind::Reject, ChoiceScope::Once),
    ];
    Ask {
        channel: Channel::Opencode {
            session: text(&props["sessionID"]),
            request: text(&props["id"]),
        },
        ..asking(action, options)
    }
}

/// opencode's `permission.asked` properties, as purlis's shim hands them to its permission hook
/// (#1691): the same ask, answered on the hook's connection before `hook_timeout`, and the shim
/// replies on opencode's own client.
pub fn opencode_permission_hooked(props: &Value, hook_timeout: std::time::Duration) -> Ask {
    let ask = opencode_permission(props);
    hooked(ask.action, ask.options, hook_timeout)
}

/// The label of opencode's always option, naming the `always` patterns it approves for the
/// rest of the session, which can be broader than the call asked about (`git push *` for
/// `git push origin main`). Joined and cut like a [`Summary`], credential shapes masked. With no
/// readable patterns it says only "Always allow", claiming no scope it cannot name; so does a
/// pattern holding a character a window draws as nothing or draws elsewhere
/// ([`purlis_unicode::hidden`]), which could make the label read narrower than it is.
fn always_label(patterns: &Value) -> String {
    let readable = |pattern: &Value| {
        pattern
            .as_str()
            .filter(|text| !purlis_unicode::hides(text, purlis_unicode::Breaks::Marked))
            .map(str::to_owned)
    };
    match listed(patterns, readable) {
        Some(patterns) => Summary::of(&format!("Always allow {patterns}"))
            .as_str()
            .to_owned(),
        None => "Always allow".to_owned(),
    }
}

/// ACP's `session/request_permission` params. The agent's options are kept in its words, each
/// read by its `kind`; the agent waits for its answer, so the ask has no deadline (ADR 0080 §5,
/// V28d). A tool call the agent itself says deletes is a [`Risk::Delete`].
pub fn acp_request_permission(params: &Value) -> Ask {
    let call = &params["toolCall"];
    let kind = call["kind"].as_str().unwrap_or_default();
    let raw = &call["rawInput"];
    let path = call["locations"]
        .as_array()
        .and_then(|all| all.first())
        .and_then(|at| at["path"].as_str());
    let line = match &raw["command"] {
        Value::String(line) => Some(line.clone()),
        Value::Array(words) => Some(
            words
                .iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join(" "),
        ),
        _ => None,
    };
    let action = match (kind, line, path) {
        ("execute", Some(line), _) => Action::Command { line },
        ("edit" | "delete" | "move", _, Some(path)) => Action::Edit {
            path: path.to_owned(),
        },
        _ => tool(if kind.is_empty() { "a tool" } else { kind }, raw),
    };
    let options = params["options"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|option| {
            let (kind, scope) = match option["kind"].as_str()? {
                "allow_once" => (ChoiceKind::Allow, ChoiceScope::Once),
                "allow_always" => (ChoiceKind::Allow, ChoiceScope::Always),
                "reject_once" => (ChoiceKind::Reject, ChoiceScope::Once),
                "reject_always" => (ChoiceKind::Reject, ChoiceScope::Always),
                _ => return None,
            };
            let id = option["optionId"].as_str()?;
            Some(choice(
                id,
                option["name"].as_str().unwrap_or(id),
                kind,
                scope,
            ))
        })
        .collect();
    let mut ask = asking(action, options);
    if let Some(title) = call["title"]
        .as_str()
        .filter(|title| !title.trim().is_empty())
    {
        ask.summary = Summary::of(title);
    }
    Ask {
        risk: if kind == "delete" {
            more_cautious(Risk::Delete, ask.risk)
        } else {
            ask.risk
        },
        channel: Channel::Acp {
            session: text(&params["sessionId"]),
            tool_call: text(&call["toolCallId"]),
        },
        ..ask
    }
}

/// A string field, or empty.
fn text(value: &Value) -> String {
    value.as_str().unwrap_or_default().to_owned()
}

/// An ask that came on a hook: answered on the hook's connection, before the hook times out.
fn hooked(action: Action, options: Vec<Choice>, hook_timeout: std::time::Duration) -> Ask {
    match Deadline::below_hook_timeout(hook_timeout) {
        Some(deadline) => Ask {
            deadline,
            channel: Channel::Hook,
            ..asking(action, options)
        },
        // No room below the hook's timeout to answer it: the pane is the only place it can be.
        None => Ask {
            options: Vec::new(),
            ..asking(action, Vec::new())
        },
    }
}

/// An ask for `action` with `options`, its summary written from the action.
fn asking(action: Action, options: Vec<Choice>) -> Ask {
    let summary = Summary::of(&match &action {
        Action::Unsaid => "Asks for you".to_owned(),
        Action::Command { line } => format!("Run a command: {line}"),
        Action::Edit { path } => format!("Change {path}"),
        Action::Tool { name, input } => format!("Use {name}: {input}"),
        Action::Elicit { fields } => format!("Give values for: {}", fields.join(", ")),
        Action::Question { question } => format!("Asks you: {question}"),
    });
    Ask {
        risk: risk_of(&action),
        action,
        options,
        summary,
        ..Ask::default()
    }
}

/// What saying yes to `action` risks, read from the action alone (SD-18's classes, #1046 L2):
/// a command line that deletes or that uses a secret, or a tool whose name says it does.
///
/// **It only ever makes an ask more cautious.** It answers a risky class or [`Risk::Normal`],
/// and a class the source gave itself is kept over it ([`more_cautious`]). A line it cannot
/// read is [`Risk::Normal`], which is what every ask was before. A push to a protected branch
/// and a production-like host need the project's own facts (#1046 L1), so they are not guessed
/// here.
pub fn risk_of(action: &Action) -> Risk {
    match action {
        Action::Command { line } => risk::of_line(line, 0),
        Action::Tool { name, .. } => risk::of_tool(name),
        Action::Unsaid | Action::Edit { .. } | Action::Elicit { .. } | Action::Question { .. } => {
            Risk::Normal
        }
    }
}

/// The graver of the class a source `gave` and the one `found` in the action, the source's on
/// a tie: a delete the line shows also hands over a secret reads as a secret use. Never a lower
/// one than either.
pub fn more_cautious(gave: Risk, found: Risk) -> Risk {
    risk::graver(gave, found)
}

/// The reading [`risk_of`] does on a command line and a tool name.
mod risk {
    use super::Risk;
    use crate::names::BINARY;
    use crate::shellseg;
    use crate::shellwrap::{self, base_lower};

    /// How many shells inside shells (`sh -c '…'`) a line is read through.
    const SCRIPT_DEPTH: usize = 4;

    /// The graver of two classes: a secret use over a delete over nothing.
    pub(super) fn graver(a: Risk, b: Risk) -> Risk {
        let rank = |r: Risk| match r {
            Risk::Normal => 0,
            Risk::Delete => 1,
            _ => 2,
        };
        if rank(b) > rank(a) { b } else { a }
    }

    /// Every command `line` runs, each read, the gravest class answered.
    pub(super) fn of_line(line: &str, depth: usize) -> Risk {
        shellseg::segment_argv(line)
            .iter()
            .map(|toks| of_segment(toks, depth))
            .fold(Risk::Normal, graver)
    }

    /// One command's words.
    fn of_segment(toks: &[String], depth: usize) -> Risk {
        // purlis's own secret verbs, wherever the program stands: `secret exec` is read as a
        // wrapper of the program it runs, so it is looked for before the wrappers come off.
        if (0..toks.len()).any(|i| hands_over_a_secret(&toks[i..])) {
            return Risk::SecretUse;
        }
        let it = shellwrap::split_env_chdir(toks);
        let mut found = of_program(&base_lower(&it.prog), it.argv.get(1..).unwrap_or(&[]));
        for run in shellwrap::commands_a_find_runs(&it.prog, &it.argv) {
            found = graver(found, of_segment(&run, depth));
        }
        if depth < SCRIPT_DEPTH {
            for script in shellwrap::shell_scripts_of(&it.prog, &it.argv) {
                found = graver(found, of_line(&script, depth + 1));
            }
        }
        found
    }

    /// Whether `words` start with purlis's command handing a secret's value over: `secret get`,
    /// `secret exec` or `secret cp`, the persona's included.
    fn hands_over_a_secret(words: &[String]) -> bool {
        let Some((program, rest)) = words.split_first() else {
            return false;
        };
        if !BINARY.recognises(&base_lower(program)) {
            return false;
        }
        let rest: Vec<&str> = rest.iter().map(String::as_str).collect();
        let verb = match rest.as_slice() {
            ["secret", verb, ..] | ["persona", "secret", verb, ..] => *verb,
            _ => return false,
        };
        matches!(verb, "get" | "exec" | "cp")
    }

    /// The words of `args` that are not options, in order.
    fn words(args: &[String]) -> Vec<&str> {
        args.iter()
            .map(String::as_str)
            .filter(|word| !word.starts_with('-'))
            .collect()
    }

    /// The words of `args` that are neither options nor the value of one of `valued`, a
    /// program's own options that take the next word as their value (`aws --region eu-west-1
    /// s3 rm …`), in order. Without this an option's value reads as the subcommand (#1046).
    /// `--name=value` is one word and needs nothing skipped.
    fn words_past<'a>(args: &'a [String], valued: &[&str]) -> Vec<&'a str> {
        let mut out = Vec::new();
        let mut skip = false;
        for arg in args {
            if std::mem::take(&mut skip) {
                continue;
            }
            if arg.starts_with('-') {
                skip = valued.contains(&arg.as_str());
                continue;
            }
            out.push(arg.as_str());
        }
        out
    }

    /// The AWS CLI's global options that take a value.
    const AWS_VALUED: &[&str] = &[
        "--region",
        "--profile",
        "--output",
        "--endpoint-url",
        "--query",
        "--ca-bundle",
        "--color",
        "--cli-read-timeout",
        "--cli-connect-timeout",
        "--cli-binary-format",
    ];

    /// The Google Cloud CLI's global options that take a value, and the two most often written
    /// before the group.
    const GCLOUD_VALUED: &[&str] = &[
        "--project",
        "--account",
        "--configuration",
        "--format",
        "--verbosity",
        "--billing-project",
        "--impersonate-service-account",
        "--flags-file",
        "--trace-token",
        "--region",
        "--zone",
    ];

    /// The Azure CLI's global options that take a value.
    const AZ_VALUED: &[&str] = &["--subscription", "--output", "-o", "--query"];

    /// kubectl's (and oc's) global options that take a value.
    const KUBECTL_VALUED: &[&str] = &[
        "-n",
        "--namespace",
        "--context",
        "--cluster",
        "--user",
        "--kubeconfig",
        "-s",
        "--server",
        "--token",
        "--as",
        "--as-group",
        "--as-uid",
        "--certificate-authority",
        "--client-certificate",
        "--client-key",
        "--request-timeout",
        "--tls-server-name",
        "-v",
        "--v",
    ];

    /// Whether `args` holds one of `flags`, or a cluster of short options holding one of
    /// `letters` (`-fD`).
    fn has_flag(args: &[String], flags: &[&str], letters: &[char]) -> bool {
        args.iter().any(|arg| {
            flags.contains(&arg.as_str())
                || (arg.starts_with('-')
                    && !arg.starts_with("--")
                    && arg.chars().skip(1).any(|c| letters.contains(&c)))
        })
    }

    /// `program` (its base name, lower case) run with `args`.
    fn of_program(program: &str, args: &[String]) -> Risk {
        let words = words(args);
        let first = |n: usize| words.iter().take(n).copied().collect::<Vec<_>>();
        match program {
            "rm" | "rmdir" | "unlink" | "shred" | "srm" => Risk::Delete,
            "find" if args.iter().any(|arg| arg == "-delete") => Risk::Delete,
            "git" => of_git(args),
            "op" => match first(2).as_slice() {
                ["read" | "run" | "inject", ..] | ["item" | "document", "get"] => Risk::SecretUse,
                ["item" | "document" | "vault", "delete"] => Risk::Delete,
                _ => Risk::Normal,
            },
            "security" => match words.first().copied().unwrap_or_default() {
                "dump-keychain" => Risk::SecretUse,
                sub if sub.starts_with("find-") && sub.ends_with("-password") => Risk::SecretUse,
                sub if sub.starts_with("delete-") => Risk::Delete,
                _ => Risk::Normal,
            },
            "vault" => match first(2).as_slice() {
                ["read", ..] | ["kv", "get"] => Risk::SecretUse,
                ["delete", ..] | ["kv", "delete" | "destroy" | "metadata"] => Risk::Delete,
                _ => Risk::Normal,
            },
            // Read both ways, the graver kept (#1046): with each global option's value skipped,
            // so `--region x s3 rm` is the delete it is, and with every value as a word, as
            // before, so naming an option's value never makes a line ask less.
            "aws" => read_both(args, AWS_VALUED, of_aws),
            "gcloud" => read_both(args, GCLOUD_VALUED, of_gcloud),
            "az" => read_both(args, AZ_VALUED, of_az),
            "gh" | "glab" => match first(2).as_slice() {
                ["auth", "token"] => Risk::SecretUse,
                _ if first(3).contains(&"delete") => Risk::Delete,
                _ => Risk::Normal,
            },
            "kubectl" | "oc" => read_both(args, KUBECTL_VALUED, of_kubectl),
            "helm" => match words.first().copied() {
                Some("uninstall" | "delete") => Risk::Delete,
                _ => Risk::Normal,
            },
            "docker" | "podman" => {
                if first(2)
                    .iter()
                    .any(|word| matches!(*word, "rm" | "rmi" | "prune"))
                {
                    Risk::Delete
                } else {
                    Risk::Normal
                }
            }
            _ => Risk::Normal,
        }
    }

    /// `of` over `args`' words read both ways, the graver answered: past each option of
    /// `valued` and its value ([`words_past`]), and with every value as a word ([`words`]).
    fn read_both(args: &[String], valued: &[&str], of: fn(&[&str]) -> Risk) -> Risk {
        graver(of(&words(args)), of(&words_past(args, valued)))
    }

    /// The AWS CLI's subcommand words.
    fn of_aws(words: &[&str]) -> Risk {
        match words {
            ["secretsmanager", "get-secret-value", ..] => Risk::SecretUse,
            ["s3", "rm" | "rb", ..] => Risk::Delete,
            [_, sub, ..] if sub.starts_with("delete-") => Risk::Delete,
            _ => Risk::Normal,
        }
    }

    /// The Google Cloud CLI's subcommand words.
    fn of_gcloud(words: &[&str]) -> Risk {
        match words {
            ["secrets", "versions", "access", ..] => Risk::SecretUse,
            _ if words.iter().take(4).any(|word| *word == "delete") => Risk::Delete,
            _ => Risk::Normal,
        }
    }

    /// The Azure CLI's subcommand words.
    fn of_az(words: &[&str]) -> Risk {
        match words {
            ["keyvault", "secret", "show" | "download", ..] => Risk::SecretUse,
            _ if words
                .iter()
                .take(4)
                .any(|word| matches!(*word, "delete" | "purge")) =>
            {
                Risk::Delete
            }
            _ => Risk::Normal,
        }
    }

    /// kubectl's (and oc's) words. An option this reading does not skip leaves its value as a
    /// word, so the verb is looked for among the first few.
    fn of_kubectl(words: &[&str]) -> Risk {
        let first = &words[..words.len().min(4)];
        let reads_a_secret = first.windows(2).any(|pair| {
            matches!(pair[0], "get" | "describe")
                && (matches!(pair[1], "secret" | "secrets") || pair[1].starts_with("secret/"))
        });
        if reads_a_secret {
            Risk::SecretUse
        } else if first.contains(&"delete") {
            Risk::Delete
        } else {
            Risk::Normal
        }
    }

    /// `git` run with `args` (the program not included).
    fn of_git(args: &[String]) -> Risk {
        let with_program: Vec<String> = std::iter::once("git".to_owned())
            .chain(args.iter().cloned())
            .collect();
        let (_globals, rest) = shellwrap::git_globals(&with_program);
        let Some((sub, args)) = rest.split_first() else {
            return Risk::Normal;
        };
        let words = words(args);
        let deletes = match sub.as_str() {
            "rm" | "clean" => true,
            "branch" => has_flag(args, &["--delete"], &['d', 'D']),
            "tag" => has_flag(args, &["--delete"], &['d']),
            "update-ref" => has_flag(args, &[], &['d']),
            "push" => {
                has_flag(args, &["--delete", "--mirror", "--prune"], &['d'])
                    || words
                        .iter()
                        .any(|refspec| refspec.starts_with(':') || refspec.starts_with("+:"))
            }
            "worktree" => matches!(words.first(), Some(&("remove" | "prune"))),
            "stash" => matches!(words.first(), Some(&("drop" | "clear"))),
            _ => false,
        };
        if deletes { Risk::Delete } else { Risk::Normal }
    }

    /// A tool by its name's words (`mcp__github__delete_file`, `removeItem`): one that says it
    /// deletes, or that reads a secret.
    pub(super) fn of_tool(name: &str) -> Risk {
        let mut words: Vec<String> = Vec::new();
        let mut word = String::new();
        for c in name.chars() {
            // A word ends at anything not a letter or digit, and before a capital.
            if (!c.is_alphanumeric() || c.is_uppercase()) && !word.is_empty() {
                words.push(std::mem::take(&mut word));
            }
            if c.is_alphanumeric() {
                word.extend(c.to_lowercase());
            }
        }
        if !word.is_empty() {
            words.push(word);
        }
        let says = |these: &[&str]| words.iter().any(|w| these.contains(&w.as_str()));
        if says(&[
            "secret",
            "secrets",
            "password",
            "passwords",
            "credential",
            "credentials",
        ]) {
            Risk::SecretUse
        } else if says(&["delete", "remove", "destroy", "purge", "erase"]) {
            Risk::Delete
        } else {
            Risk::Normal
        }
    }
}

/// The action of a hook payload's `tool_name` and `tool_input`, in Claude Code's tool names,
/// which Codex's hooks share.
fn tool_action(payload: &Value) -> Action {
    let name = payload["tool_name"].as_str().unwrap_or_default();
    let input = &payload["tool_input"];
    if let Some(line) = input["command"].as_str().filter(|_| is_shell(name)) {
        return Action::Command {
            line: line.to_owned(),
        };
    }
    let path = input["file_path"]
        .as_str()
        .or(input["notebook_path"].as_str());
    if let Some(path) =
        path.filter(|_| matches!(name, "Edit" | "MultiEdit" | "Write" | "NotebookEdit"))
    {
        return Action::Edit {
            path: path.to_owned(),
        };
    }
    tool(name, input)
}

/// Whether a tool name is a shell, as each source names it.
fn is_shell(name: &str) -> bool {
    matches!(
        name,
        "Bash" | "bash" | "shell" | "exec_command" | "local_shell"
    )
}

/// Any other tool, with its input as compact JSON.
fn tool(name: &str, input: &Value) -> Action {
    let name = if name.is_empty() { "a tool" } else { name };
    Action::Tool {
        name: name.to_owned(),
        input: if input.is_null() {
            String::new()
        } else {
            input.to_string()
        },
    }
}

fn choice(id: &str, label: &str, kind: ChoiceKind, scope: ChoiceScope) -> Choice {
    Choice {
        id: id.to_owned(),
        label: label.to_owned(),
        kind,
        scope,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::time::Duration;

    fn choice(id: &str, label: &str, kind: ChoiceKind, scope: ChoiceScope) -> Choice {
        Choice {
            id: id.to_owned(),
            label: label.to_owned(),
            kind,
            scope,
        }
    }

    #[test]
    fn a_claude_code_permission_request_reads_as_a_command_with_its_options_and_a_deadline_below_the_hook()
     {
        // The payload Claude Code's `PermissionRequest` hook is handed (hooks reference).
        let payload = json!({
            "session_id": "abc",
            "hook_event_name": "PermissionRequest",
            "tool_name": "Bash",
            "tool_input": { "command": "npm test", "description": "Run the tests" },
            "permission_suggestions": [
                { "type": "addRules", "behavior": "allow", "destination": "session",
                  "rules": [{ "toolName": "Bash", "ruleContent": "npm test" }] },
                { "type": "addRules", "behavior": "allow", "destination": "localSettings",
                  "rules": [{ "toolName": "Bash", "ruleContent": "npm:*" }] }
            ]
        });

        let ask = claude_permission_request(&payload, Duration::from_secs(60));

        assert_eq!(
            ask.action,
            Action::Command {
                line: "npm test".to_owned()
            }
        );
        assert_eq!(
            ask.options,
            [
                choice("allow", "Allow", ChoiceKind::Allow, ChoiceScope::Once),
                choice(
                    "suggestion:0",
                    "Allow Bash(npm test) for this session",
                    ChoiceKind::Allow,
                    ChoiceScope::Session
                ),
                choice(
                    "suggestion:1",
                    "Allow Bash(npm:*) from now on (localSettings)",
                    ChoiceKind::Allow,
                    ChoiceScope::Always
                ),
                choice("deny", "Deny", ChoiceKind::Reject, ChoiceScope::Once),
            ]
        );
        assert_eq!(ask.deadline, Deadline::Within(58_000));
        assert_eq!(ask.channel, Channel::Hook);
        assert_eq!(ask.summary.as_str(), "Run a command: npm test");
        assert!(!ask.elicits_secret);
    }
    #[test]
    fn a_codex_permission_request_hook_reads_as_allow_or_deny_this_once() {
        let payload = json!({
            "session_id": "019a", "hook_event_name": "PermissionRequest",
            "tool_name": "Bash", "tool_input": { "command": "cargo build" }
        });

        let ask = codex_permission_request(&payload, Duration::from_secs(30));

        assert_eq!(
            ask.action,
            Action::Command {
                line: "cargo build".to_owned()
            }
        );
        assert_eq!(
            ask.options,
            [
                choice("allow", "Allow", ChoiceKind::Allow, ChoiceScope::Once),
                choice("deny", "Deny", ChoiceKind::Reject, ChoiceScope::Once),
            ]
        );
        assert_eq!(ask.deadline, Deadline::Within(28_000));
        assert_eq!(ask.channel, Channel::Hook);
    }

    #[test]
    fn a_codex_app_server_approval_keeps_its_four_decisions_and_waits_without_a_deadline() {
        let params = json!({
            "threadId": "t1", "turnId": "u1", "itemId": "i1",
            "command": "rm -rf build", "cwd": "/w", "reason": "clean the build"
        });

        let ask =
            codex_app_server("item/commandExecution/requestApproval", &params).expect("an ask");

        assert_eq!(
            ask.action,
            Action::Command {
                line: "rm -rf build".to_owned()
            }
        );
        assert_eq!(
            ask.options,
            [
                choice("accept", "Accept", ChoiceKind::Allow, ChoiceScope::Once),
                choice(
                    "acceptForSession",
                    "Accept for this session",
                    ChoiceKind::Allow,
                    ChoiceScope::Session
                ),
                choice("decline", "Decline", ChoiceKind::Reject, ChoiceScope::Once),
                choice(
                    "cancel",
                    "Cancel the turn",
                    ChoiceKind::Cancel,
                    ChoiceScope::Once
                ),
            ]
        );
        assert_eq!(ask.deadline, Deadline::None);
        assert_eq!(
            ask.channel,
            Channel::CodexAppServer {
                thread: "t1".into(),
                turn: "u1".into(),
                item: "i1".into()
            }
        );
    }

    #[test]
    fn a_codex_app_server_grant_root_approval_reads_as_a_grant_of_the_whole_root() {
        let params =
            json!({ "threadId": "t1", "turnId": "u1", "itemId": "i2", "grantRoot": "/w/src" });

        let ask = codex_app_server("item/fileChange/requestApproval", &params).expect("an ask");

        assert_eq!(
            ask.action,
            Action::Edit {
                path: "/w/src".to_owned()
            }
        );
        assert_eq!(ask.summary.as_str(), "Allow writes anywhere under /w/src");
    }

    #[test]
    fn a_codex_app_server_file_change_without_a_root_reads_as_the_patch_it_applies() {
        let params = json!({ "threadId": "t1", "turnId": "u1", "itemId": "i3",
                             "reason": "fix the typo" });

        let ask = codex_app_server("item/fileChange/requestApproval", &params).expect("an ask");

        assert_eq!(ask.summary.as_str(), "Use apply_patch: \"fix the typo\"");
    }
    #[test]
    fn an_opencode_permission_request_reads_as_once_always_or_reject_on_its_own_server() {
        // opencode's `permission.asked` event properties.
        let props = json!({
            "id": "per_1", "sessionID": "ses_1", "permission": "bash",
            "patterns": ["git push origin main"], "metadata": {}, "always": ["git push *"]
        });

        let ask = opencode_permission(&props);

        assert_eq!(
            ask.action,
            Action::Command {
                line: "git push origin main".to_owned()
            }
        );
        assert_eq!(
            ask.options,
            [
                choice("once", "Allow once", ChoiceKind::Allow, ChoiceScope::Once),
                choice(
                    "always",
                    "Always allow git push *",
                    ChoiceKind::Allow,
                    ChoiceScope::Session
                ),
                choice("reject", "Reject", ChoiceKind::Reject, ChoiceScope::Once),
            ]
        );
        assert_eq!(ask.deadline, Deadline::None);
        assert_eq!(
            ask.channel,
            Channel::Opencode {
                session: "ses_1".into(),
                request: "per_1".into()
            }
        );
    }

    #[test]
    fn opencodes_always_option_names_every_pattern_it_approves_and_only_what_it_can_read() {
        let label = |always: Value| {
            let props = json!({ "id": "per_1", "sessionID": "ses_1", "permission": "bash",
                                "patterns": ["git push origin main"], "always": always });
            opencode_permission(&props).options[1].label.clone()
        };

        assert_eq!(
            label(json!(["git push *", "git fetch *"])),
            "Always allow git push * and git fetch *"
        );
        // Nothing readable to name: the option says no more than it knows.
        assert_eq!(label(Value::Null), "Always allow");
        assert_eq!(label(json!([])), "Always allow");
        assert_eq!(label(json!(["git push *", 7])), "Always allow");
        // A pattern a window would draw other than it is: not named at all.
        assert_eq!(
            label(json!(["git push *", "rm -rf \u{202e}* hsup tig"])),
            "Always allow"
        );
        assert_eq!(label(json!(["git\u{200b} push *"])), "Always allow");
        // As long as a summary, and no longer.
        let long = label(json!([format!("echo {}", "a".repeat(400))]));
        assert!(long.starts_with("Always allow echo a"), "{long}");
        assert!(long.ends_with('…'), "{long}");
        assert_eq!(
            long.chars().count(),
            crate::harness::model::SUMMARY_WIDTH,
            "{long}"
        );
    }

    #[test]
    fn an_acp_permission_request_keeps_the_agents_options_in_its_words_and_has_no_deadline() {
        let params = json!({
            "sessionId": "s1",
            "toolCall": { "toolCallId": "call_1", "title": "Delete old logs", "kind": "delete",
                          "locations": [{ "path": "/w/logs" }] },
            "options": [
                { "optionId": "a1", "name": "Allow once", "kind": "allow_once" },
                { "optionId": "a2", "name": "Allow always", "kind": "allow_always" },
                { "optionId": "r1", "name": "Reject", "kind": "reject_once" },
                { "optionId": "r2", "name": "Never", "kind": "reject_always" }
            ]
        });

        let ask = acp_request_permission(&params);

        assert_eq!(
            ask.action,
            Action::Edit {
                path: "/w/logs".to_owned()
            }
        );
        assert_eq!(
            ask.options,
            [
                choice("a1", "Allow once", ChoiceKind::Allow, ChoiceScope::Once),
                choice("a2", "Allow always", ChoiceKind::Allow, ChoiceScope::Always),
                choice("r1", "Reject", ChoiceKind::Reject, ChoiceScope::Once),
                choice("r2", "Never", ChoiceKind::Reject, ChoiceScope::Always),
            ]
        );
        assert_eq!(ask.risk, Risk::Delete, "the agent said it deletes");
        assert_eq!(ask.deadline, Deadline::None, "ADR 0080 §5, V28d");
        assert_eq!(
            ask.channel,
            Channel::Acp {
                session: "s1".into(),
                tool_call: "call_1".into()
            }
        );
        assert_eq!(ask.summary.as_str(), "Delete old logs");
    }
    #[test]
    fn asks_from_all_three_harnesses_and_acp_share_one_shape() {
        let hook = json!({ "tool_name": "Bash", "tool_input": { "command": "ls" } });
        let asks = [
            claude_permission_request(&hook, Duration::from_secs(60)),
            codex_permission_request(&hook, Duration::from_secs(60)),
            codex_app_server(
                "item/commandExecution/requestApproval",
                &json!({ "command": "ls" }),
            )
            .expect("an ask"),
            opencode_permission(&json!({ "permission": "bash", "patterns": ["ls"] })),
            acp_request_permission(
                &json!({ "toolCall": { "kind": "execute", "rawInput": { "command": ["ls"] } } }),
            ),
            Ask::default(),
        ];
        let keys = |ask: &Ask| -> Vec<String> {
            let value = serde_json::to_value(ask).expect("an ask serialises");
            value
                .as_object()
                .expect("an object")
                .keys()
                .cloned()
                .collect()
        };
        let shape = keys(&asks[0]);
        assert_eq!(
            shape,
            [
                "action",
                "options",
                "may_answer",
                "deadline",
                "risk",
                "summary",
                "elicits_secret",
                "channel"
            ]
        );
        for ask in &asks {
            assert_eq!(keys(ask), shape);
            assert_eq!(
                serde_json::from_value::<Ask>(serde_json::to_value(ask).unwrap()).unwrap(),
                *ask
            );
        }
        for ask in &asks[..5] {
            assert_eq!(
                ask.action,
                Action::Command {
                    line: "ls".to_owned()
                }
            );
        }
    }

    #[test]
    fn a_credential_in_what_is_asked_never_reaches_the_summary() {
        let token = "ghp_0123456789abcdefghijABCDEFGHIJ012345";
        let hook = json!({ "tool_name": "Bash", "tool_input": { "command": format!("curl -H 'Authorization: token {token}' x") } });

        let ask = claude_permission_request(&hook, Duration::from_secs(60));

        assert!(
            !ask.summary.as_str().contains(token),
            "{}",
            ask.summary.as_str()
        );
        assert!(ask.summary.as_str().starts_with("Run a command: curl"));
        // Read back off the wire, a summary is masked again: no path makes an unmasked one.
        let forged: Summary = serde_json::from_value(json!(format!("say {token}"))).unwrap();
        assert!(!forged.as_str().contains(token), "{}", forged.as_str());
    }

    #[test]
    fn a_credential_spelled_through_its_escapes_never_reaches_the_summary() {
        let token = crate::secretshape::escaped::token();
        let tail = &token[1..];
        for said in [
            format!("echo \"\\u0067{tail}\""),
            format!("echo \"x\\n{token}\""),
        ] {
            let summary = Summary::of(&said);
            assert!(
                !summary.as_str().contains(&tail[4..]),
                "{}",
                summary.as_str()
            );
            assert!(
                summary.as_str().starts_with("echo \""),
                "{}",
                summary.as_str()
            );
        }
    }

    #[test]
    fn a_summary_is_one_line_and_cut_to_its_width() {
        let summary = Summary::of(&format!("one\ntwo {}", "x".repeat(400)));

        assert!(summary.as_str().starts_with("one two x"));
        assert_eq!(
            summary.as_str().chars().count(),
            crate::harness::model::SUMMARY_WIDTH
        );
        assert!(summary.as_str().ends_with('…'));
    }

    #[test]
    fn a_summary_keeps_a_space_the_one_rule_hides_where_it_stood() {
        // SK-51 (#3202): a no-break, an en or an ideographic space is not a space a reader can
        // tell from one, so the line keeps it for the window to mark, never a plain space in
        // its place. Only the layout's own white space is collapsed.
        let summary = Summary::of("git\u{00A0}commit \u{2002}-m\u{3000}x\n\t\u{2028}\u{0B} y");

        assert_eq!(
            summary.as_str(),
            "git\u{00A0}commit \u{2002}-m\u{3000}x \u{2028}\u{0B} y"
        );
    }

    #[test]
    fn a_summary_keeps_a_line_break_a_command_line_is_not_laid_out_with() {
        // #3470: a shell lays a command out with spaces, tabs and line ends. A next line, a
        // vertical tab, a form feed or a line or paragraph separator is a character the command
        // holds, which a reader takes for a break: kept for the window to mark and say why.
        for c in ['\u{0B}', '\u{0C}', '\u{85}', '\u{2028}', '\u{2029}'] {
            let line = format!("ls{c}rm -rf x");

            assert_eq!(Summary::of(&line).as_str(), line, "U+{:04X}", u32::from(c));
        }
        assert_eq!(Summary::of("ls\r\n\t rm").as_str(), "ls rm");
    }

    #[test]
    fn a_hook_that_leaves_no_room_below_its_timeout_is_answered_in_the_pane() {
        let hook = json!({ "tool_name": "Bash", "tool_input": { "command": "ls" } });

        let ask = claude_permission_request(&hook, Duration::from_secs(2));

        assert_eq!(ask.deadline, Deadline::None);
        assert_eq!(ask.channel, Channel::Pane);
        assert!(ask.options.is_empty());
        assert_eq!(
            Deadline::below_hook_timeout(Duration::from_millis(2_500)),
            Some(Deadline::Within(500))
        );
    }
    /// The options a Claude Code `PermissionRequest` offers beside allow and deny, for one
    /// `permission_suggestions` entry.
    fn suggested(suggestion: serde_json::Value) -> Vec<Choice> {
        let payload = json!({
            "tool_name": "Bash", "tool_input": { "command": "ls" },
            "permission_suggestions": [suggestion]
        });
        let mut options = claude_permission_request(&payload, Duration::from_secs(60)).options;
        assert_eq!(options.remove(0).id, "allow");
        assert_eq!(options.pop().map(|deny| deny.id), Some("deny".to_owned()));
        options
    }

    #[test]
    fn a_claude_suggestion_to_deny_from_now_on_reads_as_a_reject() {
        let options = suggested(
            json!({ "type": "addRules", "behavior": "deny", "destination": "projectSettings",
            "rules": [{ "toolName": "Bash", "ruleContent": "rm:*" }] }),
        );

        assert_eq!(
            options,
            [choice(
                "suggestion:0",
                "Deny Bash(rm:*) from now on (projectSettings)",
                ChoiceKind::Reject,
                ChoiceScope::Always
            )]
        );
    }

    #[test]
    fn a_claude_suggestion_to_switch_mode_says_the_mode_it_switches_to() {
        let options = suggested(
            json!({ "type": "setMode", "mode": "bypassPermissions", "destination": "session" }),
        );

        assert_eq!(
            options,
            [choice(
                "suggestion:0",
                "Allow, and switch to bypassPermissions mode for this session",
                ChoiceKind::Allow,
                ChoiceScope::Session
            )]
        );
    }

    #[test]
    fn a_claude_suggestion_to_add_directories_names_them() {
        let options = suggested(
            json!({ "type": "addDirectories", "directories": ["/", "/tmp"], "destination": "localSettings" }),
        );

        assert_eq!(
            options,
            [choice(
                "suggestion:0",
                "Allow, and give access to / and /tmp from now on (localSettings)",
                ChoiceKind::Allow,
                ChoiceScope::Always
            )]
        );
    }

    #[test]
    fn a_claude_suggestion_that_rewrites_or_removes_rules_or_asks_or_is_unknown_is_not_offered() {
        // Removing a deny rule loosens what the agent may do, and "ask" is no answer at all:
        // neither may read as an option the operator approves on. Nor may a kind charter has
        // not seen.
        for suggestion in [
            json!({ "type": "removeRules", "behavior": "deny", "destination": "localSettings",
                    "rules": [{ "toolName": "Bash", "ruleContent": "rm:*" }] }),
            json!({ "type": "replaceRules", "behavior": "allow", "destination": "localSettings",
                    "rules": [{ "toolName": "Bash" }] }),
            json!({ "type": "removeDirectories", "directories": ["/w"], "destination": "session" }),
            json!({ "type": "addRules", "behavior": "ask", "destination": "session",
                    "rules": [{ "toolName": "Bash" }] }),
            json!({ "type": "addRules", "behavior": "sometimes", "destination": "session",
                    "rules": [{ "toolName": "Bash" }] }),
            json!({ "type": "addRules", "behavior": "allow", "destination": "session", "rules": [] }),
            json!({ "type": "grantEverything", "destination": "session" }),
            json!({ "behavior": "allow", "destination": "session", "rules": [{ "toolName": "Bash" }] }),
        ] {
            assert_eq!(suggested(suggestion.clone()), [], "{suggestion}");
        }
    }

    #[test]
    fn a_codex_app_server_request_of_a_method_charter_does_not_read_is_no_ask() {
        // v1's `execCommandApproval` answers in other words than the v2 decisions read here.
        assert_eq!(
            codex_app_server("execCommandApproval", &json!({ "command": "ls" })),
            None
        );
        assert_eq!(
            codex_app_server("item/tool/requestUserInput", &json!({})),
            None
        );
    }
    #[test]
    fn two_credentials_side_by_side_are_both_masked() {
        let line = "AKIAIOSFODNN7EXAMPLE ASIAIOSFODNN7EXAMPLE and ann.smith@acme-corp.io";

        let summary = Summary::of(line);

        assert!(
            !summary.as_str().contains("IOSFODNN7"),
            "{}",
            summary.as_str()
        );
        assert!(
            !summary.as_str().contains("acme-corp"),
            "{}",
            summary.as_str()
        );
        assert!(summary.as_str().contains(" and "), "{}", summary.as_str());
    }

    #[test]
    fn a_channel_with_an_empty_request_id_has_no_request_id() {
        let acp = |session: &str, call: &str| Channel::Acp {
            session: session.into(),
            tool_call: call.into(),
        };

        assert_eq!(acp("s1", "c1").request(), Some(("s1", "c1")));
        assert_eq!(acp("s1", "").request(), None);
        assert_eq!(acp("", "c1").request(), None);
        assert_eq!(
            Channel::Opencode {
                session: "s".into(),
                request: String::new()
            }
            .request(),
            None
        );
        assert_eq!(
            Channel::CodexAppServer {
                thread: "t".into(),
                turn: "u".into(),
                item: String::new()
            }
            .request(),
            None
        );
    }

    /// A shell line as Claude Code's hook asks about it.
    fn bash(line: &str) -> Ask {
        claude_permission_request(
            &json!({ "tool_name": "Bash", "tool_input": { "command": line } }),
            Duration::from_secs(60),
        )
    }

    #[test]
    fn a_delete_and_a_secret_use_read_as_risky_and_an_ordinary_command_as_normal() {
        let classed = [
            // Ordinary commands, and words that only look like the risky ones.
            ("ls -la", Risk::Normal),
            ("cargo test -p purlis-core", Risk::Normal),
            ("git status && git log --oneline -5", Risk::Normal),
            ("git push origin main", Risk::Normal),
            ("git branch -a", Risk::Normal),
            ("echo rm is a word", Risk::Normal),
            ("purlis secret list ops", Risk::Normal),
            ("grep -rn delete src", Risk::Normal),
            // Deletes.
            ("rm -rf build", Risk::Delete),
            ("cd /w && rm notes.txt", Risk::Delete),
            ("sudo rm -r /var/tmp/x", Risk::Delete),
            ("FOO=1 rmdir empty", Risk::Delete),
            ("find . -name '*.log' -delete", Risk::Delete),
            ("find . -name '*.log' -exec rm {} +", Risk::Delete),
            ("ls | xargs rm", Risk::Delete),
            ("git branch -D topic", Risk::Delete),
            ("git branch --delete topic", Risk::Delete),
            ("git -C repo tag -d v1", Risk::Delete),
            ("git push origin --delete topic", Risk::Delete),
            ("git push origin :topic", Risk::Delete),
            ("git push --mirror backup", Risk::Delete),
            ("git clean -fdx", Risk::Delete),
            ("git rm -r old", Risk::Delete),
            ("git stash drop", Risk::Delete),
            ("git worktree remove ../w2", Risk::Delete),
            ("gh repo delete acme/site --yes", Risk::Delete),
            ("kubectl delete pod web-1", Risk::Delete),
            ("kubectl -n prod delete pod web-1", Risk::Delete),
            ("git push :topic", Risk::Delete),
            ("docker rm -f web", Risk::Delete),
            ("sh -c 'rm -rf out'", Risk::Delete),
            // Secret uses.
            ("purlis secret get ops API_TOKEN", Risk::SecretUse),
            ("charter secret exec ops -- deploy", Risk::SecretUse),
            ("purlis persona secret cp ops TOKEN", Risk::SecretUse),
            ("/usr/local/bin/purlis secret get ops A", Risk::SecretUse),
            ("op read op://Eng/deploy/token", Risk::SecretUse),
            ("op item get deploy --fields password", Risk::SecretUse),
            ("security find-generic-password -s svc -w", Risk::SecretUse),
            ("gh auth token", Risk::SecretUse),
            ("vault kv get secret/app", Risk::SecretUse),
            (
                "aws secretsmanager get-secret-value --secret-id app",
                Risk::SecretUse,
            ),
            (
                "gcloud secrets versions access latest --secret app",
                Risk::SecretUse,
            ),
            ("kubectl get secret db -o yaml", Risk::SecretUse),
            ("kubectl -n prod get secrets", Risk::SecretUse),
            ("kubectl get pods", Risk::Normal),
            // An option's value is never the subcommand (#1046).
            ("aws --region eu-west-1 s3 rm s3://b/k", Risk::Delete),
            ("aws --profile ops --region x s3 rb s3://b", Risk::Delete),
            (
                "aws --output json ec2 delete-volume --volume-id v",
                Risk::Delete,
            ),
            (
                "aws --region x secretsmanager get-secret-value --secret-id app",
                Risk::SecretUse,
            ),
            ("aws --region us-east-1 s3 ls", Risk::Normal),
            ("aws --region=eu-west-1 s3 rm s3://b/k", Risk::Delete),
            (
                "gcloud --project p secrets versions access latest --secret app",
                Risk::SecretUse,
            ),
            (
                "gcloud --project p --format json compute instances delete vm",
                Risk::Delete,
            ),
            ("gcloud --project p compute instances list", Risk::Normal),
            (
                "az --subscription s keyvault secret show --name n",
                Risk::SecretUse,
            ),
            ("az -o json group delete --name rg", Risk::Delete),
            ("az --subscription s vm list", Risk::Normal),
            (
                "kubectl --context prod --namespace web --as admin delete pod web-1",
                Risk::Delete,
            ),
            (
                "kubectl --context prod -n web get secret db",
                Risk::SecretUse,
            ),
            // Read with every value as a word too, so a line never asks less for it.
            ("kubectl --context delete get pods", Risk::Delete),
            ("bash -c 'op read op://a/b/c > /tmp/x'", Risk::SecretUse),
            // Both: the secret use is the graver.
            ("purlis secret get ops A; rm -rf build", Risk::SecretUse),
        ];
        for (line, risk) in classed {
            assert_eq!(bash(line).risk, risk, "{line}");
        }
    }

    #[test]
    fn every_source_classifies_the_line_it_asks_about() {
        let line = "rm -rf build";
        let hook = json!({ "tool_name": "Bash", "tool_input": { "command": line } });
        let opencode = json!({ "permission": "bash", "patterns": [line] });
        let asks = [
            claude_permission_request(&hook, Duration::from_secs(60)),
            codex_permission_request(&hook, Duration::from_secs(60)),
            // No room below the hook's timeout: the pane's ask is classed all the same.
            codex_permission_request(&hook, Duration::ZERO),
            codex_app_server(
                "item/commandExecution/requestApproval",
                &json!({ "command": line }),
            )
            .expect("an ask"),
            opencode_permission(&opencode),
            opencode_permission_hooked(&opencode, Duration::from_secs(60)),
            acp_request_permission(
                &json!({ "toolCall": { "kind": "execute", "rawInput": { "command": ["rm", "-rf", "build"] } } }),
            ),
        ];
        for ask in asks {
            assert_eq!(ask.risk, Risk::Delete, "{ask:?}");
        }
    }

    #[test]
    fn a_tool_whose_name_says_it_deletes_or_reads_a_secret_is_risky() {
        let tool = |name: &str| {
            claude_permission_request(
                &json!({ "tool_name": name, "tool_input": { "id": 1 } }),
                Duration::from_secs(60),
            )
            .risk
        };
        assert_eq!(tool("mcp__github__delete_file"), Risk::Delete);
        assert_eq!(tool("mcp__store__removeItem"), Risk::Delete);
        assert_eq!(tool("mcp__vault__get_secret"), Risk::SecretUse);
        assert_eq!(tool("mcp__idp__read-password"), Risk::SecretUse);
        assert_eq!(tool("Read"), Risk::Normal);
        assert_eq!(tool("WebFetch"), Risk::Normal);
        assert_eq!(tool("mcp__github__list_issues"), Risk::Normal);
        // A file edit is classed by the source alone.
        let edit = claude_permission_request(
            &json!({ "tool_name": "Edit", "tool_input": { "file_path": "/w/rm" } }),
            Duration::from_secs(60),
        );
        assert_eq!(edit.risk, Risk::Normal);
    }

    #[test]
    fn a_class_the_source_gave_is_never_lowered_by_what_the_line_reads_as() {
        let risky = [
            Risk::ProtectedPush,
            Risk::Delete,
            Risk::SecretUse,
            Risk::ProductionHost,
        ];
        for given in risky {
            for found in [Risk::Normal, Risk::Delete, Risk::SecretUse] {
                // Only a graver class found replaces the source's: a secret use over a delete.
                let expected = if (given, found) == (Risk::Delete, Risk::SecretUse) {
                    Risk::SecretUse
                } else {
                    given
                };
                assert_eq!(more_cautious(given, found), expected, "{given:?} {found:?}");
            }
        }
        for found in [Risk::Normal, Risk::Delete, Risk::SecretUse] {
            assert_eq!(more_cautious(Risk::Normal, found), found);
        }
        // ACP's own delete stays one when its line reads as ordinary.
        let ask = acp_request_permission(&json!({
            "toolCall": { "kind": "delete", "rawInput": { "command": "ls" },
                          "locations": [{ "path": "/w/a" }] }
        }));
        assert_eq!(ask.risk, Risk::Delete);
    }

    #[test]
    fn a_line_nested_deep_or_unreadable_is_classed_without_a_panic() {
        // Four shells deep: read as far as the depth goes.
        let deep = "sh -c \"sh -c 'sh -c \\\"sh -c rm\\\"'\"";
        let _ = bash(deep).risk;
        assert_eq!(bash("").risk, Risk::Normal);
        assert_eq!(bash("'unclosed").risk, Risk::Normal);
    }
}
