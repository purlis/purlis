// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! opencode's adapter (ADR 0073, FD-13): charter's shim, loaded for one session by naming it in
//! the whole config opencode reads from `OPENCODE_CONFIG_CONTENT` ([`crate::opencode`] has the
//! measurements).

use super::adapter::{
    HarnessAdapter, RuleFile, Sandbox, SkillStart, SkillToStart, input_text, slash_with_words,
};
use super::{Harness, Kit, StateHooks};
use crate::sandbox::Form;

/// opencode's adapter.
///
/// **What its reports can prove** (#3822): only the chat's token. Its shim's hooks run inside
/// purlis's wrap, which denies them the spool their armed key is in
/// ([`crate::hookwire::armed`]), so any mode a report named would read as asking. None is
/// read (the trait's default `reported_mode`): opencode documents no classifier-backed mode.
pub struct Opencode;

/// The one opencode adapter.
pub static ADAPTER: Opencode = Opencode;

/// The directory an opencode chat's skills sit in: what the shim appends to `skills.paths`. It
/// is the `skills/` of the set's folder the host laid out ([`crate::skills::linked`]), which
/// holds the built-in skills as `purlis-<name>` beside the plane's
/// ([`crate::skills::Route::names_built_ins_its_own`]): opencode reads every skill by its bare
/// `name`, in this folder and the clone's and the person's own alike, and a skill of the clone's
/// named as a built-in one would otherwise be a second skill of that name, of which opencode
/// keeps whichever it read last (measured on 1.18.33). The bundle's own `skills/` is never
/// handed. No folder, none of them.
fn skill_dirs(kit: Kit<'_>) -> Vec<std::path::PathBuf> {
    let set = crate::skills::for_chat(kit, Harness::Opencode);
    crate::skills::linked(&set, kit)
        .map(|folder| folder.join(crate::skills::DIR_IN_BUNDLE))
        .into_iter()
        .collect()
}

impl HarnessAdapter for Opencode {
    fn harness(&self) -> Harness {
        Harness::Opencode
    }

    /// **The bundled shim, loaded for this session alone** ([`crate::opencode`] has the
    /// measurements, opencode 1.18.23). opencode reads a whole config from
    /// `OPENCODE_CONFIG_CONTENT` and concatenates its `plugin` list with every other
    /// config's, so naming the shim there loads it beside whatever the operator loads,
    /// writes nothing, and a project file cannot take it out. It runs the binary the
    /// environment names, as the Claude Code plugin's hooks do. `OPENCODE_PURE=0`
    /// because `1` would load no plugin at all; the flag that does the same is refused
    /// before the chat starts ([`crate::opencode::disarmed_by`]).
    ///
    /// No plugin choice: opencode has no switch that turns one plugin off.
    ///
    /// **A skill the project turned off** (SK-6, #2769) is denied in the same config's
    /// `permission.skill`, by its name: a denied skill is left out of the list the model is
    /// shown (`Skill.available` in `skill/index.ts`, opencode 1.18.33; measured through
    /// `OPENCODE_PERMISSION`, the same merge), and a call to it is refused. opencode merges
    /// objects key by key, so the operator's own rules stand beside it.
    ///
    /// Its skills are its set's ([`crate::skills::for_chat`]): the `skills/` of the set's folder
    /// the host laid out ([`crate::skills::linked`]), the built-in skills as `purlis-<name>`
    /// beside the plane's, is handed to the shim as its option, which adds it to the skills
    /// opencode discovers for this process ([`Harness::skills`], [`skill_dirs`]).
    ///
    /// It cannot report `SessionEnd`: quitting opencode fires no event and runs no exit
    /// handler in a plugin (measured). The app sees the process end.
    fn arm_under(
        &self,
        kit: Kit<'_>,
        _cwd: Option<&std::path::Path>,
        plugins: &crate::harness_plugin::Chosen,
        sandbox: Sandbox<'_>,
    ) -> StateHooks {
        let Some(shim) = kit
            .plugin
            .map(crate::opencode::shim_in)
            .filter(|shim| shim.is_file())
        else {
            return StateHooks::None;
        };
        StateHooks::ThisSessionOnly {
            args: Vec::new(),
            env: vec![
                (
                    crate::plugin::BINARY_ENV.to_owned(),
                    kit.binary.display().to_string(),
                ),
                (
                    crate::opencode::CONFIG_ENV.to_owned(),
                    crate::opencode::session_config(
                        &shim,
                        &skill_dirs(kit),
                        Some(kit.binary),
                        sandbox.applied().is_some(),
                        &crate::skills::skills_off_for(kit, Harness::Opencode, plugins),
                    ),
                ),
                (crate::opencode::PURE_ENV.to_owned(), "0".to_owned()),
            ],
            cannot_report: vec!["sessionend"],
        }
    }

    /// `OPENCODE_PURE=1`, or the flag that does the same, loads no plugin at all, so a chat
    /// started with either is refused before it starts ([`crate::opencode::disarmed_by`]).
    fn disarmed_by(&self, command: &[String], env: &[(String, String)]) -> Option<String> {
        crate::opencode::disarmed_by(command, env)
    }

    /// **Measured on opencode 1.18.33** (FM-9), from its input's own `@` completion, driven in
    /// a terminal and read in its bundle: a file is `@src/main.rs`, a folder `@src/`, and lines
    /// `@src/main.rs#10` or `@src/main.rs#10-20` — a `#` and no `L`. A typed mention ends at
    /// white space and splits at `#`, so a path holding either is handed over in plain words.
    /// charter does not type into opencode yet (ADR 0061); this is what the clipboard is given.
    fn reference(&self, reference: &crate::reference::Reference) -> String {
        let path = reference.path();
        if path.contains('#') || path.chars().any(char::is_whitespace) {
            return reference.plain();
        }
        format!(
            "@{}{}",
            reference.path_with_slash(),
            reference.lines_after("#", "")
        )
    }

    fn armed_with(&self) -> String {
        "the app arms each opencode chat with purlis's opencode plugin, for that chat alone"
            .to_owned()
    }

    fn plugins(&self) -> &'static dyn crate::harness_plugin::Adapter {
        &crate::harness_plugin::OPENCODE
    }

    fn guard_label(&self) -> &'static str {
        "opencode"
    }

    /// opencode's `skill` tool, which purlis's shim reports as `Skill`
    /// ([`crate::opencode::TOOLS`]), naming the skill in `name`.
    fn skill_start(&self, tool: &str, input: &serde_json::Value) -> Option<SkillStart> {
        (tool == "Skill")
            .then(|| input_text(input, "name"))
            .flatten()
            .map(|skill| SkillStart::Named(skill.to_owned()))
    }

    /// **opencode 1.18.33** (research of 2026-10-20, `session/prompt.ts`): every skill is also
    /// a slash command, `/<name>`, by its own name; what follows is split into words, a quoted
    /// run being one (`"[^"]*"|'[^']*'|[^\s"']+`), for `$1..$n`, and `$ARGUMENTS` is the raw line.
    /// So each value is one quoted word, in the declared order.
    fn skill_start_text(&self, skill: &SkillToStart<'_>) -> String {
        let head = format!("/{}", self.skill_name(skill.name, skill.origin));
        slash_with_words(&head, skill.given)
    }

    /// opencode 1.18.33 puts every skill in its system prompt with no limit at all, so purlis's
    /// budget is the only one its chats have (ADR 0102 §1), and no description is cut. A
    /// manual-only skill is handed without its description, which keeps it out of the listing
    /// ([`Self::skill_frontmatter`]).
    fn skill_budget(&self) -> crate::skills::Budget {
        super::SKILL_BUDGET
    }

    /// **opencode 1.18.33** (`skill/index.ts`, research of 2026-10-20): it reads `name` and
    /// `description` and nothing else. It has no manual-only key, but it lists only a skill that
    /// has a description (`fmt` filters `description !== undefined`, S:322, measured) and still
    /// runs one by `/name` or by the `skill` tool, so purlis hands a manual-only skill without
    /// its description ([`crate::skills::handed_text`]).
    fn skill_frontmatter(&self) -> crate::skills::Honours {
        crate::skills::Honours {
            reads: &[],
            manual: crate::skills::ManualOnly::WithoutDescription,
            agents: &[],
        }
    }

    /// `permission.bash` of the project's `opencode.json`, which holds command globs: a
    /// `Bash(...)` rule's glob goes there. opencode has no machine-local file, so a `local`
    /// rule has nowhere to go, and a tool's rule has no place among command globs.
    fn command_rule(
        &self,
        root: &std::path::Path,
        rule: &str,
        local: bool,
    ) -> Result<RuleFile, String> {
        use crate::scaffold::settings;
        if local {
            return Err(
                "opencode has no machine-local settings file, so a --local rule cannot \
                 reach it"
                    .to_owned(),
            );
        }
        let Some(glob) = rule.strip_prefix("Bash(").and_then(|r| r.strip_suffix(')')) else {
            return Err(format!(
                "opencode's config holds command patterns, and {rule} is not one"
            ));
        };
        let (root, glob) = (root.to_path_buf(), glob.to_owned());
        Ok(RuleFile::new(
            root.join(settings::OPENCODE),
            move |bucket, dry_run| settings::ensure_opencode_rule(&root, &glob, bucket, dry_run),
        ))
    }

    /// charter's own wrap around the whole harness, which has no sandbox of its own
    /// ([`crate::sandbox::opencode`] has the measurements).
    fn sandbox_compiler(&self) -> Option<crate::sandbox::Compiler> {
        Some(|compiled| crate::sandbox::opencode::wrap(compiled).map(Form::Opencode))
    }

    /// Yes: the wrap is around the whole harness, so its plugin's hooks run inside it.
    fn sandbox_holds_what_it_starts(&self) -> bool {
        true
    }

    /// Never, by the operator's ruling P8 of 2026-10-27 (#3468): opencode's plugins are code it
    /// loads into its own process, and it has no switch that turns one off for a single chat,
    /// so a grade's chat here would run with the person's plugins and not measure the skill
    /// alone. (Its limits and spend, #3272, would be a second reason.)
    fn can_grade(&self) -> Result<(), String> {
        Err(
            "opencode's plugins are code that runs inside it, which purlis cannot turn off for \
             one chat, so a grade there could not measure the skill alone"
                .into(),
        )
    }

    /// `--model <provider/model>`, as `opencode --help` names it (`-m, --model`).
    fn model_words(&self, model: &str) -> Vec<String> {
        vec!["--model".to_owned(), model.to_owned()]
    }

    /// The wrap's program first, with the profile written for where the chat opens, then the
    /// chat's whole line. Its traffic is pointed at charter's egress proxy, the only place the
    /// profile lets it connect, and its temp directory is its own.
    ///
    /// **Fail closed.** No directory, no proxy, or a path a profile cannot state refuses the
    /// chat. Nothing in opencode's own words can widen an operating-system profile, so none is
    /// refused for them.
    fn sandboxed_line(
        &self,
        form: &Form,
        words: crate::sandbox::Words,
        at: &crate::sandbox::At<'_>,
    ) -> Result<crate::sandbox::Line, String> {
        let Form::Opencode(wrap) = form else {
            return Err(super::adapter::not_compiled_for(Harness::Opencode));
        };
        let lead = "this project runs every chat sandboxed, and";
        let Some(cwd) = at.cwd else {
            return Err(format!(
                "{lead} an opencode chat with no directory of its own has nowhere the sandbox \
                 lets it write, so nothing was started."
            ));
        };
        let Some((confinement, tmp)) = at
            .confinement
            .and_then(|confinement| Some((confinement, confinement.tmp()?)))
        else {
            return Err(format!(
                "{lead} purlis's egress proxy was not started for this opencode chat, so \
                 nothing was started."
            ));
        };
        crate::sandbox::opencode::prepare(wrap);
        // The doors its confinement opened (#2019): a socket for each, in its own folder.
        let served = confinement.served();
        let profile = crate::sandbox::opencode::profile_serving(
            wrap,
            cwd,
            tmp,
            &confinement.proxy_ports(),
            at.hook_socket,
            served
                .as_ref()
                .map(|(folder, ports)| (*folder, ports.as_slice())),
        )
        .map_err(|why| crate::sandbox::seatbelt::not_started(lead, why, !at.no_opt_out))?;
        let mut env = crate::sandbox::seatbelt::env(&confinement.proxy_url(), tmp);
        env.push((
            crate::sandbox::opencode::STATE_ENV.to_owned(),
            tmp.join("state").display().to_string(),
        ));
        let crate::sandbox::Words {
            program,
            command,
            armed,
            charters,
        } = words;
        let (program, args) = crate::sandbox::wrapper::line(
            wrap.os,
            at.purlis,
            profile,
            program,
            [command, armed, charters].concat(),
        )
        .map_err(|why| crate::sandbox::seatbelt::not_started(lead, why, !at.no_opt_out))?;
        Ok(crate::sandbox::Line { program, args, env })
    }
}

#[cfg(test)]
mod tests;
