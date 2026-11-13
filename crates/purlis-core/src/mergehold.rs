// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The merge hold** (R-114, R-115, R-117, R-036; Q9, Q73, Q135): a chat never merges, the
//! app does, on the person's press.
//!
//! A hold is an action purlis blocks and turns into an ask. In a chat the app started, a
//! command the release floor reads as a merge ([`crate::floorguard::merge_reason`]) is refused
//! to the chat whatever its mode, and the request it names is handed to the app
//! ([`crate::hookwire::Ask::Merge`]), which holds one merge ask for the person ([`Holds`]). Only
//! the person's press merges, and the app makes the forge's merge itself ([`Holds::press`]),
//! only while the request's head is the commit the ask was raised at.
//!
//! - **The key is typed**: a repo, a request's number and its head commit ([`MergeKey`]), each
//!   checked as it is made. The command line only says which request; the head is the forge's.
//! - **There are no grants for merges**: nothing a press does lets the chat merge later, and
//!   the store of rules made from answers refuses one for a merge ask
//!   ([`crate::askrules::Store::make`]).
//! - **Only a human answers**: a press and a Don't merge each take an
//!   [`Answerer`](crate::harness::asks::Answerer), which is made only from a connection the host
//!   admitted as the window or `charter inbox`, never from a chat's own socket or token (V16a).
//!   No setting or key changes that.
//!
//! - **A merge's facts are the forge's** ([`read`]): only a repo the project's inventory lists
//!   is merged ([`listed_in`], R-118); the ask's title, author, base and checks are read from
//!   the forge, never from the chat, whose line carries none of them (R-119); and whether a
//!   ruleset protects the base is read with the app's own forge login (R-146).
//!
//! - **A press is bound to what the person was shown** (R-150, Q248): it says the ruleset state
//!   its row showed ([`Seen`]), and merges only while that is the state the ask holds, which a
//!   re-read just before the press refreshes ([`Holds::refresh`]).
//!
//! **Not here yet** (each its ticket's): A4's refusal of an unattended merge still answers first
//! (its precedence is read with attendance, R-145, #6409). A line whose request cannot be read (a
//! raw API call, a merge with no number) is refused with no ask.
//!
//! In memory only, in the app: a relaunch holds none, and the chat's next try asks again.

use std::sync::{Mutex, PoisonError};

use crate::harness::asks::{Answerer, HumanScope};

/// The source a merge ask is listed under, and the one a rule made from an answer may never
/// name ([`crate::askrules::Store::make`]).
pub const SOURCE: &str = "merge-hold";

/// The trace reason a held merge is refused with: an ask is raised for it, and the event log
/// records it as a hold, never as a block.
pub const REASON: &str = "merge-hold";

/// The trace reason a merge is refused with where its request cannot be read: nobody is asked,
/// and the event log records it as a block.
pub const REASON_UNREADABLE: &str = "merge-unreadable";

/// The trace reason a merge is refused with where its request was read but the app raised no
/// ask for it (Q239): it refused, could not read the forge, the chat has as many merge asks as
/// it may, or no app answered. The event log records it as a block.
pub const REASON_NOT_ASKED: &str = "merge-not-asked";

/// How long the app waits for the forge to say where a held request stands before it answers
/// the hook: past it, nobody is asked, and the chat is told so. Inside the hook's own wait.
pub const THE_FORGE_IS_READ_WITHIN: std::time::Duration = std::time::Duration::from_millis(1_200);

/// The most merge asks one chat has waiting at once: one more is not asked.
pub const AT_MOST_PER_CHAT: usize = 3;

/// **A value refused as part of a merge's key.** It names the part, never the value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("this is not a {0} a merge can name")]
pub struct Invalid(&'static str);

/// **A repo on a forge, by its path** (`owner/name`, or a GitLab group's `group/sub/name`): two
/// to eight parts of `A-Z a-z 0-9 . _ -`, none of them `.` or `..`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Repo(String);

/// **A base branch**, the one a request merges into: 1 to 255 bytes, no space or control
/// character, not starting with `-`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Base(String);

/// **A head commit**, as a forge names one: 40 or 64 lower-case hex digits.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct HeadSha(String);

macro_rules! checked {
    ($type:ident, $what:literal, $check:path) => {
        impl TryFrom<String> for $type {
            type Error = Invalid;
            fn try_from(value: String) -> Result<Self, Invalid> {
                if $check(&value) {
                    Ok(Self(value))
                } else {
                    Err(Invalid($what))
                }
            }
        }

        impl TryFrom<&str> for $type {
            type Error = Invalid;
            fn try_from(value: &str) -> Result<Self, Invalid> {
                Self::try_from(value.to_owned())
            }
        }

        impl From<$type> for String {
            fn from(value: $type) -> String {
                value.0
            }
        }

        impl $type {
            /// The value, as checked.
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }
    };
}

checked!(Repo, "repo", is_repo);
checked!(HeadSha, "head commit", is_head);
checked!(Base, "base branch", is_base);

fn is_base(value: &str) -> bool {
    (1..=255).contains(&value.len())
        && !value.starts_with('-')
        && !value.chars().any(|c| c.is_whitespace() || c.is_control())
}

fn is_repo(value: &str) -> bool {
    let parts: Vec<&str> = value.split('/').collect();
    value.len() <= 255
        && (2..=8).contains(&parts.len())
        && parts.iter().all(|part| {
            !part.is_empty()
                && *part != "."
                && *part != ".."
                && part
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
        })
}

fn is_head(value: &str) -> bool {
    matches!(value.len(), 40 | 64)
        && value
            .bytes()
            .all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

/// **What a merge ask is keyed on**: the repo, the request's number, and the head commit and
/// base branch the forge said the request was at when the ask was raised (Q239: the base is
/// pinned, so a request retargeted after the ask is not merged by it). Never a command line.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct MergeKey {
    pub repo: Repo,
    pub number: u64,
    pub head: HeadSha,
    pub base: Base,
}

impl MergeKey {
    /// Whether `self` and `other` are the same request, whatever its head and base.
    fn same_request(&self, other: &MergeKey) -> bool {
        self.repo == other.repo && self.number == other.number
    }
}

/// **The request a held command names**: its number, and its repo where the command names one
/// (`-R owner/name`). Read from the command's words, and only to say which request: the app
/// finds the repo in the project and reads the head from the forge.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Request {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repo: Option<Repo>,
    pub number: u64,
}

/// **A command the merge hold refuses.**
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Held {
    /// A merge of one request purlis can name: the person is asked to merge it.
    Request(Request),
    /// A merge purlis cannot name the request of (a raw API call, a merge with no number, two
    /// merges on one line): refused, and nobody is asked.
    Unreadable,
    /// `purlis change land`, which lands a change's requests: refused in a chat the app started,
    /// as the person's to do now (Q9, Q93), and nobody is asked.
    Land,
}

/// **Whether `cmd` is held**, and which request it merges where that can be read: `None` for a
/// command the release floor does not read as a merge.
pub fn held(cmd: &str) -> Option<Held> {
    crate::floorguard::merge_reason(cmd)?;
    let segments = crate::shellseg::segment_argv(cmd);
    if segments.iter().any(|toks| lands(toks)) {
        return Some(Held::Land);
    }
    let mut found = segments.iter().filter_map(|toks| merge_of(toks));
    Some(match (found.next(), found.next()) {
        (Some(Some(request)), None) => Held::Request(request),
        _ => Held::Unreadable,
    })
}

/// Whether one segment is `purlis change land` (or its older name, `charter change land`).
fn lands(toks: &[String]) -> bool {
    let (prog, _, argv) = crate::shellwrap::split_env(toks);
    let base = crate::shellwrap::base_lower(&prog);
    let mut words = argv
        .get(1..)
        .unwrap_or(&[])
        .iter()
        .filter(|arg| !arg.starts_with('-'));
    matches!(base.as_str(), "purlis" | "charter")
        && words.next().map(String::as_str) == Some("change")
        && words.next().map(String::as_str) == Some("land")
}

/// The flags of `gh pr merge` and `glab mr merge` that take a value, which is never the
/// request's number.
const VALUE_FLAGS: &[&str] = &[
    "-b",
    "--body",
    "-F",
    "--body-file",
    "-t",
    "--subject",
    "-A",
    "--author-email",
    "--match-head-commit",
    "-m",
    "--message",
    "--sha",
    "--squash-message",
];

/// One segment's merge: `None` where it is no `gh pr merge`, `glab mr merge` or `glab mr
/// accept`; `Some(None)` where it is one whose request cannot be read.
fn merge_of(toks: &[String]) -> Option<Option<Request>> {
    let (prog, _, argv) = crate::shellwrap::split_env(toks);
    let base = crate::shellwrap::base_lower(&prog);
    let args = argv.get(1..).unwrap_or(&[]);
    let mut repo = None;
    let mut words = Vec::new();
    let mut i = 0;
    while i < args.len() {
        let arg = args[i].as_str();
        if arg == "-R" || arg == "--repo" {
            repo = Some(args.get(i + 1).map(String::as_str));
            i += 2;
            continue;
        }
        if let Some(value) = arg.strip_prefix("--repo=") {
            repo = Some(Some(value));
        } else if VALUE_FLAGS.contains(&arg) {
            i += 1;
        } else if !arg.starts_with('-') {
            words.push(arg);
        }
        i += 1;
    }
    let verbs: &[&str] = match base.as_str() {
        "gh" => &["merge"],
        "glab" => &["merge", "accept"],
        _ => return None,
    };
    let noun = if base == "gh" { "pr" } else { "mr" };
    match words[..] {
        [n, verb, ..] if n == noun && verbs.contains(&verb) => {}
        _ => return None,
    }
    let repo = match repo {
        None => None,
        Some(named) => match named.map(Repo::try_from) {
            Some(Ok(repo)) => Some(repo),
            _ => return Some(None),
        },
    };
    Some(match words[2..] {
        [number] => number
            .parse::<u64>()
            .ok()
            .filter(|n| *n > 0 && number.bytes().all(|b| b.is_ascii_digit()))
            .map(|number| Request { repo, number }),
        _ => None,
    })
}

/// **What the app answered about a held merge** (Q239), as the hook heard it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppSaid<'a> {
    /// The app read the request from the forge and holds one ask for it: what the chat is told.
    Held(&'a str),
    /// The app raised no ask, in its words.
    Refused(&'a str),
    /// No app answered in time, or none was there.
    Unheard,
}

/// **The trace reason the hook refuses `held` with**, by what the app said (Q239): a hold only
/// where the app confirmed it holds an ask, which it records itself; otherwise a block, which
/// the event log records from the refusal, so a stop is never silent.
pub fn reason(held: &Held, app: &AppSaid<'_>) -> &'static str {
    match (held, app) {
        (Held::Request(_), AppSaid::Held(_)) => REASON,
        (Held::Request(_), _) => REASON_NOT_ASKED,
        (Held::Unreadable | Held::Land, _) => REASON_UNREADABLE,
    }
}

/// **What the chat is told when its merge is held**, with what the app said: that a chat never
/// merges, that nothing was merged, and what happens next. It says the person is asked only
/// where the app confirmed it holds the ask.
pub fn refusal(held: &Held, app: &AppSaid<'_>) -> String {
    const OPENS: &str = "purlis holds merges: a chat never merges, the app does, on the \
                         person's press. Nothing was merged.";
    match (held, app) {
        (Held::Request(_), AppSaid::Held(said)) => format!("{OPENS} {said}"),
        (Held::Request(request), AppSaid::Refused(why)) => format!(
            "{OPENS} Nobody was asked to merge {}{}: {why} Ask the person to merge it in the \
             window. Do not merge it another way.",
            request.number_said(),
            request.repo_said()
        ),
        (Held::Request(request), AppSaid::Unheard) => format!(
            "{OPENS} The app was not heard from about {}{}, so nobody was asked: ask the person \
             to merge it in the window. Do not merge it another way.",
            request.number_said(),
            request.repo_said()
        ),
        (Held::Land, _) => format!(
            "{OPENS} `purlis change land` lands a change's requests, which in a chat purlis \
             started is the person's to do now, so nobody was asked. Ask them to land it from \
             the window, or run `gh pr merge <number>` (or `glab mr merge <number>`) for one \
             request, and the person is asked to merge it in Asks."
        ),
        (Held::Unreadable, _) => format!(
            "{OPENS} purlis cannot tell which request this merges, so nobody was asked. Run `gh \
             pr merge <number>` (or `glab mr merge <number>`) on its own, and the person is asked \
             to merge it in Asks."
        ),
    }
}

impl Request {
    fn number_said(&self) -> String {
        format!("request {}", self.number)
    }

    fn repo_said(&self) -> String {
        self.repo
            .as_ref()
            .map(|repo| format!(" of {}", repo.as_str()))
            .unwrap_or_default()
    }
}

/// **What the app tells the chat it holds the merge**, once it has read the request from the
/// forge and an ask waits for it.
pub fn handed_said(request: &Request) -> String {
    format!(
        "The person is asked in Asks to merge {}{}. Carry on with other work; do not merge it \
         another way.",
        request.number_said(),
        request.repo_said()
    )
}

/// How the app finds a held request and reads it from the forge: its key and what its ask shows,
/// or why nobody is asked.
pub type Resolve<'a> = dyn Fn(&Request) -> Result<(MergeKey, Facts), String> + 'a;

/// **The app's half of a held merge**: `resolve` finds the request in the project and reads it
/// from the forge, as the app's adapter does ([`MergeKey`], [`read`]), and chat `session`'s ask
/// for it is held at `now`, with the facts the forge gave. Answers what became of it, or why
/// nobody was asked.
pub fn asked(
    holds: &Holds,
    session: u32,
    request: &Request,
    resolve: &Resolve<'_>,
    now: u64,
) -> Result<(MergeKey, Raised), String> {
    let (key, facts) = resolve(request)?;
    let raised = holds.raise(session, key.clone(), facts, now);
    Ok((key, raised))
}

/// **Whether a ruleset protects a request's base**, as the app's own login read it (R-146).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ruleset {
    /// A ruleset protects it: GitHub's rules for the branch, GitLab's protected branch.
    Protects,
    /// None does.
    None,
    /// The forge did not say, in these words. Never read as none.
    Unknown(String),
}

/// **The ruleset state a person was shown** (R-150): what a press says it saw, and what it is
/// held to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Seen {
    Protects,
    None,
    Unknown,
}

impl Ruleset {
    /// The state this read gives, as a row shows it.
    pub fn seen(&self) -> Seen {
        match self {
            Ruleset::Protects => Seen::Protects,
            Ruleset::None => Seen::None,
            Ruleset::Unknown(_) => Seen::Unknown,
        }
    }
}

/// **What a merge ask shows of its request** (R-119, R-146), every one of them read from the
/// forge when the ask was raised, never from the chat. Its base is the key's ([`MergeKey`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Facts {
    pub title: String,
    pub author: String,
    /// The checks at the ask's head.
    pub checks: crate::forge::checks::Checks,
    pub ruleset: Ruleset,
    /// When the forge was read for these facts, in seconds since 1970: a try again at the same
    /// head reads it again, and the newer read replaces these ([`Holds::raise`]).
    pub read_at: u64,
}

/// **A request, as the forge says it stands**: where it is ([`HeadAt`](crate::forge::pr::HeadAt)),
/// which the ask is keyed on, and what the ask shows ([`Facts`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Read {
    pub at: crate::forge::pr::HeadAt,
    pub facts: Facts,
}

/// **Reads request `number` of the repo at `path` from the forge** for a merge ask at `now`,
/// every call as the window, so with the app's own forge login and never a chat's token
/// (R-146). **The head alone gates the ask** (Q248): where the request stands must be read
/// within `within`, or nobody is asked. The checks at its head and whether a ruleset protects its
/// base are read alongside it, and one the forge has not given by then, or fails to give, is
/// unknown, never passed or none, and the ask is still raised. A late read is left to finish on
/// its own and never waited for.
pub fn read(
    backend: std::sync::Arc<dyn crate::forge::backend::ForgeBackend>,
    path: &str,
    number: u64,
    within: std::time::Duration,
    now: u64,
) -> Result<Read, String> {
    let by = std::time::Instant::now() + within;
    let left = || by.saturating_duration_since(std::time::Instant::now());
    let late = "the forge did not say in time";
    let head = {
        let path = path.to_owned();
        alongside(&backend, move |forge| {
            let pr = crate::forge::pr::Pr {
                number,
                url: String::new(),
            };
            forge.head(&crate::forge::backend::Caller::window(), &path, &pr)
        })
    };
    let at = head
        .recv_timeout(left())
        .map_err(|_| "the forge did not answer in time".to_owned())?
        .map_err(|why| why.to_string())?;
    let checks = {
        let (path, sha) = (path.to_owned(), at.sha.clone());
        alongside(&backend, move |forge| {
            forge.checks_at(
                &crate::forge::backend::Caller::window(),
                &path,
                &sha,
                number,
            )
        })
    };
    let ruleset = {
        let (path, base) = (path.to_owned(), at.base.clone());
        alongside(&backend, move |forge| {
            forge.ruleset_protects(&crate::forge::backend::Caller::window(), &path, &base)
        })
    };
    let checks = checks
        .recv_timeout(left())
        .unwrap_or_else(|_| crate::forge::checks::Checks::unknown(late));
    let ruleset = match ruleset.recv_timeout(left()) {
        Ok(Ok(true)) => Ruleset::Protects,
        Ok(Ok(false)) => Ruleset::None,
        Ok(Err(why)) => Ruleset::Unknown(why.to_string()),
        Err(_) => Ruleset::Unknown(late.to_owned()),
    };
    Ok(Read {
        facts: Facts {
            title: at.title.clone(),
            author: at.author.clone(),
            checks,
            ruleset,
            read_at: now,
        },
        at,
    })
}

/// Runs `read` against `backend` on a thread of its own and hands back where its answer comes.
/// The thread is never joined: a read that comes too late finishes, and its answer is dropped.
/// A thread that cannot be started sends nothing, so it reads as late.
fn alongside<T: Send + 'static>(
    backend: &std::sync::Arc<dyn crate::forge::backend::ForgeBackend>,
    read: impl FnOnce(&dyn crate::forge::backend::ForgeBackend) -> T + Send + 'static,
) -> std::sync::mpsc::Receiver<T> {
    let (tell, heard) = std::sync::mpsc::channel();
    let backend = std::sync::Arc::clone(backend);
    let started = std::thread::Builder::new()
        .name("merge-hold".to_owned())
        .spawn(move || {
            let _ = tell.send(read(backend.as_ref()));
        });
    if let Err(why) = started {
        tracing::warn!("purlis: a merge ask's read could not start: {why}");
    }
    heard
}

/// **Where the request stands now**, as the forge says at a press.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct At {
    pub base: Base,
    /// Whether the repo's own method squashes it.
    pub squash: bool,
}

/// What a press got the forge to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Merged {
    /// Merged now.
    Now,
    /// Put in its base branch's merge queue or train, which merges it in its turn.
    Queued,
}

/// **Whether the project's inventory lists the repo at `path` on `forge`** (Q239): a row of the
/// same forge kind and path whose web address is on the same host, read through
/// [`crate::inventory::load`], which refuses an inventory a link takes out of the project. A row
/// with no web address names no host, and is not this repo.
pub fn listed_in(root: &std::path::Path, forge: &crate::forge::Forge, path: &str) -> bool {
    let Ok(doc) = crate::inventory::load(root, "") else {
        return false;
    };
    crate::inventory::listed(&doc).iter().any(|row| {
        let text = |field: &str| row.get(field).and_then(serde_json::Value::as_str);
        text("path_with_namespace") == Some(path)
            && text("forge") == Some(forge.kind.word())
            && text("web_url")
                .and_then(|web| url::Url::parse(web).ok())
                .and_then(|web| {
                    web.host_str().map(|host| match web.port() {
                        Some(port) => format!("{host}:{port}"),
                        None => host.to_owned(),
                    })
                })
                .is_some_and(|host| host.eq_ignore_ascii_case(&forge.host))
    })
}

/// **The forge, as a press needs it**: where the request stands now, and its merge, only while
/// its head is `key.head`. The app's adapter ([`ByForge`]) makes these calls with its own forge
/// login, never a chat's.
pub trait MergeForge {
    /// Where the request `key` names stands now, or why the forge could not say.
    fn at(&self, key: &MergeKey) -> Result<At, String>;
    /// Merges, by the repo's method (`squash`), or says why the forge refused, in its words.
    fn merge_at(&self, key: &MergeKey, squash: bool) -> Result<Merged, String>;
}

/// **A press, through a forge backend** (Q239): a repo that lands through a merge queue or
/// train has the request put in it, as `purlis change land` does, and any other is merged at
/// the key's head. A forge that sets it to merge later instead, which then merges whatever
/// head the branch has, is an error the person sees, never a merge.
pub struct ByForge<'a> {
    pub backend: &'a dyn crate::forge::backend::ForgeBackend,
    pub caller: crate::forge::backend::Caller,
    /// The repo's path on its forge.
    pub path: String,
}

impl ByForge<'_> {
    fn pr(key: &MergeKey) -> crate::forge::pr::Pr {
        crate::forge::pr::Pr {
            number: key.number,
            url: String::new(),
        }
    }
}

impl MergeForge for ByForge<'_> {
    fn at(&self, key: &MergeKey) -> Result<At, String> {
        let now = self
            .backend
            .head(&self.caller, &self.path, &Self::pr(key))
            .map_err(|why| why.to_string())?;
        Ok(At {
            base: Base::try_from(now.base).map_err(|why| why.to_string())?,
            squash: now.squash,
        })
    }

    fn merge_at(&self, key: &MergeKey, squash: bool) -> Result<Merged, String> {
        let pr = Self::pr(key);
        let how = crate::forge::pr::MergeAs {
            squash,
            title: format!("Merge request {} of {}", key.number, key.repo.as_str()),
            message: "Merged in purlis on the person's press.".to_owned(),
        };
        let head = key.head.as_str();
        let queued = self
            .backend
            .lands_through_queue(&self.caller, &self.path, &pr)
            .map_err(|why| format!("purlis could not tell how it lands: {why}"))?;
        if queued {
            return self
                .backend
                .enqueue_at(&self.caller, &self.path, &pr, head, &how)
                .map(|()| Merged::Queued)
                .map_err(|why| why.to_string());
        }
        match self
            .backend
            .merge_at(&self.caller, &self.path, &pr, head, &how)
            .map_err(|why| why.to_string())?
        {
            crate::forge::pr::MergedAt::Now => Ok(Merged::Now),
            crate::forge::pr::MergedAt::Later(why) => Err(format!(
                "{why}. It will merge whatever head the branch has when a pipeline passes, not \
                 the one you were asked about. Cancel its auto-merge on the forge."
            )),
        }
    }
}

/// One merge ask that waits: the chat it holds, its key, what the forge said of the request
/// when it was raised, and since when, in seconds since 1970.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Waits {
    pub session: u32,
    pub key: MergeKey,
    pub facts: Facts,
    pub since: u64,
    /// **Asked once more** (Q269): the window read the request again and asked the person once
    /// more, of these facts. Only [`Reread::AsksOnceMore`] sets it, a raise whose facts show
    /// otherwise clears it, and [`Holds::press`] merges nothing without it.
    pub asked: bool,
}

impl Facts {
    /// Whether `self` and `other` show the person the same, whenever each was read.
    fn shows_as(&self, other: &Facts) -> bool {
        Facts {
            read_at: other.read_at,
            ..self.clone()
        } == *other
    }
}

/// **Why a waiting ask is read again** ([`Holds::refresh`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reread {
    /// Merge was pressed once: the read is what the person is asked once more of (Q248), and the
    /// ask is marked asked (Q269).
    AsksOnceMore,
    /// The once-more Merge was pressed: the read is checked before the merge, and leaves the
    /// mark as it is (checks or a title that change after the question still merge, Q269).
    Presses,
}

impl Waits {
    /// The ask shows `facts`, unless they were read before what it shows (Q269); where they
    /// show otherwise, the person was not asked of them.
    fn replace(&mut self, facts: Facts) {
        if facts.read_at < self.facts.read_at {
            return;
        }
        if !self.facts.shows_as(&facts) {
            self.asked = false;
        }
        self.facts = facts;
    }
}

/// What a re-read did to a waiting ask ([`Holds::refresh`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refreshed {
    /// The request is at the ask's head and base still: the ask shows the new read.
    Same,
    /// The request moved to another head or base: the ask, which its head can no longer merge,
    /// is retired for one at where it is now (Q242).
    Moved,
}

/// What became of a raise ([`Holds::raise`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Raised {
    /// One merge ask began to wait: the first for that request, or one in the place of the
    /// chat's ask at another head or base (Q242).
    New,
    /// The chat's ask for that request, at that head and base, waits already: still one.
    Already,
    /// The chat has as many merge asks waiting as one chat may: nothing was asked.
    TooMany,
}

/// Why a press merged nothing.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum NotMerged {
    /// No such ask waits: answered already, put away, or its chat gone.
    #[error("this merge is not waiting any more, so nothing was merged")]
    NotHeld,
    /// The forge refused it, in its words. The ask still waits.
    #[error("the forge did not merge it: {0}")]
    Forge(String),
    /// The request now merges into another branch than the one it was asked about. The ask
    /// still waits; the chat's next try asks again at the new base.
    #[error(
        "the request now merges into {now}, not {was} as it did when you were asked, so nothing \
         was merged"
    )]
    BaseMoved { was: String, now: String },
    /// Whether a ruleset protects the base is not what the person was shown (R-150). The ask
    /// still waits, showing what purlis read last.
    #[error(
        "whether a ruleset protects the base changed since you were shown it, so nothing was \
         merged. Press Merge again"
    )]
    RulesetChanged,
    /// The person was not asked once more of what the ask shows (Q269): Merge was not pressed
    /// once first, or the request's facts changed since. The ask still waits.
    #[error(
        "you were not asked once more about this merge as it stands now, so nothing was merged. \
         Press Merge, read what it asks, then merge"
    )]
    NotAsked,
}

/// **The merge asks the app holds**, until the person answers.
#[derive(Debug, Default)]
pub struct Holds {
    held: Mutex<Vec<Waits>>,
}

impl Holds {
    fn lock(&self) -> std::sync::MutexGuard<'_, Vec<Waits>> {
        self.held.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Holds chat `session`'s merge of `key` at `now`, showing `facts`: one ask per chat and
    /// request however often it tries, waiting since the first, and [`AT_MOST_PER_CHAT`] of one
    /// chat's at most.
    /// **A moved head is a new ask** (Q242): a try again at another head or base than the
    /// waiting ask's retires it and raises a new one in its place, waiting since `now`, so a key
    /// is never rewritten under an ask the person may be looking at.
    /// **A try again at the same head refreshes what the ask shows** (Q248): it is the one ask
    /// still, waiting since it was first raised, and the newer read's `facts` replace the old;
    /// **an older read never replaces a newer** (Q269). Facts that show otherwise take the
    /// ask's asked mark away.
    pub fn raise(&self, session: u32, key: MergeKey, facts: Facts, now: u64) -> Raised {
        let mut held = self.lock();
        if let Some(at) = held
            .iter()
            .position(|one| one.session == session && one.key.same_request(&key))
        {
            if held[at].key == key {
                held[at].replace(facts);
                return Raised::Already;
            }
            held.remove(at);
        }
        if held.iter().filter(|one| one.session == session).count() >= AT_MOST_PER_CHAT {
            return Raised::TooMany;
        }
        held.push(Waits {
            session,
            key,
            facts,
            since: now,
            asked: false,
        });
        Raised::New
    }

    /// **A re-read of chat `session`'s waiting ask for `was`** (Q248), as the forge gives it now:
    /// at `now` (the key the read gives) and showing `facts`, which keep the time they were
    /// read. At the same head and base it is the one ask still, waiting since it was raised; at
    /// another, the ask at `was` is retired and one at `now` waits from the read (Q242). An ask
    /// that waits no more is never raised again by a re-read. An older read never replaces a
    /// newer (Q269). Read `why` [`Reread::AsksOnceMore`], the ask is marked asked where it shows
    /// what this read gave.
    pub fn refresh(
        &self,
        session: u32,
        was: &MergeKey,
        now: MergeKey,
        facts: Facts,
        why: Reread,
    ) -> Result<Refreshed, NotMerged> {
        let mut held = self.lock();
        let at = held
            .iter()
            .position(|one| one.session == session && one.key == *was)
            .ok_or(NotMerged::NotHeld)?;
        if now == *was {
            let one = &mut held[at];
            let asked = one.asked;
            let before = one.facts.ruleset.seen();
            one.replace(facts.clone());
            one.asked = match why {
                Reread::AsksOnceMore => one.facts.shows_as(&facts),
                // The press's own read keeps the mark only while the ruleset is what the person
                // was asked of: a changed one is asked of again (Q269).
                Reread::Presses => asked && one.facts.ruleset.seen() == before,
            };
            return Ok(Refreshed::Same);
        }
        if !was.same_request(&now) {
            return Err(NotMerged::NotHeld);
        }
        let since = facts.read_at;
        held.remove(at);
        held.retain(|one| !(one.session == session && one.key.same_request(&now)));
        held.push(Waits {
            session,
            key: now,
            facts,
            since,
            asked: false,
        });
        Ok(Refreshed::Moved)
    }

    /// Every merge ask that waits, oldest first.
    pub fn every(&self) -> Vec<Waits> {
        self.lock().clone()
    }

    /// The key of chat `session`'s waiting ask for request `number` of `repo` at `head`: what
    /// a press names, with the base the ask was raised at.
    pub fn key_of(
        &self,
        session: u32,
        repo: &Repo,
        number: u64,
        head: &HeadSha,
    ) -> Option<MergeKey> {
        self.lock()
            .iter()
            .find(|one| {
                one.session == session
                    && one.key.repo == *repo
                    && one.key.number == number
                    && one.key.head == *head
            })
            .map(|one| one.key.clone())
    }

    /// **The person's press**: chat `session`'s ask for `key` is taken out, so a second press
    /// merges nothing, the forge is read again, and asked to merge exactly `key` by the repo's
    /// own method, only while it still merges into `key.base` (Q239). `seen` is the ruleset
    /// state the person's row showed: unless it is the one the ask holds, nothing is merged and
    /// the ask waits, showing what purlis read (R-150). Where the base moved or the forge
    /// refuses, the ask waits again, as it was, unless the chat's try again raised a newer ask
    /// for the request meanwhile: then one row stands for it, the newer. `by` is the human who
    /// pressed, which no chat can be.
    pub fn press(
        &self,
        session: u32,
        key: &MergeKey,
        seen: Seen,
        by: Answerer,
        forge: &dyn MergeForge,
    ) -> Result<(HumanScope, Merged), NotMerged> {
        let taken = {
            let mut held = self.lock();
            let at = held
                .iter()
                .position(|one| one.session == session && one.key == *key)
                .ok_or(NotMerged::NotHeld)?;
            if !held[at].asked {
                return Err(NotMerged::NotAsked);
            }
            if held[at].facts.ruleset.seen() != seen {
                return Err(NotMerged::RulesetChanged);
            }
            held.remove(at)
        };
        let merged = forge
            .at(key)
            .map_err(NotMerged::Forge)
            .and_then(|at| {
                if at.base == key.base {
                    Ok(at)
                } else {
                    Err(NotMerged::BaseMoved {
                        was: key.base.as_str().to_owned(),
                        now: at.base.as_str().to_owned(),
                    })
                }
            })
            .and_then(|at| forge.merge_at(key, at.squash).map_err(NotMerged::Forge));
        match merged {
            Ok(merged) => Ok((by.scope(), merged)),
            Err(why) => {
                let mut held = self.lock();
                if !held
                    .iter()
                    .any(|one| one.session == session && one.key.same_request(key))
                {
                    held.push(taken);
                }
                Err(why)
            }
        }
    }

    /// **The person's Don't merge**: the ask waits no more and nothing is merged. Answers
    /// whether it was waiting.
    pub fn dont_merge(&self, session: u32, key: &MergeKey, _by: Answerer) -> bool {
        let mut held = self.lock();
        let before = held.len();
        held.retain(|one| !(one.session == session && one.key == *key));
        held.len() != before
    }

    /// Chat `session` has closed: none of its merge asks waits.
    pub fn forget(&self, session: u32) {
        self.lock().retain(|one| one.session != session);
    }
}

#[cfg(test)]
#[path = "mergehold/tests.rs"]
mod tests;
