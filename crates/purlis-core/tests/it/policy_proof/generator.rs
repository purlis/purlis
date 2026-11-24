// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Generated policies** (M75 slice 42, #7051; spec §14.6, §14.11): rules written from a seed,
//! every one inside the subset the load-time check admits, and the fixed list of cells they are
//! asked.
//!
//! The generator writes **only** what the check admits, and the check is not trusted to say so
//! for it: `the_generator_emits_only_subset_policies` loads every policy through the production
//! loader (`policy::load`, which runs the layering, `@id` and sandbox-subset checks) and fails on
//! the first problem. A rule outside the subset is the generator's bug, never a policy to skip.
//!
//! A run is a list of seeds ([`seeds`]): the three fixed ones on every pull request, and the
//! nightly's own, from `PURLIS_PROOF_SEEDS`, so a red night names the seed that replays it. The
//! order the cells are asked in is shuffled by the seed ([`shuffled`]), so an answer that
//! depends on what was asked before it shows up.

use proptest::prelude::*;
use proptest::test_runner::{RngAlgorithm, TestRng};

use super::{Cell, PolicyInput, RULES_HEADER};
use purlis_core::enforcement::Template;

/// The seeds every pull request runs, when `PURLIS_PROOF_SEEDS` names none.
pub const FIXED_SEEDS: [u64; 3] = [1, 7, 42];

/// The variable the nightly sets: a comma-separated list of seeds, each a `u64`.
pub const SEEDS_VARIABLE: &str = "PURLIS_PROOF_SEEDS";

/// How many policies a seed writes.
pub const POLICIES_PER_SEED: usize = 10;

/// **The seeds to run**: `PURLIS_PROOF_SEEDS` when it is set, else [`FIXED_SEEDS`].
pub fn seeds() -> Vec<u64> {
    seeds_from(std::env::var(SEEDS_VARIABLE).ok().as_deref())
}

/// [`seeds`] over a value: a list that is not wholly seeds panics, because a nightly that quietly
/// fell back to the fixed ones would prove nothing new and say it had.
pub fn seeds_from(value: Option<&str>) -> Vec<u64> {
    let Some(value) = value.filter(|v| !v.trim().is_empty()) else {
        return FIXED_SEEDS.to_vec();
    };
    value
        .split(',')
        .map(|seed| {
            seed.trim().parse::<u64>().unwrap_or_else(|_| {
                panic!("{SEEDS_VARIABLE} holds `{seed}`, which is not a u64 seed")
            })
        })
        .collect()
}

/// The generator's stream: all 64 bits of the seed reach the ChaCha key (its bytes four times
/// over), so two seeds never share a stream.
fn rng(seed: u64) -> TestRng {
    let bytes = seed.to_le_bytes();
    let key: Vec<u8> = bytes.iter().copied().cycle().take(32).collect();
    TestRng::from_seed(RngAlgorithm::ChaCha, &key)
}

fn pick(rng: &mut TestRng, below: usize) -> usize {
    (rng.next_u32() as usize) % below
}

/// **The cells in another order**, from the seed: a Fisher–Yates shuffle over the seeded stream.
pub fn shuffled<T>(seed: u64, mut items: Vec<T>) -> Vec<T> {
    let mut rng = rng(seed.wrapping_add(0x5a5a_5a5a_5a5a_5a5a));
    for at in (1..items.len()).rev() {
        items.swap(at, pick(&mut rng, at + 1));
    }
    items
}

/// One policy the generator wrote.
#[derive(Debug, Clone)]
pub struct Generated {
    /// The seed that wrote it.
    pub seed: u64,
    /// Its place among that seed's policies.
    pub index: usize,
    /// The rules, without the version line.
    pub rules: String,
}

impl Generated {
    /// The policy as a cell runs under it: `strict`, with these rules on top.
    pub fn input(&self) -> PolicyInput {
        PolicyInput::with_rules(Template::Strict, &self.rules)
    }

    /// The text a project would commit, with its version line.
    pub fn file_text(&self) -> String {
        format!("{RULES_HEADER}{}", self.rules)
    }
}

/// An action the generator writes rules about, and what its rules may say.
struct Kind {
    action: &'static str,
    family: &'static str,
    /// The resource's entity type and the cell's id.
    resource: (&'static str, &'static str),
    /// Other ids of that type a rule may name.
    others: &'static [&'static str],
    /// Conditions over what the action declares, each true for some cell and false for others.
    conditions: &'static [&'static str],
}

/// Conditions every action's context supports.
const COMMON: [&str; 6] = [
    "context.depth >= 3",
    "context.depth < 1",
    "context.sandbox == \"full\"",
    "context.sandbox != \"off\"",
    "context.attended",
    "context.harness_mode == \"default\"",
];

// No `dispatch-to`, `use` or any action whose shipped rule reads `principal.persona`: the oracle
// asks over no entities (slice 09), so that rule errs on the missing chat and the oracle refuses
// the cell. The family slices that give the oracle a world add their actions here.
const KINDS: [Kind; 8] = [
    Kind {
        action: "merge",
        family: "guards.merge",
        resource: ("Branch", "acme/web#main"),
        others: &["acme/web#dev", "acme/api#main"],
        conditions: &["context.via == \"hook\"", "context.via == \"cli\""],
    },
    Kind {
        action: "land",
        family: "guards.merge",
        resource: ("Repo", "acme/web"),
        others: &["acme/api"],
        conditions: &["context.via == \"hook\""],
    },
    Kind {
        action: "push",
        family: "guards.git",
        resource: ("Branch", "acme/web#main"),
        others: &["acme/web#dev"],
        conditions: &["context.force", "context.brokered", "context.amend"],
    },
    Kind {
        action: "release",
        family: "guards.release",
        resource: ("Repo", "acme/web"),
        others: &["acme/api"],
        conditions: &["context.via == \"hook\""],
    },
    Kind {
        action: "save",
        family: "guards.save",
        resource: ("Repo", "acme/web"),
        others: &["acme/api"],
        conditions: &["context.extent == \"push\"", "context.extent == \"commit\""],
    },
    Kind {
        action: "install",
        family: "skills",
        resource: ("Skill", "lint"),
        others: &["format"],
        conditions: &[],
    },
    // The sandbox's two actions say only what the subset admits (`sandbox_rules.rs`): a single
    // path or workspace subject (it refuses two conditions joined with `&&`).
    Kind {
        action: "read",
        family: "sandbox",
        resource: ("Path", "/plane/other/x"),
        others: &[],
        conditions: &[],
    },
    Kind {
        action: "write",
        family: "sandbox",
        resource: ("Path", "/plane/workspaces/qa/a/f"),
        others: &[],
        conditions: &[],
    },
];

/// The subjects a sandbox rule may name: each inside the subset (`sandbox_rules_tests.rs` holds
/// the ones it refuses).
const SANDBOX_SUBJECTS: [&str; 5] = [
    "resource in Path::\"/plane/workspaces/qa/a\"",
    "resource == Path::\"/plane/other/x\"",
    "resource in Workspace::\"billing\"",
    "resource has workspace && resource.workspace != principal.workspace",
    "true",
];

fn prefix(family: &str) -> String {
    format!("{}-", family.replace(['.', '_'], "-"))
}

fn rule(rng: &mut TestRng, n: usize) -> String {
    let kind = &KINDS[pick(rng, KINDS.len())];
    let permit = pick(rng, 3) == 0;
    let ask = !permit && pick(rng, 2) == 0;
    let body = if kind.family == "sandbox" {
        let subject = SANDBOX_SUBJECTS[pick(rng, SANDBOX_SUBJECTS.len())];
        format!("when {{ {subject} }}")
    } else {
        let mut conditions: Vec<String> = Vec::new();
        for _ in 0..pick(rng, 3) {
            let pool: Vec<&str> = COMMON
                .iter()
                .chain(kind.conditions.iter())
                .copied()
                .collect();
            conditions.push(pool[pick(rng, pool.len())].to_owned());
        }
        if pick(rng, 3) == 0 {
            let ids: Vec<&str> = std::iter::once(kind.resource.1)
                .chain(kind.others.iter().copied())
                .collect();
            conditions.push(format!(
                "resource == {}::\"{}\"",
                kind.resource.0,
                ids[pick(rng, ids.len())]
            ));
        }
        match (conditions.is_empty(), pick(rng, 4) == 0) {
            (true, _) => String::new(),
            (false, true) => format!("unless {{ {} }}", conditions.join(" && ")),
            (false, false) => format!("when {{ {} }}", conditions.join(" && ")),
        }
    };
    format!(
        "@id(\"{}gen-{n}\") @level(\"project\"){} {} (principal, action == Action::\"{}\", resource) {body};\n",
        prefix(kind.family),
        if ask { " @ask(\"you\")" } else { "" },
        if permit { "permit" } else { "forbid" },
        kind.action,
    )
}

/// **The policies `seed` writes**: [`POLICIES_PER_SEED`] of them, each of one to four rules.
pub fn generate(seed: u64) -> Vec<Generated> {
    let mut rng = rng(seed);
    (0..POLICIES_PER_SEED)
        .map(|index| {
            let rules: String = (0..1 + pick(&mut rng, 4))
                .map(|n| rule(&mut rng, n))
                .collect();
            Generated { seed, index, rules }
        })
        .collect()
}

fn base(sandbox: &str, attended: bool, depth: i64) -> serde_json::Value {
    serde_json::json!({
        "sandbox": sandbox,
        "attended": attended,
        "harness_mode": "default",
        "depth": depth,
    })
}

fn context_of(action: &str, sandbox: &str, attended: bool, depth: i64) -> serde_json::Value {
    let mut context = base(sandbox, attended, depth);
    let extra = match action {
        "merge" | "land" | "release" => serde_json::json!({"via": "hook"}),
        "push" => serde_json::json!({
            "force": depth >= 3, "skip-hooks": false, "brokered": attended, "amend": false,
        }),
        "save" => serde_json::json!({"extent": "push"}),
        _ => serde_json::json!({}),
    };
    for (key, value) in extra.as_object().expect("an object").clone() {
        context[key] = value;
    }
    context
}

/// **The fixed list of cells** every generated policy is asked: each action the generator writes
/// about, as a floor-level attended chat at the root and as a full-sandbox unattended chat three
/// deep, so the conditions on both sides of a rule are reached.
pub fn cells() -> Vec<Cell> {
    KINDS
        .iter()
        .flat_map(|kind| {
            [("floor", true, 0), ("full", false, 3)].into_iter().map(
                |(sandbox, attended, depth)| Cell {
                    policy: None,
                    action: kind.action,
                    resource: kind.resource,
                    context: context_of(kind.action, sandbox, attended, depth),
                    gap: None,
                },
            )
        })
        .collect()
}
