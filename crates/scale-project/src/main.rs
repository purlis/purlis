// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! `scale-project`: a developer's tool for the nightly scale run (#5497). Never shipped.
//!
//! ```text
//! scale-project generate --out <dir> [--seed N] [--at 2026-11-06T12:00:00] [--small]
//! scale-project index --project <dir>
//! scale-project look --project <dir> [--times N]
//! ```
//!
//! `generate` writes the project and prints one line of JSON saying what it holds. `index`
//! builds the project index as the session host's keeper does when it opens a project (ADR
//! 0079), and prints how long it took: a person's chats meet a project the host has indexed, so
//! the run's timings of the hook and of recall are taken on one. `look` times the project's git
//! standing as the app takes it (`planegit::standing`), after priming it as auto-save does when
//! the app opens the project (`planegit::prime`), and prints how long the prime took and the
//! samples as one line of JSON.

use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Instant;

use clap::{Parser, Subcommand};
use scale_project::{Plan, QUERY, Size, generate, persona_name, workspace_name};

#[derive(Parser)]
#[command(about = "A seeded project at the scale spec's target size, and timings on it")]
struct Args {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// DIAGNOSTIC (#5672), to be reverted: the syscalls of reading every memory.
    #[command(hide = true)]
    Readbench {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        mode: String,
    },
    /// Write a project into an empty (or absent) directory.
    Generate {
        #[arg(long)]
        out: PathBuf,
        #[arg(long, default_value_t = 5497)]
        seed: u64,
        /// The newest moment the project carries, UTC. The nightly run passes the time it
        /// runs: the app collects a dispatch record 30 days after it ended.
        #[arg(long, default_value = "2026-11-06T12:00:00")]
        at: chrono::NaiveDateTime,
        /// A few of everything instead of the target size.
        #[arg(long)]
        small: bool,
    },
    /// Build the project index, as the session host does when it opens the project.
    Index {
        #[arg(long)]
        project: PathBuf,
    },
    /// Prime the project's git standing as auto-save does, then time it as the app takes it.
    Look {
        #[arg(long)]
        project: PathBuf,
        #[arg(long, default_value_t = 5)]
        times: usize,
    },
}

fn main() -> ExitCode {
    match Args::parse().command {
        Command::Generate {
            out,
            seed,
            at,
            small,
        } => {
            let size = if small { Size::small() } else { Size::TARGET };
            if let Err(why) = std::fs::create_dir_all(&out) {
                eprintln!("scale-project: cannot make {}: {why}", out.display());
                return ExitCode::FAILURE;
            }
            let started = Instant::now();
            if let Err(why) = generate(&Plan { seed, size, at }, &out) {
                eprintln!("scale-project: the project is not written: {why}");
                return ExitCode::FAILURE;
            }
            println!(
                "{}",
                serde_json::json!({
                    "seed": seed,
                    "at": at.to_string(),
                    "workspaces": size.workspaces,
                    "personas": size.personas,
                    "workspace_memories": size.workspace_memories,
                    "persona_memories": size.persona_memories,
                    "shared_memories": size.shared_memories,
                    "todos": size.todos_per_workspace * size.workspaces,
                    "sessions": size.sessions_per_workspace * size.workspaces,
                    "dispatches": size.dispatches,
                    "query": QUERY,
                    "workspace": workspace_name(1),
                    "persona": persona_name(1),
                    "seconds": started.elapsed().as_secs_f64(),
                })
            );
            ExitCode::SUCCESS
        }
        Command::Index { project } => {
            let started = Instant::now();
            // What `projectindex::Keeper` does before it hears its first change: build (or
            // sweep) the index, then record the git standing the briefing reads in its place.
            let writer = match purlis_core::projectindex::Writer::open(&project) {
                Ok(writer) => writer,
                Err(why) => {
                    eprintln!("scale-project: {} is not indexed: {why}", project.display());
                    return ExitCode::FAILURE;
                }
            };
            let built = started.elapsed().as_secs_f64() * 1000.0;
            let standing = purlis_core::planegit::shared_standing(&project);
            if let Err(why) = writer.record_standing(&standing) {
                eprintln!("scale-project: the standing is not recorded: {why}");
                return ExitCode::FAILURE;
            }
            println!(
                "{}",
                serde_json::json!({
                    "built_ms": built,
                    "ms": started.elapsed().as_secs_f64() * 1000.0,
                })
            );
            ExitCode::SUCCESS
        }
        Command::Readbench { project, mode } => {
            readbench(&project, &mode);
            ExitCode::SUCCESS
        }
        Command::Look { project, times } => {
            // What the app does once as auto-save starts (`planegit::prime`), before any look
            // a person meets: without it every look reads the whole tree, a state the app
            // leaves within one status of opening the project (#5486). Timed on its own row.
            let started = Instant::now();
            purlis_core::planegit::prime(&project);
            let primed = started.elapsed().as_secs_f64() * 1000.0;
            let samples: Vec<f64> = (0..times.max(1))
                .map(|_| {
                    let started = Instant::now();
                    let standing = purlis_core::planegit::standing(&project);
                    std::hint::black_box(standing);
                    started.elapsed().as_secs_f64() * 1000.0
                })
                .collect();
            println!(
                "{}",
                serde_json::json!({ "primed_ms": primed, "samples_ms": samples })
            );
            ExitCode::SUCCESS
        }
    }
}

/// DIAGNOSTIC (#5672), to be reverted.
fn readbench(root: &std::path::Path, mode: &str) {
    use std::io::Read;
    fn walk(d: &std::path::Path, out: &mut Vec<PathBuf>) {
        for e in std::fs::read_dir(d).into_iter().flatten().flatten() {
            let p = e.path();
            if e.file_type().is_ok_and(|t| t.is_dir()) {
                walk(&p, out)
            } else if p.extension().is_some_and(|x| x == "md") {
                out.push(p)
            }
        }
    }
    let mut files = Vec::new();
    walk(root, &mut files);
    let started = Instant::now();
    let next = std::sync::atomic::AtomicUsize::new(0);
    let threads: usize = std::env::var("SCRATCH_THREADS").ok().and_then(|t| t.parse().ok()).unwrap_or_else(|| std::thread::available_parallelism().map_or(1, |n| n.get()));
    std::thread::scope(|s| {
        for _ in 0..threads {
            s.spawn(|| loop {
                let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                let Some(p) = files.get(i) else { break };
                match mode {
                    "stat" => {
                        let _ = std::fs::metadata(p);
                    }
                    "stat+read" => {
                        let _ = std::fs::metadata(p);
                        let _ = std::fs::read(p);
                    }
                    "open+read" => {
                        if let Ok(mut f) = std::fs::File::open(p) {
                            let _ = f.metadata();
                            let mut v = Vec::new();
                            let _ = f.read_to_end(&mut v);
                        }
                    }
                    _ => {}
                }
            });
        }
    });
    println!("{mode}: {} files in {:?} on {threads} threads", files.len(), started.elapsed());
}
