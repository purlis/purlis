// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The fallback transport: the forge's own CLI, `gh api` or `glab api`, logged in as whoever
//! logged that CLI in (ADR 0070 §5).
//!
//! It turns a [`Call`] into the argv charter has always sent, byte for byte: the recorded
//! Python behaviour (ADR 0046) and `a_forge_cli_is_asked_exactly_what_python_asked.rs` pin it.
//! The CLI is found and started as [`super`]'s module docs say, with an emptied environment
//! that keeps only the CLI's credential variables.

use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use super::transport::{Call, Endpoint, Field, NoAnswer, Reply, Transport};
use super::{Forge, ForgeError, Kind, STATUS_TIMEOUT, TOKEN_ENV, cli_env_keeping, find_cli};
use crate::netlog::{self, Feature, Via};
use crate::worktree::git;

/// The CLI transport. `Cli::default()` passes the CLI every credential variable it would log
/// in with; [`Cli::as_the_operator`] withholds the token ones.
#[derive(Debug, Clone, Copy, Default)]
pub struct Cli {
    withhold_tokens: bool,
}

impl Cli {
    /// The CLI logged in as the operator's own stored login, and never as a token in this
    /// process's environment: it is given none of [`TOKEN_ENV`].
    pub fn as_the_operator() -> Cli {
        Cli {
            withhold_tokens: true,
        }
    }

    /// Run the CLI with `args`, under `timeout`, listing it in the network log as a forge
    /// call to the host `--hostname` names (OB-15).
    pub fn run(&self, kind: Kind, args: &[String], timeout: Duration) -> Result<Reply, NoAnswer> {
        let host = hostname(args).unwrap_or_else(|| kind.default_host().to_string());
        let path = subcommand(args);
        self.run_listed(
            kind,
            args,
            timeout,
            Listed {
                feature: Feature::Forge,
                host: &host,
                method: "CLI",
                path: &path,
            },
        )
    }

    /// Run the CLI with `args`, under `timeout`, listing it in the network log as `listed`.
    pub(crate) fn run_listed(
        &self,
        kind: Kind,
        args: &[String],
        timeout: Duration,
        listed: Listed<'_>,
    ) -> Result<Reply, NoAnswer> {
        let withhold = self.withhold_tokens;
        let started = Instant::now();
        let answer = call_with(kind, args, timeout, |name| {
            !(withhold && TOKEN_ENV.contains(&name))
        });
        // A CLI that could not be found or started made no call; one that ran or timed out did.
        if !matches!(answer, Err(NoAnswer::Missing(_))) {
            netlog::record(netlog::Call {
                feature: listed.feature,
                via: match kind {
                    Kind::GitHub => Via::Gh,
                    Kind::GitLab => Via::Glab,
                },
                host: listed.host,
                method: listed.method,
                path: listed.path,
                status: listed_status(&answer),
                answered: answer.is_ok(),
                took: started.elapsed(),
            });
        }
        answer
    }
}

/// The HTTP status a CLI call is listed with: the one the CLI said the forge answered, for a
/// call that failed (#990). `gh api` and `glab api` say it only then, and a call that
/// succeeded is listed with none rather than a guess. The reply the backend reads is not
/// touched: this is the network log's alone.
fn listed_status(answer: &Result<Reply, NoAnswer>) -> Option<u16> {
    match answer {
        Ok(reply) if !reply.ok() => status_said(&reply.err),
        _ => None,
    }
}

/// The status `gh` or `glab` said on its standard error: `gh: Not Found (HTTP 404)`, `glab:
/// 404 Group Not Found (HTTP 404)`. The CLI puts it last on the line that says why, after the
/// forge's own message, so only a line that ends with it counts, and the last such line; a
/// number HTTP has no status for is none.
fn status_said(err: &str) -> Option<u16> {
    err.lines().rev().find_map(|line| {
        let digits = line.trim_end().strip_suffix(')')?.rsplit_once("(HTTP ")?.1;
        if digits.len() != 3 || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
            return None;
        }
        digits
            .parse::<u16>()
            .ok()
            .filter(|status| (100..=599).contains(status))
    })
}

/// What the network log is told about a CLI call.
pub(crate) struct Listed<'a> {
    pub feature: Feature,
    pub host: &'a str,
    pub method: &'a str,
    pub path: &'a str,
}

/// The value of `--hostname` in `args`.
fn hostname(args: &[String]) -> Option<String> {
    args.iter().enumerate().find_map(|(at, arg)| {
        if arg == "--hostname" {
            args.get(at + 1).cloned()
        } else {
            arg.strip_prefix("--hostname=").map(str::to_string)
        }
    })
}

/// The CLI's own subcommand in `args`, such as `auth/status`: its first two words, flags and
/// their values skipped. Never an argument's value.
fn subcommand(args: &[String]) -> String {
    let mut words = Vec::new();
    let mut skip = false;
    for arg in args {
        if skip {
            skip = false;
        } else if arg == "--hostname" {
            skip = true;
        } else if arg.starts_with("--hostname=") {
            // The host and its value in one word: nothing more to skip.
        } else if arg.starts_with('-') {
            break;
        } else {
            words.push(arg.as_str());
            if words.len() == 2 {
                break;
            }
        }
    }
    words.join("/")
}

/// The argv `call` is sent as.
pub fn argv(forge: &Forge, call: &Call) -> Vec<String> {
    let host = forge.host.as_str();
    let mut args = match (&call.endpoint, forge.kind) {
        (Endpoint::Graphql, _) => strings(&["api", "graphql", "--hostname", host]),
        (Endpoint::Rest { .. }, Kind::GitHub) => strings(&["api", "--hostname", host]),
        (Endpoint::Rest { .. }, Kind::GitLab) => strings(&["--hostname", host, "api"]),
    };
    if let Endpoint::Rest { method, path } = &call.endpoint {
        if let Some(method) = method {
            args.extend(strings(&["-X", method.word()]));
        }
        args.push(path.clone());
    }
    for field in &call.fields {
        let (flag, name, value) = match field {
            Field::Text(name, value) => ("-f", name, value),
            Field::Typed(name, value) => ("-F", name, value),
            // Refused before `argv` is built ([`Transport::send`]); never sent as a path.
            Field::File(..) => continue,
        };
        args.push(flag.to_string());
        args.push(format!("{name}={value}"));
    }
    args
}

impl Transport for Cli {
    fn send(&self, forge: &Forge, call: &Call) -> Result<Reply, NoAnswer> {
        if call.fields.iter().any(|f| matches!(f, Field::File(..))) {
            return Err(NoAnswer::Refused(FILE_REFUSED.to_string()));
        }
        let method = match &call.endpoint {
            Endpoint::Graphql => "POST",
            Endpoint::Rest { method: None, .. } => "GET",
            Endpoint::Rest {
                method: Some(method),
                ..
            } => method.word(),
        };
        self.run_listed(
            forge.kind,
            &argv(forge, call),
            call.timeout,
            Listed {
                feature: Feature::Forge,
                host: &forge.host,
                method,
                path: call.path(),
            },
        )
    }

    fn check_auth(&self, forge: &Forge) -> Result<(), ForgeError> {
        let cli = forge.kind.cli();
        let host = forge.host.as_str();
        let args = match forge.kind {
            Kind::GitHub => strings(&["auth", "status", "--hostname", host]),
            Kind::GitLab => strings(&["--hostname", host, "auth", "status"]),
        };
        let answer = match self.run(forge.kind, &args, STATUS_TIMEOUT) {
            Ok(answer) => answer,
            Err(NoAnswer::Timeout(why)) => {
                return Err(ForgeError::transport(format!(
                    "{cli} did not answer for {host}: {why}"
                )));
            }
            Err(
                NoAnswer::Missing(why)
                | NoAnswer::Refused(why)
                | NoAnswer::HeldBack { said: why, .. },
            ) => {
                return Err(ForgeError::transport(why));
            }
        };
        let logged_in = match forge.kind {
            Kind::GitHub => answer.ok(),
            // glab exits 0 while logged in to nothing, so its own words are asked too.
            Kind::GitLab => answer.ok() && answer.both().contains("Logged in"),
        };
        if logged_in {
            Ok(())
        } else {
            Err(ForgeError::new(format!(
                "{cli} is not authenticated for {host}. Run: {cli} auth login"
            )))
        }
    }
}

/// The token variables either CLI would sign in with in place of its stored login: `gh`'s
/// ([`TOKEN_ENV`]) and `glab`'s.
/// Why the CLI transport sends no call with a file (#2026): `gh api -F name=@path` and
/// `glab api` read a file from disk, and the only file purlis sends goes with its own sign-in.
pub const FILE_REFUSED: &str =
    "purlis sends a file only with its own sign-in, never through gh or glab";

const EVERY_TOKEN_ENV: [&str; 7] = [
    TOKEN_ENV[0],
    TOKEN_ENV[1],
    TOKEN_ENV[2],
    TOKEN_ENV[3],
    "GITLAB_TOKEN",
    "GITLAB_ACCESS_TOKEN",
    "OAUTH_TOKEN",
];

/// `kind`'s CLI asked for the token of its own stored login on `host`: `gh auth token
/// --hostname <host>`, or `glab config get token --host <host>`. For the import a person asked
/// for (ADR 0077 §5, [`super::cliimport`]) and nothing else.
///
/// It is given none of either CLI's token variables, so the answer is the login the CLI keeps,
/// never a token this process was started with. It sends nothing over the network, so it is not
/// listed in the network log. The answer's `out` is the token: the caller wipes it.
pub(crate) fn stored_token(kind: Kind, host: &str) -> Result<Reply, NoAnswer> {
    let args = match kind {
        Kind::GitHub => strings(&["auth", "token", "--hostname", host]),
        Kind::GitLab => strings(&["config", "get", "token", "--host", host]),
    };
    call_with(kind, &args, STATUS_TIMEOUT, |name| {
        !EVERY_TOKEN_ENV.contains(&name)
    })
}

/// Run `kind`'s CLI with `args`, passing on only the credential variables `keep` accepts.
fn call_with(
    kind: Kind,
    args: &[String],
    timeout: Duration,
    keep: impl Fn(&str) -> bool,
) -> Result<Reply, NoAnswer> {
    let cli = kind.cli();
    let Some(path) = find_cli(cli) else {
        return Err(NoAnswer::Missing(format!(
            "purlis could not find {cli} on PATH — install it and log in (`{cli} auth login`)"
        )));
    };
    run_found(cli, &path, args, timeout, keep)
}

/// [`call_with`], once the CLI has been found at `path`.
fn run_found(
    cli: &str,
    path: &std::path::Path,
    args: &[String],
    timeout: Duration,
    keep: impl Fn(&str) -> bool,
) -> Result<Reply, NoAnswer> {
    let mut cmd = Command::new(path);
    cmd.args(args)
        .env_clear()
        .envs(cli_env_keeping(path.parent(), keep))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    // The forge's CLI runs git and its own helpers: at the deadline they are ended with it.
    git::lead(&mut cmd);
    let child = crate::forklock::spawn(&mut cmd)
        .map_err(|e| NoAnswer::Missing(format!("purlis could not run {cli}: {e}")))?;
    let run = git::wait(child, timeout)
        .map_err(|e| NoAnswer::Missing(format!("purlis could not run {cli}: {e}")))?;
    match run.code {
        Some(code) => Ok(Reply::of(code, run.out, run.err)),
        None => Err(NoAnswer::Timeout(format!(
            "timed out after {}s: {cli} {}",
            timeout.as_secs(),
            args.join(" ")
        ))),
    }
}

fn strings(args: &[&str]) -> Vec<String> {
    args.iter().map(|s| (*s).to_string()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// In the host, where nothing but its own deadline ends what purlis starts: `gh` and
    /// `glab` run git and helpers of their own, and a deadline that ended the CLI alone would
    /// leave them. Each is its group's leader, which is what the deadline's stop ends.
    #[cfg(unix)]
    #[test]
    fn each_forge_cli_is_started_as_the_leader_of_a_process_group_of_its_own() {
        for cli in ["gh", "glab"] {
            let dir = tempfile::tempdir().expect("a directory");
            let said = dir.path().join("which-process-it-is");
            let over = dir.path().join("the-test-is-over");
            let program = stand_in::program(
                dir.path(),
                cli,
                &format!(
                    "#!/bin/sh\necho $$ > '{said}.part'\n/bin/mv '{said}.part' '{said}'\n\
                     while [ ! -e '{over}' ]; do /bin/sleep 0.1; done\n",
                    said = said.display(),
                    over = over.display()
                ),
            );
            let running = std::thread::spawn(move || {
                run_found(cli, &program, &[], Duration::from_secs(30), |_| false).is_ok()
            });
            let pid = stand_in::pid_named_in(&said, Duration::from_secs(10))
                .expect("the stand-in said which process it is");
            let _ends = stand_in::Ends::group(pid);
            let it = rustix::process::Pid::from_raw(pid.cast_signed()).expect("a pid");
            let group = rustix::process::getpgid(Some(it));

            std::fs::write(&over, "").expect("the stand-in is told to end");
            assert!(running.join().expect("the call returns"), "{cli} answered");
            assert_eq!(group.ok(), Some(it), "{cli} did not lead its own group");
            assert_ne!(
                Some(rustix::process::getpgrp()),
                group.ok(),
                "{cli} ran in this process's group"
            );
        }
    }

    fn args(words: &[&str]) -> Vec<String> {
        strings(words)
    }

    #[test]
    fn a_cli_call_is_listed_by_its_subcommand_and_never_by_a_value() {
        let auth = args(&["auth", "status", "--hostname", "ghe.example.com"]);
        assert_eq!(
            (subcommand(&auth), hostname(&auth).as_deref()),
            ("auth/status".to_string(), Some("ghe.example.com"))
        );
        let glab = args(&["--hostname", "gl.example.com", "api", "projects/a%2Fb"]);
        assert_eq!(subcommand(&glab), "api/projects/a%2Fb");
        let filed = args(&[
            "issue",
            "create",
            "--repo",
            "o/r",
            "--title",
            "a secret title",
        ]);
        assert_eq!(subcommand(&filed), "issue/create");
        assert_eq!(hostname(&filed), None);
        let joined = args(&["--hostname=gl.example.com", "auth", "status"]);
        assert_eq!(
            (subcommand(&joined), hostname(&joined).as_deref()),
            ("auth/status".to_string(), Some("gl.example.com"))
        );
    }

    /// #990: a failed call through `gh` or `glab` is listed with the HTTP status the CLI says
    /// the forge answered, which both print last on the line that says why.
    #[test]
    fn a_failed_cli_call_is_listed_with_the_status_the_forge_answered() {
        assert_eq!(status_said("gh: Not Found (HTTP 404)\n"), Some(404));
        assert_eq!(
            status_said("glab: 404 Group Not Found (HTTP 404)"),
            Some(404)
        );
        assert_eq!(
            status_said("gh: Validation Failed (HTTP 422)\nPull request already exists\n"),
            Some(422)
        );
        assert_eq!(status_said("  gh: Server Error (HTTP 502)\n"), Some(502));
        // The forge's own message is what comes first, and never decides it: the CLI's
        // status is the one that ends the line.
        assert_eq!(
            status_said("gh: said (HTTP 200) here (HTTP 401)"),
            Some(401)
        );
        assert_eq!(status_said("gh: said (HTTP 200) here"), None);
        // No status the CLI said, or none HTTP has: none listed.
        assert_eq!(status_said("error connecting to api.github.com"), None);
        assert_eq!(status_said("gh: odd (HTTP 42)"), None);
        assert_eq!(status_said("gh: odd (HTTP 4040)"), None);
        assert_eq!(status_said("gh: odd (HTTP 999)"), None);
        assert_eq!(status_said(""), None);
    }

    #[test]
    fn only_a_failed_call_is_listed_with_a_status() {
        let failed = Reply::of(1, String::new(), "gh: Not Found (HTTP 404)".to_owned());
        assert_eq!(listed_status(&Ok(failed)), Some(404));
        // A call that succeeded printed no status: it is not guessed.
        let ok = Reply::of(0, "{}".to_owned(), "gh: (HTTP 404)".to_owned());
        assert_eq!(listed_status(&Ok(ok)), None);
        assert_eq!(
            listed_status(&Err(NoAnswer::Timeout("gh: (HTTP 404)".to_owned()))),
            None
        );
    }
}
