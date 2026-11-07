// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A project at the scale spec's target size, written from a seed (#5497, spec #5486).
//!
//! **Written through purlis-core's own writers**, each given its stamp: `purlis init`'s
//! scaffold, a workspace's `ensure`, its vision, todos and memories, a persona's definition and
//! memories, session records, and the LIVE block. So what this writes is the plane format by
//! construction, and a change to a writer is a change here too. The one exception is the
//! dispatch record, whose writer mints its id from the clock: those are core's own
//! [`dispatchrecord::Record`], serialized as the store writes them, with an id made from the
//! seed.
//!
//! **The same seed and moment give the same bytes.** Everything this writes comes from one
//! seeded generator ([`Rng`], SplitMix64, written out here so no crate's update can change
//! it) and from [`Plan::at`], the newest moment the project carries. Nothing reads the clock.
//! `at` is given rather than fixed because the app collects a dispatch record 30 days after it
//! ended: a nightly run passes the day it runs, so its project is never older than that.

use std::io;
use std::path::Path;

use chrono::{Duration, NaiveDateTime};
use purlis_core::dispatchrecord::{self, Record};
use purlis_core::workspaces::Plane;

/// How much of each thing a generated project holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Size {
    pub workspaces: usize,
    pub personas: usize,
    /// Across every workspace, spread evenly.
    pub workspace_memories: usize,
    /// Across every persona, spread evenly; the shared store is apart.
    pub persona_memories: usize,
    pub shared_memories: usize,
    pub todos_per_workspace: usize,
    pub sessions_per_workspace: usize,
    pub dispatches: usize,
    /// Of `dispatches`, how many are still running.
    pub running_dispatches: usize,
}

impl Size {
    /// The spec's target (#5486): 1,000 workspaces, 100 personas, 100,000 memories across the
    /// workspaces and 100,000 across the personas.
    ///
    /// The rest is in the proportions of the projects in use when it was written (four
    /// workspaces, about 750 memories, a few open todos and records each, a dispatch a few
    /// times a day), taken up to that size: five open todos and ten session records a
    /// workspace, a thousand shared memories, and five thousand dispatches, twenty of them
    /// running.
    pub const TARGET: Self = Self {
        workspaces: 1_000,
        personas: 100,
        workspace_memories: 100_000,
        persona_memories: 100_000,
        shared_memories: 1_000,
        todos_per_workspace: 5,
        sessions_per_workspace: 10,
        dispatches: 5_000,
        running_dispatches: 20,
    };

    /// A few of everything, for a test.
    pub fn small() -> Self {
        Self {
            workspaces: 3,
            personas: 2,
            workspace_memories: 30,
            persona_memories: 20,
            shared_memories: 4,
            todos_per_workspace: 2,
            sessions_per_workspace: 2,
            dispatches: 6,
            running_dispatches: 1,
        }
    }
}

/// What to generate.
#[derive(Debug, Clone)]
pub struct Plan {
    pub seed: u64,
    pub size: Size,
    /// The newest moment anything in the project carries.
    pub at: NaiveDateTime,
}

/// A word every generated project holds in many memories but not most: what the scale run
/// asks `recall` for.
pub const QUERY: &str = "migration";

/// The name of the workspace numbered `n`, from 1.
pub fn workspace_name(n: usize) -> String {
    format!("ws-{n:04}")
}

/// The name of the persona numbered `n`, from 1.
pub fn persona_name(n: usize) -> String {
    format!("persona-{n:03}")
}

/// Writes the project `plan` describes into `root`, an empty directory.
///
/// **Each workspace and each persona's memory is written on a thread of its own pool**, from a
/// generator seeded by the plan's seed and its own number ([`Rng::of`]), so what each holds does
/// not depend on which thread wrote it or when. Almost all of a project's time is the disk's: a
/// store's writer takes its lock and writes each file whole. What the plane holds once (`init`,
/// the personas' definitions and the roster they rewrite, the LIVE block, the dispatch records)
/// is written in order, by this thread.
pub fn generate(plan: &Plan, root: &Path) -> io::Result<()> {
    let size = plan.size;
    let now = plan.at.and_utc();
    init(root)?;
    let plane = Plane::open(root);

    let per_workspace = spread(size.workspace_memories, size.workspaces);
    in_parallel(size.workspaces, |i| {
        let n = i + 1;
        let mut rng = Rng::of(plan.seed, Stream::Workspace, n);
        let name = workspace_name(n);
        purlis_core::wscmd::ensure::ensure(root, &name, now, "Scale Generator")
            .map_err(io::Error::other)?;
        let workspace = plane.workspace(&name).map_err(io::Error::other)?;
        workspace.set_vision(&rng.sentence(6, 12))?;
        for t in 0..size.todos_per_workspace {
            workspace.add_todo(&rng.title(t), rng.before(plan.at, 60))?;
        }
        for m in 0..per_workspace[i] {
            let title = rng.title(m);
            workspace.remember_titled(&rng.body(), Some(&title), rng.before(plan.at, 365))?;
        }
        for _ in 0..size.sessions_per_workspace {
            session(root, &name, &mut rng, plan.at)?;
        }
        Ok(())
    })?;
    let names: Vec<String> = (1..=size.workspaces).map(workspace_name).collect();
    purlis_core::wscmd::write_live_block(root, names.iter().map(String::as_str))?;

    for n in 1..=size.personas {
        let mut rng = Rng::of(plan.seed, Stream::Definition, n);
        define(
            root,
            &persona_name(n),
            &rng.sentence(2, 4),
            &rng.sentence(4, 8),
        )?;
    }
    let per_persona = spread(size.persona_memories, size.personas);
    // The shared store is the one after the personas.
    in_parallel(size.personas + 1, |i| {
        let n = i + 1;
        let mut rng = Rng::of(plan.seed, Stream::Persona, n);
        let (name, count) = if n > size.personas {
            ("_shared".to_owned(), size.shared_memories)
        } else {
            (persona_name(n), per_persona[i])
        };
        let persona = plane.persona(&name).map_err(io::Error::other)?;
        for m in 0..count {
            let title = rng.title(m);
            persona.remember_titled(&rng.body(), Some(&title), rng.before(plan.at, 365))?;
        }
        Ok(())
    })?;

    let mut rng = Rng::of(plan.seed, Stream::Dispatch, 0);
    for d in 0..size.dispatches {
        let running = d < size.running_dispatches;
        dispatch(root, &mut rng, plan.at, size, running)?;
    }
    Ok(())
}

/// Runs `work(i)` for every `i` in `0..count` on a pool of threads, and answers the first
/// failure, if any. Four threads a core: the work waits on the disk far more than it computes.
fn in_parallel(count: usize, work: impl Fn(usize) -> io::Result<()> + Sync) -> io::Result<()> {
    let next = std::sync::atomic::AtomicUsize::new(0);
    let failed = std::sync::Mutex::new(None);
    let threads = std::thread::available_parallelism()
        .map_or(4, std::num::NonZeroUsize::get)
        .saturating_mul(4)
        .clamp(1, 32);
    std::thread::scope(|scope| {
        for _ in 0..threads {
            scope.spawn(|| {
                loop {
                    let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    if i >= count {
                        return;
                    }
                    if let Err(why) = work(i) {
                        failed
                            .lock()
                            .unwrap_or_else(std::sync::PoisonError::into_inner)
                            .get_or_insert(why);
                        // Nothing more is started once one has failed.
                        next.store(count, std::sync::atomic::Ordering::Relaxed);
                        return;
                    }
                }
            });
        }
    });
    match failed
        .into_inner()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
    {
        Some(why) => Err(why),
        None => Ok(()),
    }
}

/// `n` spread over `over` as evenly as whole numbers allow, the first ones taking the rest.
fn spread(n: usize, over: usize) -> Vec<usize> {
    let over = over.max(1);
    (0..over)
        .map(|i| n / over + usize::from(i < n % over))
        .collect()
}

/// What `purlis init` writes, with no front-door persona: the personas are the generator's.
fn init(root: &Path) -> io::Result<()> {
    let place = purlis_core::plane::Place {
        root: root.to_path_buf(),
        is_plane: false,
    };
    let args = purlis_core::scaffold::InitArgs {
        forge: Some("github".to_owned()),
        owner: "scale".to_owned(),
        host: None,
        clone_this_repo: false,
        plane_is_this_repo: false,
        adopt: None,
        now: None,
        front_door: None,
    };
    let outcome = purlis_core::scaffold::init(&place, &args);
    if outcome.code != 0 {
        return Err(io::Error::other(said(&outcome.said)));
    }
    Ok(())
}

/// `purlis persona create <name> --role … --delegate-when …`.
fn define(root: &Path, name: &str, role: &str, when: &str) -> io::Result<()> {
    let ask = purlis_core::personaverbs::define::Create {
        name,
        role: Some(role),
        delegate_when: Some(when),
        vault: None,
        extends: None,
        select: None,
        force: false,
    };
    let mut told = Vec::new();
    let code =
        purlis_core::personaverbs::define::create(root, &ask, None, &mut |say| told.push(say));
    if code != 0 {
        return Err(io::Error::other(said(&told)));
    }
    Ok(())
}

fn said(told: &[impl std::fmt::Debug]) -> String {
    told.iter()
        .map(|say| format!("{say:?}"))
        .collect::<Vec<_>>()
        .join("; ")
}

/// One session record of a closed chat in `workspace`, as `purlis session record` writes it.
fn session(root: &Path, workspace: &str, rng: &mut Rng, at: NaiveDateTime) -> io::Result<()> {
    let title = rng.title(0);
    let body = format!(
        "## Goal\n{}\n\n## Done\n{}\n\n## Decisions\n{}\n\n## Open\n{}\n\n## How to resume\n{}\n",
        rng.sentence(6, 12),
        rng.body(),
        rng.sentence(6, 12),
        rng.sentence(4, 8),
        rng.sentence(4, 8)
    );
    let facts = purlis_core::sessionrecord::Facts {
        place: purlis_core::active::Place::Workspace(workspace.to_owned()),
        at: rng.before(at, 90),
        chat: None,
        persona: None,
        pieces: Vec::new(),
    };
    let new = purlis_core::sessionrecord::New {
        title: &title,
        body: &body,
        facts: &facts,
    };
    purlis_core::sessionrecord::record(root, &new)
        .map(|_| ())
        .map_err(|why| io::Error::other(why.to_string()))
}

/// One dispatch record, ended within the last 20 days unless `running`.
fn dispatch(
    root: &Path,
    rng: &mut Rng,
    at: NaiveDateTime,
    size: Size,
    running: bool,
) -> io::Result<()> {
    let started = rng.before(at, 20);
    let id = ulid::Ulid::from_parts(
        u64::try_from(started.and_utc().timestamp_millis()).unwrap_or_default(),
        (u128::from(rng.next()) << 64) | u128::from(rng.next()),
    )
    .to_string();
    let workspace = workspace_name(1 + rng.below(size.workspaces));
    let persona = persona_name(1 + rng.below(size.personas));
    let stamp = |moment: NaiveDateTime| moment.format("%Y-%m-%dT%H:%M:%SZ").to_string();
    let ended = (!running).then(|| started + Duration::minutes(5 + rng.below(240) as i64));
    // Through core's own reading of a record, so a field this does not name takes the default
    // core gives it, and a shape core would not read fails here rather than in the run.
    let record: Record = serde_json::from_value(serde_json::json!({
        "v": dispatchrecord::VERSION,
        "id": id,
        "mode": "task",
        "asker": { "chat": 1 + rng.below(40), "name": rng.title(0), "workspace": workspace },
        "persona": persona,
        "worker": {
            "chat": 1 + rng.below(40),
            "name": rng.title(0),
            "persona": persona,
            "harness": "claude",
        },
        "place": { "workspace": workspace, "folder": format!("workspaces/{workspace}") },
        "brief": rng.body(),
        "report_owed": true,
        "started": stamp(started),
        "ended": ended.map(stamp),
        "report": ended.map(|_| serde_json::json!({ "outcome": "done", "text": rng.body() })),
        "cleared": !running,
    }))
    .map_err(io::Error::other)?;
    let dir = dispatchrecord::dir(root);
    std::fs::create_dir_all(&dir)?;
    let text = serde_json::to_string_pretty(&record).map_err(io::Error::other)?;
    let path = dir.join(format!("{id}.json"));
    std::fs::write(&path, format!("{text}\n"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}

/// SplitMix64: the whole of the project's randomness, kept here so that no dependency's
/// release can change what a seed gives.
struct Rng(u64);

/// What a generator is for: each thing the project holds draws from its own.
#[derive(Clone, Copy)]
enum Stream {
    Workspace = 1,
    Definition = 2,
    Persona = 3,
    Dispatch = 4,
}

impl Rng {
    /// The generator for thing `n` of `stream`, from the plan's `seed`.
    fn of(seed: u64, stream: Stream, n: usize) -> Self {
        let mut mixed = Self(seed ^ ((stream as u64) << 56));
        let from = mixed.next() ^ n as u64;
        Self(Self(from).next())
    }

    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// A number in `0..n`.
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }

    /// A moment within `days` before `at`, to the second.
    fn before(&mut self, at: NaiveDateTime, days: usize) -> NaiveDateTime {
        at - Duration::seconds(self.below(days * 86_400) as i64)
    }

    /// A word, the first ones of [`WORDS`] far more often than the last, as in prose.
    fn word(&mut self) -> &'static str {
        let a = self.below(WORDS.len());
        let b = self.below(WORDS.len());
        WORDS[a.min(b)]
    }

    fn words(&mut self, least: usize, most: usize) -> Vec<&'static str> {
        let n = least + self.below(most - least + 1);
        (0..n).map(|_| self.word()).collect()
    }

    /// A sentence of `least..=most` words, capitalised, with a full stop.
    fn sentence(&mut self, least: usize, most: usize) -> String {
        let mut text = self.words(least, most).join(" ");
        if let Some(first) = text.get(..1) {
            text.replace_range(..1, &first.to_uppercase());
        }
        text.push('.');
        text
    }

    /// A title, made distinct within its store by its number there.
    fn title(&mut self, n: usize) -> String {
        let mut words = self.words(3, 7).join(" ");
        if let Some(first) = words.get(..1) {
            words.replace_range(..1, &first.to_uppercase());
        }
        format!("{words} {}", n + 1)
    }

    /// A memory's body: two to five sentences.
    fn body(&mut self) -> String {
        let n = 2 + self.below(4);
        (0..n)
            .map(|_| self.sentence(6, 16))
            .collect::<Vec<_>>()
            .join(" ")
    }
}

/// The vocabulary, most common first. [`QUERY`] sits in the middle, so a recall for it finds
/// a few percent of the memories, as a real query does.
const WORDS: &[&str] = &[
    "the",
    "build",
    "test",
    "api",
    "service",
    "deploy",
    "config",
    "branch",
    "review",
    "release",
    "chat",
    "workspace",
    "memory",
    "index",
    "cache",
    "queue",
    "worker",
    "client",
    "server",
    "schema",
    "token",
    "session",
    "window",
    "panel",
    "sidebar",
    "commit",
    "merge",
    "rebase",
    "pipeline",
    "runner",
    "image",
    "cluster",
    "node",
    "pod",
    "volume",
    "secret",
    "vault",
    "persona",
    "dispatch",
    "report",
    "rollout",
    "flag",
    "metric",
    "alert",
    "latency",
    "budget",
    "timeout",
    "retry",
    "limit",
    "watch",
    "folder",
    "file",
    "path",
    "lock",
    "write",
    "read",
    "parse",
    "format",
    "render",
    "theme",
    "colour",
    "layout",
    "story",
    "baseline",
    "screenshot",
    "keyboard",
    "focus",
    "menu",
    "palette",
    "search",
    "query",
    "result",
    "rank",
    "score",
    "migration",
    "database",
    "table",
    "column",
    "row",
    "backup",
    "restore",
    "snapshot",
    "fixture",
    "seed",
    "scale",
    "nightly",
    "regression",
    "issue",
    "label",
    "milestone",
    "ticket",
    "spec",
    "decision",
    "amendment",
    "ruling",
    "operator",
    "person",
    "agent",
    "harness",
    "hook",
    "prompt",
    "context",
    "briefing",
    "summary",
    "vision",
    "todo",
    "record",
    "journal",
    "slug",
    "title",
    "body",
    "frontmatter",
    "header",
    "footer",
    "link",
    "anchor",
    "socket",
    "pipe",
    "signal",
    "process",
    "thread",
    "mutex",
    "channel",
    "event",
    "batch",
    "interval",
    "debounce",
    "burst",
    "stream",
    "buffer",
    "chunk",
    "byte",
    "frame",
    "terminal",
    "pane",
    "tab",
    "split",
    "close",
    "open",
    "start",
    "stop",
    "restart",
    "upgrade",
    "downgrade",
    "install",
    "remove",
    "prune",
    "archive",
    "forget",
    "remember",
    "recall",
    "share",
    "local",
    "live",
    "remote",
    "origin",
    "fetch",
    "push",
    "pull",
    "standing",
    "saving",
    "fsmonitor",
    "inotify",
    "fsevents",
    "kqueue",
];
