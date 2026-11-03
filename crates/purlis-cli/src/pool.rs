// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **`purlis pool`** (#4284, spec #4275): a pool's limits, for a script or an advanced person,
//! with the names and the checks the window and the file have, all read from
//! [`purlis_core::poollimits`].
//!
//! **In a chat it changes nothing** (Q6, the person's ruling of 2026-11-01 on PC-1): the value is
//! handed to the app as a proposal, which only the person applies. A chat that can't reach the
//! app is refused, and nothing here falls back to writing the file.

use clap::Subcommand;
use purlis_core::poollimits::{self, Limit};
use purlis_core::poolsetup::{
    self,
    asked::{Asked, Created},
};
use purlis_core::runners;

#[derive(Subcommand)]
pub enum PoolCommand {
    /// List a pool's limits: each one's key, its value, and whether the value is yours.
    Limits {
        /// The pool's name.
        pool: String,
        /// One JSON object for a script.
        #[arg(long)]
        json: bool,
    },
    /// Add a Docker pool on this machine: its runner is made from the image and paired here.
    ///
    /// This is what Pool setup's Create button runs, and Pool setup picks it up. A team's own
    /// image is checked against the runner image contract first.
    Add {
        /// The pool's name: lower-case letters, digits and dashes.
        name: String,
        /// The runner image, by its digest. See what this does in Pool setup names purlis's own.
        #[arg(long)]
        image: String,
        /// How many CPUs its runner may use, to the thousandth.
        #[arg(long)]
        cpus: Option<String>,
        /// The memory its runner may use, in MiB.
        #[arg(long)]
        memory_mib: Option<u32>,
        /// How many processes its runner may run at once.
        #[arg(long)]
        processes: Option<u32>,
        /// The size of its runner's /tmp, in MiB.
        #[arg(long)]
        tmp_mib: Option<u32>,
        /// How long its runner waits without a link before it suspends: 12h, 90m.
        #[arg(long)]
        suspend_after: Option<String>,
        /// The most frames a second its runner's browsers are watched at.
        #[arg(long)]
        frames: Option<u32>,
        /// The widest frame its runner's browsers are watched at, in pixels.
        #[arg(long)]
        width: Option<u32>,
    },
    /// Pool setup: each step's state and what to do next, for a pool or a new one. It only reads.
    Setup {
        /// The pool. Without it, a new Docker pool.
        name: Option<String>,
        /// One JSON object for a script.
        #[arg(long)]
        json: bool,
    },
    /// Set one of a pool's limits, or return it to the default with `--reset`.
    ///
    /// In a chat the app started, this writes nothing: the person is asked in the window, and
    /// only they apply it.
    Limit {
        /// The pool's name.
        pool: String,
        /// The limit's key, as `purlis pool limits` lists it.
        key: String,
        /// The new value, a whole number in the limit's unit.
        #[arg(allow_hyphen_values = true, required_unless_present = "reset")]
        value: Option<String>,
        /// Return it to following the default.
        #[arg(long, conflicts_with = "value")]
        reset: bool,
    },
}

/// Runs `command`, and says why not where it can't.
pub fn run(command: &PoolCommand) -> std::process::ExitCode {
    match answered(command) {
        Ok(code) => std::process::ExitCode::from(code),
        Err(why) => {
            crate::voice::err(why);
            std::process::ExitCode::FAILURE
        }
    }
}

fn answered(command: &PoolCommand) -> Result<u8, String> {
    let config = || {
        purlis_core::machine::config_root()
            .ok_or_else(|| "There is no config home on this machine to keep a pool in.".to_owned())
    };
    let in_a_chat = crate::gradelink::in_a_chat(&purlis_core::envvar::var);
    match command {
        // A chat doesn't reach this machine's Docker or a runner, so it asks nothing of them.
        PoolCommand::Setup { .. } if in_a_chat => Err(
            "Pool setup reads this machine's Docker and pools, which a chat doesn't reach. Run \
             purlis pool setup in your own terminal."
                .to_owned(),
        ),
        PoolCommand::Add { .. } if in_a_chat => Err(
            "Adding a pool uses this machine's Docker, which a chat doesn't reach. Run it in your \
             own terminal, or use Pool setup in the window."
                .to_owned(),
        ),
        PoolCommand::Add {
            name,
            image,
            cpus,
            memory_mib,
            processes,
            tmp_mib,
            suspend_after,
            frames,
            width,
        } => {
            let asked = Asked {
                name: name.clone(),
                image: image.clone(),
                cpus: cpus.clone(),
                memory_mib: *memory_mib,
                processes: *processes,
                tmp_mib: *tmp_mib,
                suspend_after: suspend_after.clone(),
                frames: *frames,
                width: *width,
            };
            added(&config()?, &asked)
        }
        PoolCommand::Setup { name, json } => {
            crate::poolsetup::answered(name.as_deref(), *json)?;
            Ok(0)
        }
        // A chat can't read the pool's file, so it is told what can be asked for, and no value.
        PoolCommand::Limits { json, .. } if in_a_chat => {
            let held = runners::PoolLimitsHeld {
                chosen: poollimits::Chosen::default(),
                refused: Vec::new(),
                docker: false,
            };
            if *json {
                let mut said = limits_json(&held);
                said["defaultsOnly"] = true.into();
                if let Some(rows) = said["limits"].as_array_mut() {
                    for row in rows {
                        row["inForce"] = serde_json::Value::Null;
                    }
                }
                println!("{said}");
            } else {
                for line in limits_lines(&held) {
                    println!("{line}");
                }
                crate::voice::info(
                    "In a chat these are purlis's defaults, not this pool's values: only the \
                     person sees those, in the window.",
                );
            }
            Ok(0)
        }
        PoolCommand::Limits { pool, json } => {
            let held = runners::limits_held_of(&config()?, pool).map_err(|why| why.to_string())?;
            if *json {
                println!("{}", limits_json(&held));
            } else {
                for line in limits_lines(&held) {
                    println!("{line}");
                }
            }
            Ok(0)
        }
        PoolCommand::Limit {
            pool,
            key,
            value,
            reset: _,
        } => {
            // A limit that can't change says so, why, and what to do instead (#4281).
            if let Some(fixed) = poollimits::fixed().iter().find(|one| one.key == *key) {
                return Err(format!("{}: {}", fixed.plain, fixed.said()));
            }
            let limit = poollimits::of_key(key).ok_or_else(|| {
                format!(
                    "a pool has no limit named {}: `purlis pool limits <pool>` lists them",
                    purlis_core::shown::short(key)
                )
            })?;
            let value = value
                .as_deref()
                .map(|typed| limit.check_typed(typed, None))
                .transpose()?;
            if in_a_chat {
                return crate::brokered::propose_limit(pool, limit, value);
            }
            runners::set_pool_limit(&config()?, pool, limit.id, value)
                .map_err(|why| why.to_string())?;
            let chosen = runners::chosen_limits_of(&config()?, pool).unwrap_or_default();
            crate::voice::ok(saved_said(pool, limit, value, &chosen));
            Ok(0)
        }
    }
}

/// **Adds the pool `asked` asks for**, as Pool setup's button does.
fn added(config: &std::path::Path, asked: &Asked) -> Result<u8, String> {
    let docker = poolsetup::docker_here()
        .ok_or_else(|| "purlis found no docker on this machine.".to_owned())?;
    docker.answers().map_err(|why| why.0)?;
    let keyring = purlis_core::linkkey::this_builds_keyring(config);
    let desktop = purlis_core::linkkey::desktop(config, &*keyring)?.pinned();
    match poolsetup::asked::create(config, &docker, asked, &desktop)
        .map_err(|why| why.to_string())?
    {
        Created::Added => {
            crate::voice::ok(format!(
                "Pool {} is added, and its runner is made.",
                asked.name
            ));
            Ok(0)
        }
        Created::Breaks(problems) => Err(format!(
            "This image can't be a runner, and nothing was made:\n{}",
            problems.join("\n")
        )),
    }
}

/// What a save says.
fn saved_said(
    pool: &str,
    limit: &Limit,
    value: Option<u64>,
    chosen: &poollimits::Chosen,
) -> String {
    let now = value.map_or_else(
        || {
            format!(
                "back to the default, {}",
                limit.unit.said(chosen.default_of(limit.id))
            )
        },
        |value| limit.unit.said(value),
    );
    format!(
        "Pool {pool}: {} is {now}. {}",
        limit.plain,
        limit.takes_effect.said()
    )
}

/// The limits this kind of pool has, under `held`.
fn listed(held: &runners::PoolLimitsHeld) -> impl Iterator<Item = &'static Limit> + use<> {
    let docker = held.docker;
    let refused: Vec<_> = held.refused.iter().map(|one| one.limit).collect();
    // A value a Docker pool should never have held is still listed, so it can be reset.
    poollimits::all()
        .iter()
        .filter(move |limit| !(docker && limit.clusters_only) || refused.contains(&limit.id))
}

fn limits_lines(held: &runners::PoolLimitsHeld) -> Vec<String> {
    let mut lines = Vec::new();
    for limit in listed(held) {
        let value = held.chosen.in_force(limit.id);
        let whose = if held.chosen.get(limit.id).is_some() {
            "yours"
        } else {
            "default"
        };
        let refused = held
            .refused
            .iter()
            .find(|one| one.limit == limit.id)
            .map(poollimits::RefusedValue::said)
            .or_else(|| held.chosen.over_highest_said(limit.id))
            .unwrap_or_default();
        lines.push(format!(
            "{}\t{}\t{whose}\t{}\t{refused}",
            limit.key,
            limit.unit.said(value),
            limit.plain
        ));
    }
    lines
}

fn limits_json(held: &runners::PoolLimitsHeld) -> serde_json::Value {
    let rows: Vec<serde_json::Value> = listed(held)
        .map(|limit| {
            serde_json::json!({
                "key": limit.key,
                "plain": limit.plain,
                "unit": limit.unit.word(),
                "default": held.chosen.default_of(limit.id),
                "defaultByTeam": held.chosen.default_by_team(limit.id),
                "highest": held.chosen.highest(limit.id),
                "set": held.chosen.get(limit.id),
                "inForce": held.chosen.in_force(limit.id),
                "applies": limit.takes_effect.said(),
                "refused": held.refused.iter().find(|one| one.limit == limit.id)
                    .map(purlis_core::poollimits::RefusedValue::said)
                    .or_else(|| held.chosen.over_highest_said(limit.id)),
            })
        })
        .collect();
    serde_json::json!({ "limits": rows })
}
