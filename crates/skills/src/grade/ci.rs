// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The CI grader's scoring** (#5726, spec #5714; ADR 0105 as amended 2026-11-08): recorded
//! machine-readable output of Claude Code, Codex and opencode in, a score per built-in skill and
//! its trigger bar out.
//!
//! It launches nothing. Whatever ran a harness hands it what the harness printed in its print
//! mode, one [`Run`] per chat, and it reads which skills each chat started ([`started`]) by §2's
//! routes: the harness's own Skill tool, a read of a briefed `SKILL.md` on Codex, or purlis's
//! `skill_load`, a plugin's namespace no part of the name. Only tool calls count, never text the
//! model printed. It then scores each built-in skill it is given ([`score`]) against the trigger
//! bar its `evals/sources.json` names, as `docs/ci-grader.md` says.

mod output;
mod sources;

pub use output::{Harness, Listing, Started, Unread, started};
pub use sources::{Bar, SOURCES_PATH, Source, Sources, Tier, sources_of};

use super::{TRIGGER_THRESHOLD, TriggerTest};

/// **A built-in skill to score**: its name, its trigger tests and its sources file, as their
/// texts, the sources file `None` where the skill has none.
#[derive(Debug, Clone, Copy)]
pub struct BuiltIn<'a> {
    pub name: &'a str,
    pub triggers: &'a str,
    pub sources: Option<&'a str>,
}

/// **Whether a run had the crowd fixture in its listing.**
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Setting {
    /// Among the crowd fixture: what a skill is held to its bar by.
    Crowded,
    /// purlis's skills alone, reported beside the crowded run, never instead of it.
    Bare,
}

impl Setting {
    /// Its word.
    pub fn word(self) -> &'static str {
        match self {
            Self::Crowded => "crowded",
            Self::Bare => "bare",
        }
    }
}

/// **One chat the CI grader ran**: its prompt, its setting, and what its output read as.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Run {
    pub setting: Setting,
    pub query: String,
    pub read: Result<Started, Unread>,
}

/// **What the runs ran on**: the harness, its version and the model, as whatever ran them says.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct On {
    pub harness: Harness,
    pub version: String,
    pub model: String,
}

/// **A share**: of `of` prompts, the `hits`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Ratio {
    pub hits: u32,
    pub of: u32,
}

impl Ratio {
    /// `hits / of`, or none where there is nothing to share.
    pub fn share(self) -> Option<f64> {
        (self.of > 0).then(|| f64::from(self.hits) / f64::from(self.of))
    }
}

/// **A skill's score in one setting.**
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Score {
    /// Of its prompts that should start it and ran, the ones that did.
    pub recall: Ratio,
    /// Of the prompts that ran and should not start it, the ones that did: its own, and every
    /// other scored built-in's that are not its.
    pub false_triggers: Ratio,
}

/// **Why a prompt counts against a skill.**
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Why {
    /// It should have started the skill, and did not in half its runs or more.
    Missed,
    /// It should not have started the skill, and did in half its runs or more.
    FalseTrigger,
    /// None of its runs read: nothing says what it starts.
    NotRun,
}

/// **A prompt that counts against a skill**, with what its runs started instead.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Failing {
    pub query: String,
    pub setting: Setting,
    pub why: Why,
    /// The runs that read, and of them, the ones that started the skill.
    pub runs: u32,
    pub started_it: u32,
    /// The other skills half its runs or more started.
    pub started_instead: Vec<String>,
}

/// **A skill scored against its bar.**
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Scored {
    pub skill: String,
    pub tier: Tier,
    pub bar: Bar,
    /// Its score among the crowd fixture, and without it: none for a setting no chat ran in.
    pub crowded: Option<Score>,
    pub bare: Option<Score>,
    /// Whether its crowded score meets its bar, every prompt of it run: never without a
    /// crowded run.
    pub meets_its_bar: bool,
    /// The prompts against it, crowded first.
    pub failing: Vec<Failing>,
}

/// **One built-in skill's result.**
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "is", rename_all = "kebab-case")]
pub enum Graded {
    Scored(Scored),
    /// It could not be scored, and why: no tier, a sources file that does not read, trigger
    /// tests that do not.
    NotScored {
        skill: String,
        why: String,
    },
}

impl Graded {
    /// The skill it is of.
    pub fn skill(&self) -> &str {
        match self {
            Self::Scored(scored) => &scored.skill,
            Self::NotScored { skill, .. } => skill,
        }
    }

    /// **Its verdict, in words**: what it ran on, its bar and whether it meets it, its score in
    /// each setting, and each prompt that counts against it. A line each, for a CI run's summary
    /// and a nightly's issue.
    pub fn verdict(&self, on: &On) -> String {
        let head = format!(
            "{} on {} {} ({})",
            self.skill(),
            on.harness.shown(),
            on.version,
            on.model
        );
        let scored = match self {
            Self::NotScored { why, .. } => return format!("{head}: not scored, as {why}.\n"),
            Self::Scored(scored) => scored,
        };
        let meets = if scored.meets_its_bar {
            "meets"
        } else {
            "below"
        };
        let mut text = format!(
            "{head}: {meets} its {} bar, recall at least {} and false triggers at most {}.\n",
            scored.tier.word(),
            percent(scored.bar.recall),
            percent(scored.bar.false_triggers),
        );
        for (setting, score) in [
            (Setting::Crowded, scored.crowded),
            (Setting::Bare, scored.bare),
        ] {
            let shown = match setting {
                Setting::Crowded => "Crowded",
                Setting::Bare => "Bare",
            };
            match score {
                None => text.push_str(&format!("{shown}: not run.\n")),
                Some(score) => text.push_str(&format!(
                    "{shown}: recall {}, false triggers {}.\n",
                    ratio(score.recall),
                    ratio(score.false_triggers)
                )),
            }
        }
        if !scored.failing.is_empty() {
            text.push_str("Failing prompts:\n");
        }
        for failing in &scored.failing {
            let why = match failing.why {
                Why::Missed => "missed",
                Why::FalseTrigger => "false trigger",
                Why::NotRun => "not run",
            };
            let mut line = format!("- {}, {why}: \"{}\"", failing.setting.word(), failing.query);
            if failing.runs > 0 {
                line.push_str(&format!(
                    " (started it in {} of {} runs",
                    failing.started_it, failing.runs
                ));
                if !failing.started_instead.is_empty() {
                    line.push_str(&format!(
                        "; started {} instead",
                        failing.started_instead.join(", ")
                    ));
                }
                line.push(')');
            }
            line.push('\n');
            text.push_str(&line);
        }
        text
    }
}

/// `share` as a whole percent.
fn percent(share: f64) -> String {
    format!("{:.0}%", share * 100.0)
}

/// `ratio` as a percent and its counts, `50% (1 of 2)`; `none (0 of 0)` where nothing counted.
fn ratio(ratio: Ratio) -> String {
    match ratio.share() {
        Some(share) => format!("{} ({} of {})", percent(share), ratio.hits, ratio.of),
        None => "none (0 of 0)".to_owned(),
    }
}

/// **The CI grader's report**: what it ran on, and each built-in's result, in the order given.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Report {
    pub on: On,
    pub skills: Vec<Graded>,
}

impl Report {
    /// **Whether it passes a CI check**: it scored at least one built-in, and every built-in it
    /// was given was scored and meets its bar.
    pub fn passes(&self) -> bool {
        !self.skills.is_empty()
            && self
                .skills
                .iter()
                .all(|graded| matches!(graded, Graded::Scored(scored) if scored.meets_its_bar))
    }
}

/// A prompt of a scored skill: whom it belongs to, if anyone among the built-ins.
struct Prompt<'a> {
    query: &'a str,
    owner: Option<&'a str>,
}

/// **Scores `built_ins` from `runs`**, on what `on` says. Each prompt's runs are counted as a
/// grade's are: a skill started on it where half the runs that read or more started it. A
/// built-in that starts on a prompt that is not its own, whoever's it is, has a false trigger;
/// the skill it belongs to, where it should start it, has a miss.
pub fn score(on: On, built_ins: &[BuiltIn<'_>], runs: &[Run]) -> Report {
    let read: Vec<Result<Readable<'_>, Graded>> = built_ins.iter().map(read_one).collect();
    let prompts: Vec<Prompt<'_>> = read
        .iter()
        .flatten()
        .flat_map(
            |Readable {
                 skill,
                 tests,
                 sources,
             }| {
                tests.iter().map(move |test| Prompt {
                    query: &test.query,
                    owner: if test.should_trigger {
                        Some(*skill)
                    } else {
                        sources
                            .of(&test.query)
                            .and_then(|source| source.belongs_to.as_deref())
                    },
                })
            },
        )
        .collect();
    let skills = read
        .iter()
        .map(|one| match one {
            Err(not_scored) => not_scored.clone(),
            Ok(one) => Graded::Scored(scored(
                one.skill,
                &one.tests,
                one.sources.tier,
                &prompts,
                runs,
            )),
        })
        .collect();
    Report { on, skills }
}

/// A built-in whose tests and sources read.
struct Readable<'a> {
    skill: &'a str,
    tests: Vec<TriggerTest>,
    sources: Sources,
}

/// A built-in's tests and sources, read, or why it is not scored.
fn read_one<'a>(built_in: &BuiltIn<'a>) -> Result<Readable<'a>, Graded> {
    let not_scored = |why: String| Graded::NotScored {
        skill: built_in.name.to_owned(),
        why,
    };
    let none = super::Fixtures {
        triggers: super::Read::Absent,
        outcomes: super::Read::Absent,
    };
    let tests =
        match super::fixtures_over(none, Some(built_in.triggers), None, built_in.name).triggers {
            super::Read::Read(tests) => tests,
            super::Read::Refused(why) => return Err(not_scored(why.terminal())),
            super::Read::Absent => return Err(not_scored("it has no trigger tests".to_owned())),
        };
    let Some(text) = built_in.sources else {
        return Err(not_scored(format!(
            "it has no {SOURCES_PATH}, so no tier to hold it to"
        )));
    };
    let sources = sources_of(text, built_in.name, &tests).map_err(not_scored)?;
    Ok(Readable {
        skill: built_in.name,
        tests,
        sources,
    })
}

/// How one prompt's runs in one setting came out: the runs that read, and how many of them
/// started each skill.
struct Tally {
    runs: u32,
    started: Vec<(String, u32)>,
}

impl Tally {
    fn of(query: &str, setting: Setting, runs: &[Run]) -> Self {
        let mut tally = Tally {
            runs: 0,
            started: Vec::new(),
        };
        let read = runs
            .iter()
            .filter(|run| run.setting == setting && run.query == query)
            .filter_map(|run| run.read.as_ref().ok());
        for started in read {
            tally.runs = tally.runs.saturating_add(1);
            for skill in &started.skills {
                match tally.started.iter_mut().find(|(name, _)| name == skill) {
                    Some((_, count)) => *count = count.saturating_add(1),
                    None => tally.started.push((skill.clone(), 1)),
                }
            }
        }
        tally
    }

    fn count(&self, skill: &str) -> u32 {
        self.started
            .iter()
            .find(|(name, _)| name == skill)
            .map_or(0, |(_, count)| *count)
    }

    /// Whether half its runs or more started `skill`, as a grade's trigger test counts it.
    fn started(&self, skill: &str) -> bool {
        self.runs > 0 && f64::from(self.count(skill)) / f64::from(self.runs) >= TRIGGER_THRESHOLD
    }

    fn instead_of(&self, skill: &str) -> Vec<String> {
        self.started
            .iter()
            .map(|(name, _)| name)
            .filter(|name| *name != skill && self.started(name))
            .cloned()
            .collect()
    }
}

/// `skill` scored over every scored built-in's `prompts`, its own `tests` among them.
fn scored(
    skill: &str,
    tests: &[TriggerTest],
    tier: Tier,
    prompts: &[Prompt<'_>],
    runs: &[Run],
) -> Scored {
    let mut failing = Vec::new();
    let mut score_in = |setting: Setting| {
        if !runs.iter().any(|run| run.setting == setting) {
            return None;
        }
        let mut score = Score {
            recall: Ratio { hits: 0, of: 0 },
            false_triggers: Ratio { hits: 0, of: 0 },
        };
        let mut seen: Vec<&str> = Vec::new();
        for prompt in prompts {
            if seen.contains(&prompt.query) {
                continue;
            }
            seen.push(prompt.query);
            let own = tests.iter().any(|test| test.query == prompt.query);
            if !own && prompt.owner == Some(skill) {
                continue;
            }
            let tally = Tally::of(prompt.query, setting, runs);
            let should = prompt.owner == Some(skill);
            let mut against = |why: Why| {
                failing.push(Failing {
                    query: prompt.query.to_owned(),
                    setting,
                    why,
                    runs: tally.runs,
                    started_it: tally.count(skill),
                    started_instead: tally.instead_of(skill),
                });
            };
            if tally.runs == 0 {
                if own {
                    against(Why::NotRun);
                }
                continue;
            }
            let started = tally.started(skill);
            if should {
                score.recall.of = score.recall.of.saturating_add(1);
                if started {
                    score.recall.hits = score.recall.hits.saturating_add(1);
                } else {
                    against(Why::Missed);
                }
            } else {
                score.false_triggers.of = score.false_triggers.of.saturating_add(1);
                if started {
                    score.false_triggers.hits = score.false_triggers.hits.saturating_add(1);
                    against(Why::FalseTrigger);
                }
            }
        }
        Some(score)
    };
    let crowded = score_in(Setting::Crowded);
    let bare = score_in(Setting::Bare);
    let bar = tier.bar();
    let not_run = failing
        .iter()
        .any(|failing| failing.setting == Setting::Crowded && failing.why == Why::NotRun);
    let meets_its_bar = !not_run
        && crowded.is_some_and(|crowded| {
            crowded
                .recall
                .share()
                .is_some_and(|share| share >= bar.recall)
                && crowded
                    .false_triggers
                    .share()
                    .is_none_or(|share| share <= bar.false_triggers)
        });
    Scored {
        skill: skill.to_owned(),
        tier,
        bar,
        crowded,
        bare,
        meets_its_bar,
        failing,
    }
}

#[cfg(test)]
mod tests;
