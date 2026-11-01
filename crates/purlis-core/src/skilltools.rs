// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The skill tools of purlis's MCP server** (SK-13, #2776; ADR 0063 as amended 2026-10-20):
//! `skill_list`, `skill_runs`, `skill_find` and `skill_load`, the same on every harness, and all a
//! harness on [`crate::skills::Route::Mcp`] has of its skills.
//!
//! They keep the chat tools' rules ([`crate::chattools`]): the chat's own place, resolved at each
//! call by the server; no tool takes a workspace, a project or a path; skills are named, never
//! addressed by path. Each answers over **the chat's own skill set** ([`crate::skills::of_a_chat`])
//! and nothing else: a skill the set leaves out is never listed, found, loaded or shown runs of.
//! All four only read, or load, so they are pre-allowed and answer on a read-only project.
//!
//! **A load opens a skill run** ([`start_run`]), started by "a load through MCP"
//! ([`StartedBy::McpLoad`]), through the one tracker every start goes through: the server sends
//! the host the line a tool hook sends ([`crate::hookwire::SKILL_LOAD`]), so the host opens it
//! as it opens a run of the harness's own Skill tool. A load inside an open run is its child; a
//! second load of the skill in charge opens nothing.
//!
//! **A loaded skill keeps its narrowing where the harness can apply it** (ADR 0063 as amended,
//! §2). A skill that narrows the chat, on a harness whose own start applies that
//! ([`crate::harness::HarnessAdapter::narrowed_start`]) and which purlis armed with the skill,
//! is not loaded into the chat: the load is refused, opens no run, and says how to start it with
//! the harness's own skill tool, whose start opens the run. Anywhere else the skill is loaded and
//! the answer says "Narrowing not applied", never silently.
//!
//! **A manual-only skill is not loaded** (#4529): a harness never starts one by itself, so neither
//! does this tool. The load opens no run, and its answer says to ask the person to run it.
//!
//! **`skill_runs` shows the runs of the chat's own copies**: of a skill of its set, of the same
//! copy (its level), and run in the chat's own place. A run whose row does not say which copy it
//! is of (written before this) is no copy's, and is not shown.
//!
//! Behind the `skill-studio` flag until its finish line (#2786): the server lists them only
//! while the flag is on.

use std::path::Path;

use serde_json::{Map, Value, json};

use crate::chattools::{Tool, no_arguments, one_string};
use crate::counts::{counted, plural};
use crate::harness::Harness;
use crate::skills::runs::{Level, StartedBy};
use crate::skills::{Effective, Route, SkillSet};

/// The tool that lists the chat's skills.
pub const SKILL_LIST: &str = "skill_list";
/// The tool that lists the runs of the chat's skills.
pub const SKILL_RUNS: &str = "skill_runs";
/// The tool that finds the chat's skills that match a query.
pub const SKILL_FIND: &str = "skill_find";
/// The tool that loads one of the chat's skills by its name, and opens its run.
pub const SKILL_LOAD: &str = "skill_load";

/// The most runs one `skill_runs` lists.
const MOST_RUNS: usize = 20;

/// The four, in the order a harness lists them.
pub static TOOLS: [Tool; 4] = [
    Tool {
        name: SKILL_LIST,
        description: "List the skills this chat has, each by name with its level and what it \
                      does. Load one with skill_load.",
        schema: no_arguments,
        read_only: true,
    },
    Tool {
        name: SKILL_RUNS,
        description: "List the recent runs of this chat's skills, newest first, each with what \
                      started it, how it ended and what happened inside it (tool calls, errors, \
                      refusals, asks): signals, never a pass or a fail. With a name, that \
                      skill's runs alone.",
        schema: || {
            json!({
                "type": "object",
                "properties": {
                    "name": { "type": "string", "description": "The skill's name." },
                },
                "additionalProperties": false,
            })
        },
        read_only: true,
    },
    Tool {
        name: SKILL_FIND,
        description: "Find the skills of this chat that match what you are asked to do, best \
                      first, each by name with what it does. Load one with skill_load.",
        schema: || one_string("query", "Words for what the skill would do."),
        read_only: true,
    },
    Tool {
        name: SKILL_LOAD,
        description: "Load one of this chat's skills by its name: its instructions, and the \
                      paths of the other files in its folder, which it may tell you to read or \
                      run. Follow it before acting on the request it matches. A manual-only \
                      skill is not loaded: it says to ask the person to run it.",
        schema: || {
            one_string(
                "name",
                "The skill's name, as skill_list or skill_find gives it.",
            )
        },
        read_only: true,
    },
];

/// Whether `name` is one of these tools.
pub fn is_one(name: &str) -> bool {
    TOOLS.iter().any(|tool| tool.name == name)
}

/// The chat a skill tool answers for: where it works, as the server resolved it at this call.
#[derive(Debug, Clone, Copy)]
pub struct Asking<'a> {
    /// The plane's root.
    pub plane: &'a Path,
    /// The workspace it works in, or `None` at the plane root.
    pub workspace: Option<&'a str>,
    /// The persona it runs as.
    pub persona: Option<&'a str>,
    /// Its harness, by the word [`crate::hookwire::HARNESS_ENV`] names it, where it names one.
    /// One named is one purlis armed, with the skills of its set.
    pub harness: Option<&'a str>,
    /// The route its skills take ([`crate::skills::route_of`]), or none for a harness with none.
    pub route: Option<Route>,
    /// The directory of the built-in skills purlis ships ([`crate::skills::built_in_here`]).
    pub built_in: Option<&'a Path>,
}

impl Asking<'_> {
    /// Its skill set. The route changes how a skill is handed over, never which, so a harness
    /// with no route of its own has the set it would have on [`Route::Mcp`].
    pub fn set(&self) -> SkillSet {
        crate::skills::of_a_chat(
            self.built_in,
            self.plane,
            self.workspace,
            self.persona,
            self.route.unwrap_or(Route::Mcp),
        )
    }

    /// The words its harness's own skill tool starts `effective` by, applying what it narrows,
    /// where purlis armed the harness with it: a harness the chat names, whose adapter has such
    /// a start. A terminal's own Claude Code, which names none, has no plugin of the plane's
    /// skills to start one from.
    /// **What its harness reads of a skill's frontmatter** (SK-8): its adapter's, or, for a
    /// harness purlis has no adapter for, none of the keys purlis does not read itself. A load
    /// hands the `SKILL.md` purlis writes for it, as every other route does.
    fn honours(&self) -> crate::skills::Honours {
        Harness::of_chat(self.harness).map_or(
            crate::skills::Honours {
                reads: &[],
                manual: crate::skills::ManualOnly::Listing,
                agents: &[],
            },
            Harness::skill_frontmatter,
        )
    }

    fn narrowed_start(&self, effective: &Effective) -> Option<String> {
        self.harness?;
        Harness::of_chat(self.harness)?
            .adapter()
            .narrowed_start(&effective.skill.name, effective.origin)
    }
}

/// **What the server tells a chat of its skills at its start**, in its instructions: the one
/// line [`crate::skills::POINTER`] for a chat on [`Route::Mcp`], whose harness runs none of
/// purlis's hooks and so is briefed by nothing else, and nothing for a chat on any other route,
/// which is told about its skills by that route (a chat is told once).
pub fn briefing(route: Option<Route>) -> Option<&'static str> {
    (route == Some(Route::Mcp)).then_some(crate::skills::POINTER)
}

/// **Answers the skill tool `name` for `chat`, with `args`.** A load opens its run through
/// `start`, handed the skill's name, only where the skill is loaded into the chat.
pub fn call(
    chat: &Asking<'_>,
    name: &str,
    args: &Map<String, Value>,
    start: &dyn Fn(&str),
) -> Result<String, String> {
    let set = chat.set();
    match name {
        SKILL_LIST => Ok(list(&set)),
        SKILL_RUNS => Ok(runs(chat, &set, text(args, "name"))),
        SKILL_FIND => find(&set, text(args, "query").unwrap_or_default()),
        SKILL_LOAD => {
            let name = text(args, "name")
                .ok_or_else(|| "say which skill to load, by its name".to_owned())?;
            load(chat, &set, name, start)
        }
        other => Err(format!("purlis has no skill tool {other:?}")),
    }
}

/// The string `key` of `args`, trimmed, where it is one and not blank.
fn text<'a>(args: &'a Map<String, Value>, key: &str) -> Option<&'a str> {
    args.get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|text| !text.is_empty())
}

/// One skill, as a list says it: its name, its level and what it does.
fn line(effective: &Effective) -> String {
    format!(
        "- `{}` ({}): {}",
        effective.skill.name,
        crate::skills::level_shown(effective.origin, effective.local),
        crate::shown::one_line(&effective.skill.description, crate::shown::DISPLAY_LIMIT)
    )
}

fn list(set: &SkillSet) -> String {
    if set.effective.is_empty() {
        return "This chat has no skills.".to_owned();
    }
    let mut said = format!(
        "This chat has {}:",
        counted(set.effective.len(), "skill", "skills")
    );
    for effective in &set.effective {
        said.push('\n');
        said.push_str(&line(effective));
    }
    said
}

fn find(set: &SkillSet, query: &str) -> Result<String, String> {
    if query.is_empty() {
        return Err("say what the skill would do: skill_find takes words to look for".to_owned());
    }
    let found = purlis_skills::find(set, query);
    if found.is_empty() {
        return Ok(format!(
            "No skill of this chat matches {}. skill_list lists every one.",
            crate::shown::short(query)
        ));
    }
    let mut said = format!(
        "{} of this chat {} {}, best first:",
        counted(found.len(), "skill", "skills"),
        plural(found.len(), "matches", "match"),
        crate::shown::short(query)
    );
    for effective in found {
        said.push('\n');
        said.push_str(&line(effective));
    }
    Ok(said)
}

fn load(
    chat: &Asking<'_>,
    set: &SkillSet,
    name: &str,
    start: &dyn Fn(&str),
) -> Result<String, String> {
    let effective = purlis_skills::by_name(set, name).ok_or_else(|| {
        format!(
            "this chat has no skill named {}: skill_find finds one by what it does, and \
             skill_list lists them",
            crate::shown::short(name)
        )
    })?;
    let loaded = purlis_skills::load(effective, &chat.honours())?;
    let skill = &effective.skill.name;
    if loaded.manual {
        return Ok(format!(
            "`{skill}` is manual-only, so it was not loaded. Ask the person to run it."
        ));
    }
    if loaded.narrowing.any()
        && let Some(words) = chat.narrowed_start(effective)
    {
        return Ok(format!(
            "`{skill}` narrows this chat: it {}. This harness applies that only when it starts \
             the skill itself, so it was not loaded here. Start it with this harness's own skill \
             tool as `{words}`.",
            loaded.narrowing.said()
        ));
    }
    start(skill);
    let mut said = format!(
        "Loaded the skill `{skill}` ({}).",
        crate::skills::level_shown(effective.origin, effective.local)
    );
    if loaded.narrowing.any() {
        said.push_str(&format!(
            "\nNarrowing not applied: this skill {}, which this harness cannot do for a skill \
             loaded here.",
            loaded.narrowing.said()
        ));
    }
    if !loaded.files.is_empty() {
        said.push_str("\nThe other files in its folder, to read or run when it says to:");
        for file in &loaded.files {
            said.push_str(&format!("\n- {}", file.display()));
        }
    }
    said.push_str("\n\n--- SKILL.md ---\n");
    said.push_str(&loaded.text);
    Ok(said)
}

/// Whether `run` is a run of `effective`'s own copy, in `chat`'s own place.
fn of_the_copy(chat: &Asking<'_>, effective: &Effective, run: &crate::skilluse::Run) -> bool {
    let level = Level {
        origin: effective.origin,
        local: effective.local,
    };
    run.is_of(&effective.skill.name)
        && run
            .copy
            .as_ref()
            .is_some_and(|copy| copy.level == level && copy.workspace.as_deref() == chat.workspace)
}

fn runs(chat: &Asking<'_>, set: &SkillSet, name: Option<&str>) -> String {
    let runs: Vec<crate::skilluse::Run> = crate::skilluse::runs(chat.plane)
        .into_iter()
        .filter(|run| {
            set.effective
                .iter()
                .filter(|effective| name.is_none_or(|name| effective.skill.name == name))
                .any(|effective| of_the_copy(chat, effective, run))
        })
        .take(MOST_RUNS)
        .collect();
    if runs.is_empty() {
        return match name {
            Some(name) => format!(
                "No run of {} is recorded here: this chat has no skill of that name, or it has \
                 not been used here.",
                crate::shown::short(name)
            ),
            None => "No run of this chat's skills is recorded here yet.".to_owned(),
        };
    }
    let mut said = format!(
        "The last {}, newest first:",
        counted(runs.len(), "run", "runs")
    );
    for run in &runs {
        let by = run.started_by.map_or("not recorded", StartedBy::word);
        let end = run.end.as_ref().map_or_else(
            || "no end recorded".to_owned(),
            |end| {
                format!(
                    "ended with its {}: {}, {}, {}, {}",
                    end.ended.word(),
                    counted(end.tool_calls, "tool call", "tool calls"),
                    counted(end.tool_errors, "error", "errors"),
                    counted(end.refusals, "refusal", "refusals"),
                    counted(end.asks, "ask", "asks"),
                )
            },
        );
        said.push_str(&format!(
            "\n- {} `{}`, started by {by}, {end}",
            run.started, run.skill
        ));
    }
    said
}

/// **Opens the skill run a load of `skill` starts** in `hook`'s chat, made as a hook's is
/// ([`crate::toolhooks::skill_run`]): the line a tool hook sends ([`crate::hookwire::SKILL_LOAD`])
/// is delivered on the chat's hook socket, and the host opens the run once per turn and records
/// it. **Where no host hears it** (no socket, a chat number that is not one, or a line neither
/// taken nor spooled), its start is logged as a hook's unheard start is
/// ([`crate::toolhooks::started_unheard`]), so no load goes unrecorded.
pub fn start_run(hook: &crate::toolhooks::Hook<'_>, skill: &str) {
    let started = crate::toolhooks::skill_run(hook, skill.to_owned(), StartedBy::McpLoad);
    if !told_the_host(hook, &started) {
        crate::toolhooks::started_unheard(hook, &started);
    }
}

/// Whether the host took `started`'s line, or its spool holds it.
#[cfg(unix)]
fn told_the_host(
    hook: &crate::toolhooks::Hook<'_>,
    started: &crate::hookwire::SkillStarted,
) -> bool {
    let socket = crate::envvar::lookup(crate::hookwire::SOCKET_ENV, hook.env);
    let chat = crate::envvar::lookup(crate::hookwire::CHAT_ENV, hook.env)
        .and_then(|chat| chat.parse().ok());
    let (Some(socket), Some(chat)) = (socket, chat) else {
        return false;
    };
    let call = crate::hookwire::ToolCall {
        chat,
        tool_hook: crate::hookwire::SKILL_LOAD.to_owned(),
        tool: Some(SKILL_LOAD.to_owned()),
        call: None,
        args: None,
        decision: crate::hookwire::Decision::None,
        rule: None,
        hook_ms: 0,
        agent: None,
        at_ms: crate::hookwire::ms_since_1970(std::time::SystemTime::now()),
        skill: Some(Box::new(started.clone())),
    };
    let token = crate::hookwire::ChatToken::read(hook.env);
    crate::hookwire::deliver_tool(Path::new(&socket), token.as_ref(), &call).is_ok()
}

#[cfg(not(unix))]
fn told_the_host(_: &crate::toolhooks::Hook<'_>, _: &crate::hookwire::SkillStarted) -> bool {
    false
}

#[cfg(test)]
mod tests;
