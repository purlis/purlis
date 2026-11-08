// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A built-in skill's sources file** (spec #5714): `evals/sources.json`, kept beside Skill
//! studio's unchanged `evals/triggers.json`. It holds the skill's trigger bar, its neighbours,
//! and a note for each of its prompts saying where it came from; a prompt that should not start
//! the skill may name the neighbour it belongs to.

/// The sources file in a skill's folder, `/` between the parts.
pub const SOURCES_PATH: &str = "evals/sources.json";

/// **A skill's trigger bar** (ADR 0105 as amended 2026-11-08).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Tier {
    /// Where a miss leaks a secret, loses work or loses context: at least 95% recall, at most 5%
    /// false triggers.
    Strict,
    /// The rest: at least 85% recall, at most 5% false triggers.
    Standard,
}

/// **What a tier holds a skill to**, as shares of one.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Bar {
    pub recall: f64,
    pub false_triggers: f64,
}

impl Tier {
    /// The bar it holds a skill to.
    pub fn bar(self) -> Bar {
        match self {
            Self::Strict => Bar {
                recall: 0.95,
                false_triggers: 0.05,
            },
            Self::Standard => Bar {
                recall: 0.85,
                false_triggers: 0.05,
            },
        }
    }

    /// Its word, as the sources file spells it.
    pub fn word(self) -> &'static str {
        match self {
            Self::Strict => "strict",
            Self::Standard => "standard",
        }
    }
}

/// **A sources file, read whole.**
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sources {
    pub tier: Tier,
    pub neighbours: Vec<String>,
    pub prompts: Vec<Source>,
}

/// One prompt's source note, and the neighbour it belongs to where it is one of theirs.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Source {
    pub query: String,
    pub source: String,
    #[serde(default)]
    pub belongs_to: Option<String>,
}

/// The file as it is written: every key optional, so a missing one is said by name.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Written {
    tier: Option<Tier>,
    #[serde(default)]
    neighbours: Vec<String>,
    #[serde(default)]
    prompts: Vec<Source>,
}

impl Sources {
    /// The source of `query`, where the file has one.
    pub fn of(&self, query: &str) -> Option<&Source> {
        self.prompts.iter().find(|source| source.query == query)
    }
}

/// **The sources file `text` of the skill `skill`, whose trigger tests are `triggers`**, or a
/// sentence saying what is wrong with it. It is refused whole: a file with no tier, a prompt
/// with no source or two, a source for no prompt, an empty note, or a prompt said to belong to a
/// skill that is not one of the neighbours, or that the skill itself should start.
pub fn sources_of(
    text: &str,
    skill: &str,
    triggers: &[crate::grade::TriggerTest],
) -> Result<Sources, String> {
    let written: Written =
        serde_json::from_str(text).map_err(|why| format!("{SOURCES_PATH} does not read: {why}"))?;
    let tier = written
        .tier
        .ok_or_else(|| format!("{SOURCES_PATH} names no tier: say \"strict\" or \"standard\""))?;
    let sources = Sources {
        tier,
        neighbours: written.neighbours,
        prompts: written.prompts,
    };
    for neighbour in &sources.neighbours {
        if neighbour == skill {
            return Err(format!("{SOURCES_PATH} names {skill} as its own neighbour"));
        }
    }
    for test in triggers {
        let Some(source) = sources.of(&test.query) else {
            return Err(format!(
                "{SOURCES_PATH} has no source for the prompt \"{}\"",
                test.query
            ));
        };
        if source.source.trim().is_empty() {
            return Err(format!(
                "{SOURCES_PATH} says nothing of where \"{}\" came from",
                test.query
            ));
        }
        if let Some(owner) = &source.belongs_to {
            if test.should_trigger {
                return Err(format!(
                    "{SOURCES_PATH} says \"{}\" belongs to {owner}, but {skill} should start on it",
                    test.query
                ));
            }
            if !sources.neighbours.contains(owner) {
                return Err(format!(
                    "{SOURCES_PATH} says \"{}\" belongs to {owner}, which is not one of its \
                     neighbours",
                    test.query
                ));
            }
        }
    }
    for (at, source) in sources.prompts.iter().enumerate() {
        if sources.prompts[..at]
            .iter()
            .any(|earlier| earlier.query == source.query)
        {
            return Err(format!(
                "{SOURCES_PATH} has two sources for \"{}\"",
                source.query
            ));
        }
        if !triggers.iter().any(|test| test.query == source.query) {
            return Err(format!(
                "{SOURCES_PATH} has a source for \"{}\", which is no trigger test",
                source.query
            ));
        }
    }
    Ok(sources)
}
