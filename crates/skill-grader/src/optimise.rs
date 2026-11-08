// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The description optimiser** (#5740 of spec #5714, decision Q5): a developer's tool that,
//! for one built-in, grades its description with the CI grader, asks Claude Code for a rewrite
//! from the trigger tests that failed, grades the rewrite on a copy of the plugin, and says both
//! scores side by side. It proposes; the developer decides. It writes the proposal into the
//! skill's `SKILL.md` only when asked ([`Optimise::write`]), and nothing else ever.
//!
//! It runs on the developer's own keys, handed to it in its environment by `purlis secret
//! exec`, and every key it holds is taken out of whatever it says or writes.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use purlis_skills::grade::ci::{self, Failing, Score, Setting, Why};

use crate::{Ask, Graded, grade, scrubbed, summary};

/// The longest description Codex reads whole, and the house style's limit (spec #5714, story
/// 28).
pub const MOST_CHARS: usize = 1024;

/// **What to optimise.**
pub struct Optimise<'a> {
    /// The grade to run, current and proposed: its harness, program, plugin, crowd and key.
    /// Its `skills` are not read; [`Optimise::skill`] is graded alone.
    pub ask: Ask<'a>,
    /// The built-in whose description is proposed.
    pub skill: &'a str,
    /// The Claude Code that writes the proposal.
    pub proposer: &'a Path,
    /// Its Anthropic key: none, or blank, starts nothing.
    pub proposer_key: Option<&'a str>,
    /// Whether to put the proposal in the skill's `SKILL.md`.
    pub write: bool,
}

/// **What it came to**, each said in Markdown with no key in it.
#[derive(Debug)]
pub enum Optimised {
    /// Nothing was started, and why.
    Skipped(String),
    /// The current description was graded and nothing failed, so nothing was proposed.
    Nothing(String),
    /// A proposal, both scores, and whether it was written.
    Proposed { said: String, written: bool },
}

/// **Grade `skill`'s description, propose another from its failing tests, grade that, and say
/// both.** An `Err` is a step that could not be done, said with no key in it.
pub fn optimise(asked: &Optimise<'_>) -> Result<Optimised, String> {
    let keys: Vec<&str> = [asked.ask.key, asked.proposer_key]
        .into_iter()
        .flatten()
        .map(str::trim)
        .filter(|key| !key.is_empty())
        .collect();
    let scrub = |text: &str| scrubbed(text, &keys);
    run(asked, &keys)
        .map(|optimised| match optimised {
            Optimised::Skipped(said) => Optimised::Skipped(scrub(&said)),
            Optimised::Nothing(said) => Optimised::Nothing(scrub(&said)),
            Optimised::Proposed { said, written } => Optimised::Proposed {
                said: scrub(&said),
                written,
            },
        })
        .map_err(|why| scrub(&why))
}

fn run(asked: &Optimise<'_>, keys: &[&str]) -> Result<Optimised, String> {
    let ask = &asked.ask;
    let skill = asked.skill;
    if ask.key.is_none_or(|key| key.trim().is_empty()) {
        return Ok(Optimised::Skipped(no_key(crate::key_name(ask.harness))));
    }
    let Some(proposer_key) = asked.proposer_key.filter(|key| !key.trim().is_empty()) else {
        return Ok(Optimised::Skipped(no_key("ANTHROPIC_API_KEY")));
    };
    let skill_md = ask.plugin.join("skills").join(skill).join("SKILL.md");
    let text = std::fs::read_to_string(&skill_md)
        .map_err(|why| format!("{} could not be read: {why}", skill_md.display()))?;
    let current = description_of(&text)
        .ok_or_else(|| format!("{} has no description to improve", skill_md.display()))?;

    let now = graded(ask, ask.plugin, ask.records.join("current"), skill)?;
    if crowded_failing(&now.score).next().is_none() {
        return Ok(Optimised::Nothing(format!(
            "{skill} on {}: every trigger test passes among the crowd, so there is nothing to \
             propose.\n\n{}",
            now.ran, now.verdict
        )));
    }

    let built_in = crate::launch::graded(&ask.plugin.join("skills"), skill);
    let sources = built_in
        .sources
        .as_deref()
        .and_then(|text| ci::sources_of(text, skill, &built_in.tests).ok());
    let reply = propose(
        asked.proposer,
        proposer_key,
        &ask.records,
        &brief(skill, &current, &now.score, sources.as_ref()),
    )?;
    // What a written description must be: one line, and nothing a key was.
    let proposal = one_line(&scrubbed(&reply, keys));
    if proposal.is_empty() {
        return Err("the proposer answered with no description".to_owned());
    }
    let rewritten = purlis_skills::fence::with_keys(&text, &[("description", Some(&proposal))])
        .map_err(|why| format!("the proposal cannot be a description: {why}"))?;

    // The proposal graded on a copy of the plugin, so the plugin itself is untouched.
    let copy = ask.records.with_extension("proposed-plugin");
    let _ = std::fs::remove_dir_all(&copy);
    copy_dir(ask.plugin, &copy)
        .map_err(|why| format!("the plugin could not be copied to grade the proposal: {why}"))?;
    let proposed = std::fs::write(copy.join("skills").join(skill).join("SKILL.md"), &rewritten)
        .map_err(|why| format!("the proposal could not be put in the copy: {why}"))
        .and_then(|()| graded(ask, &copy, ask.records.join("proposed"), skill));
    let _ = std::fs::remove_dir_all(&copy);
    let proposed = proposed?;

    let written = if asked.write {
        std::fs::write(&skill_md, &rewritten)
            .map_err(|why| format!("{} was not written: {why}", skill_md.display()))?;
        true
    } else {
        false
    };
    Ok(Optimised::Proposed {
        said: said(
            skill,
            &current,
            &proposal,
            &now,
            &proposed,
            written.then_some(&skill_md),
        ),
        written,
    })
}

fn no_key(name: &str) -> String {
    format!(
        "The description optimiser started nothing: no `{name}` in its environment. Hand it your \
         own key with `purlis secret exec <vault> --env {name}=<key> -- skill-grader optimise …`, \
         never as an argument."
    )
}

/// One graded description: its score, its verdict in words, and what it ran as.
struct Scored {
    score: ci::Scored,
    verdict: String,
    ran: String,
}

/// `skill` graded with the plugin at `plugin`, its runs kept under `records`.
fn graded(ask: &Ask<'_>, plugin: &Path, records: PathBuf, skill: &str) -> Result<Scored, String> {
    let one = Ask {
        harness: ask.harness,
        program: ask.program,
        purlis: ask.purlis,
        plugin,
        crowd: ask.crowd,
        cwd: ask.cwd,
        pinned: ask.pinned,
        skills: vec![skill.to_owned()],
        key: ask.key,
        records,
        parallel: ask.parallel,
    };
    match grade(&one) {
        Graded::Skipped(why) => Err(why),
        Graded::Done { card, .. } => {
            let on = &card.on;
            let ran = format!("{} {} ({})", on.harness.shown(), on.version, on.model);
            match card.skills.first() {
                Some(graded @ ci::Graded::Scored(score)) => Ok(Scored {
                    score: score.clone(),
                    verdict: graded.verdict(on),
                    ran,
                }),
                Some(graded @ ci::Graded::NotScored { .. }) => Err(graded.verdict(on)),
                None => Err(format!("{skill} was not graded")),
            }
        }
    }
}

/// **What the proposer is asked**: the skill, its description, the tests that failed and how,
/// its neighbours, and the house style.
fn brief(skill: &str, current: &str, score: &ci::Scored, sources: Option<&ci::Sources>) -> String {
    let mut out = format!(
        "You rewrite the description of {skill}, a skill built into purlis. A coding agent \
         reads the description in its skill listing, among about forty other skills, and \
         decides from it alone whether to start the skill.\n\nIts description now:\n\n\
         {current}\n\nIn the trigger tests it failed:\n\n"
    );
    for prompt in crowded_failing(score) {
        let belongs_to = sources
            .and_then(|sources| sources.of(&prompt.query))
            .and_then(|source| source.belongs_to.as_deref());
        let how = match (prompt.why, belongs_to) {
            (Why::Missed, _) => "should have started it and did not".to_owned(),
            (Why::FalseTrigger, Some(owner)) => format!("started it, and belongs to {owner}"),
            (Why::FalseTrigger, None) => "started it, and should not have".to_owned(),
            (Why::NotRun, _) => "no run of it said what it started".to_owned(),
        };
        let _ = writeln!(out, "- \"{}\": {how}", prompt.query);
    }
    let neighbours = sources.map_or(&[][..], |sources| &sources.neighbours[..]);
    if !neighbours.is_empty() {
        let _ = write!(
            out,
            "\nIts neighbours, the skills it is most often confused with: {}.\n",
            neighbours.join(", ")
        );
    }
    let _ = write!(
        out,
        "\nWrite a new description in purlis's house style: open with the situation it is for, \
         in the words people use; say the word purlis; name the phrases people use; give one \
         \"Not for X — that is purlis-Y\" clause per neighbour; at most 1,024 characters, on one \
         line. Keep what the current description gets right. Answer with the description alone: \
         no quotes, no heading, nothing before or after it."
    );
    out
}

/// The prompts against `score` among the crowd: what its bar holds it to.
fn crowded_failing(score: &ci::Scored) -> impl Iterator<Item = &Failing> {
    score
        .failing
        .iter()
        .filter(|failing| failing.setting == Setting::Crowded)
}

/// **The proposer's answer**: Claude Code started headless with no tool, no plugin and no MCP
/// server, its key in the environment alone, and the brief its one word.
fn propose(program: &Path, key: &str, records: &Path, brief: &str) -> Result<String, String> {
    std::fs::create_dir_all(records)
        .map_err(|why| format!("{} could not be made: {why}", records.display()))?;
    let output = Command::new(program)
        .args(["-p", "--output-format", "json", "--max-turns", "1"])
        .args(["--tools", "", "--strict-mcp-config"])
        .arg(brief)
        .env("ANTHROPIC_API_KEY", key)
        .current_dir(records)
        .stdin(Stdio::null())
        .stderr(Stdio::inherit())
        .output()
        .map_err(|why| format!("{} did not start: {why}", program.display()))?;
    let out = String::from_utf8_lossy(&output.stdout);
    let result = out
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .find(|event| event["type"] == "result");
    match result {
        Some(event) if event["is_error"] != true => event["result"]
            .as_str()
            .map(str::to_owned)
            .ok_or_else(|| "the proposer's result said no text".to_owned()),
        Some(event) => Err(format!(
            "the proposer ended in an error: {}",
            event["result"].as_str().unwrap_or("it said no more")
        )),
        None => Err(format!(
            "the proposer gave no result (it exited {})",
            output.status
        )),
    }
}

/// `text` as one line, its spaces collapsed and any quotes around it taken off.
fn one_line(text: &str) -> String {
    let line = text.split_whitespace().collect::<Vec<_>>().join(" ");
    line.strip_prefix('"')
        .and_then(|inner| inner.strip_suffix('"'))
        .unwrap_or(&line)
        .trim()
        .to_owned()
}

fn description_of(text: &str) -> Option<String> {
    purlis_skills::frontmatter(text)
        .into_iter()
        .find(|(key, _)| key == "description")
        .map(|(_, value)| value)
        .filter(|value| !value.is_empty())
}

/// `from`'s files and folders, copied into `to`.
fn copy_dir(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let into = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir(&entry.path(), &into)?;
        } else {
            std::fs::copy(entry.path(), into)?;
        }
    }
    Ok(())
}

/// Where the proposal breaks the house style a machine can check.
fn breaks_style(proposal: &str) -> Vec<String> {
    let mut breaks = Vec::new();
    if !proposal.to_lowercase().contains("purlis") {
        breaks.push("it does not say the word purlis".to_owned());
    }
    let chars = proposal.chars().count();
    if chars > MOST_CHARS {
        breaks.push(format!("it is {chars} characters, past {MOST_CHARS}"));
    }
    breaks
}

/// **Both scores, side by side**, under the two descriptions.
fn said(
    skill: &str,
    current: &str,
    proposal: &str,
    now: &Scored,
    proposed: &Scored,
    written: Option<&PathBuf>,
) -> String {
    let mut out = format!(
        "### {skill} on {}: a proposed description\n\nNow:\n\n> {current}\n\nProposed:\n\n> \
         {proposal}\n\n",
        now.ran
    );
    let breaks = breaks_style(proposal);
    if !breaks.is_empty() {
        let _ = writeln!(
            out,
            "The proposal breaks the house style: {}.\n",
            breaks.join("; ")
        );
    }
    out.push_str(
        "| listing | recall now | recall proposed | false triggers now | false triggers \
         proposed |\n|---|---|---|---|---|\n",
    );
    let row = |out: &mut String, word: &str, a: &Score, b: &Score| {
        let _ = writeln!(
            out,
            "| {word} | {} | {} | {} | {} |",
            summary::rate(a.recall),
            summary::rate(b.recall),
            summary::rate(a.false_triggers),
            summary::rate(b.false_triggers),
        );
    };
    if let (Some(a), Some(b)) = (&now.score.crowded, &proposed.score.crowded) {
        row(&mut out, "crowded", a, b);
    }
    if let (Some(a), Some(b)) = (&now.score.bare, &proposed.score.bare) {
        row(&mut out, "bare", a, b);
    }
    let _ = write!(
        out,
        "\nNow:\n\n{}\nProposed:\n\n{}\n",
        now.verdict, proposed.verdict
    );
    match written {
        Some(file) => {
            let _ = writeln!(out, "Written to {}.", file.display());
        }
        None => out.push_str(
            "Nothing was written. To keep the proposal, put it in the skill's SKILL.md, or run \
             again with `--write`, which writes the proposal that run makes.\n",
        ),
    }
    out
}
