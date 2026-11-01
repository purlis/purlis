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
    /// Set one of a pool's limits, or return it to purlis's default with `--reset`.
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
        /// Return it to following purlis's default.
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
            crate::voice::ok(saved_said(pool, limit, value));
            Ok(0)
        }
    }
}

/// What a save says.
fn saved_said(pool: &str, limit: &Limit, value: Option<u64>) -> String {
    let now = value.map_or_else(
        || {
            format!(
                "back to purlis's default, {}",
                limit.unit.said(limit.default)
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
            .map_or_else(String::new, poollimits::RefusedValue::said);
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
                "default": limit.default,
                "set": held.chosen.get(limit.id),
                "inForce": held.chosen.in_force(limit.id),
                "applies": limit.takes_effect.said(),
                "refused": held.refused.iter().find(|one| one.limit == limit.id)
                    .map(purlis_core::poollimits::RefusedValue::said),
            })
        })
        .collect();
    serde_json::json!({ "limits": rows })
}
