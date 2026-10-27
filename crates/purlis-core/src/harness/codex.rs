// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Codex's adapter (ADR 0073, FD-13): charter's hooks, armed on one session by Codex's own
//! `-c hooks.<Event>=…` flags.

use super::adapter::{HarnessAdapter, RuleFile, Sandbox, SkillStart, SkillToStart, input_text};
use super::{Harness, Kit, StateHooks};
use crate::sandbox::Form;

/// Codex's adapter.
pub struct Codex;

/// The one Codex adapter.
pub static ADAPTER: Codex = Codex;

/// What a Codex chat is started with for its `SessionStart` briefing to list its skills: the
/// built-in skills' directory, and the set's folder the host laid the plane's out in, where its
/// set holds any (SK-8: the briefing names each by the `SKILL.md` purlis wrote for it there).
fn listed(kit: Kit<'_>) -> Vec<(String, String)> {
    let set = crate::skills::for_chat(kit, Harness::Codex);
    let Some(dir) = set.built_in_dir() else {
        return Vec::new();
    };
    let mut env = vec![(
        crate::skills::LISTED_ENV.to_owned(),
        dir.display().to_string(),
    )];
    if let Some(handed) = crate::skills::linked(&set, kit) {
        env.push((
            crate::skills::PLANE_LISTED_ENV.to_owned(),
            handed.display().to_string(),
        ));
    }
    env
}

/// The `-c` flags that hand a Codex chat the plugins and skills the project turned off: one
/// `plugins."<id>".enabled=false` per plugin, then one `skills.config` holding every skill.
/// Each key and name is quoted as TOML quotes it, so no id can end the value early.
fn chosen_flags(plugins: &crate::harness_plugin::Chosen, skills_off: &[String]) -> Vec<String> {
    let mut args = Vec::new();
    for (id, on) in &plugins.plugins {
        if !on {
            args.push("-c".to_owned());
            args.push(format!(
                "plugins.{}.enabled=false",
                crate::extension::project::toml_key(id)
            ));
        }
    }
    if !skills_off.is_empty() {
        let entries: Vec<String> = skills_off
            .iter()
            .map(|name| {
                format!(
                    "{{name={},enabled=false}}",
                    toml::Value::String(name.clone())
                )
            })
            .collect();
        args.push("-c".to_owned());
        args.push(format!("skills.config=[{}]", entries.join(",")));
    }
    args
}

impl HarnessAdapter for Codex {
    fn harness(&self) -> Harness {
        Harness::Codex
    }

    /// **Measured on codex-cli 0.147.0, and it refutes what this arm used to say** —
    /// that Codex could only be armed in `~/.codex/config.toml` and could never say it
    /// was waiting. Every fact here was taken from the real binary driving a real turn
    /// against a stand-in model server, the TUI in a pane as the app runs it (#27):
    ///
    /// * `-c hooks.<Event>=[…]` arms a hook for ONE session. Codex lists its source as
    ///   "Session flags", and it runs BESIDE the operator's own `[[hooks.<Event>]]` for
    ///   the same event rather than replacing it — both fired. Nothing is written.
    /// * `SessionStart`, `UserPromptSubmit`, `Stop` and `SessionEnd` each fire with
    ///   `session_id` in the payload. `SessionStart` names a `source` (`startup`, and
    ///   `resume` with the SAME id on `codex resume <id>`); `SessionEnd` names a
    ///   `reason` (`other`, on `/quit`). `Stop` is "right before Codex ends its turn",
    ///   which is the falling edge `Stop` means for Claude Code.
    /// * There is no `Notification`. Codex tells a hook it is asking for approval only
    ///   through `PermissionRequest` — which fired exactly when the prompt appeared,
    ///   and not for a command that needed none. That hook DECIDES a permission, so the
    ///   app arms it as Claude Code's is (#1691, [`permission_flag`]): it decides only
    ///   what the person chose in the window, and otherwise nothing, and Codex asks.
    /// * **And there is no second way round it** (charter-app#52, read out of the same
    ///   0.147.0 binary the measurements above were taken on). The binary carries
    ///   eleven hook events and no more — `PreToolUse`, `PermissionRequest`,
    ///   `PostToolUse`, `PreCompact`, `PostCompact`, `SessionStart`, `SessionEnd`,
    ///   `UserPromptSubmit`, `SubagentStart`, `SubagentStop`, `Stop` — and the only
    ///   one that fires when the prompt appears is the one that decides it. The
    ///   `notify` program is not a second channel either: in 0.147.0 it is
    ///   `legacy_notify`, a shim over `Stop` whose one payload type is
    ///   `agent-turn-complete`, which is the falling edge `Stop` already gives. What
    ///   is left is `PreToolUse` without a matching `PostToolUse` for long enough —
    ///   which is a guess at timing over a harness's behaviour, and ADR 0018 admits
    ///   no state that did not come from a hook saying so.
    /// * **After a command, `PostToolUse`** (#1663, read from the `rust-v0.147.0` source, not
    ///   measured live): every shell tool is `Bash` to a hook, and the payload carries
    ///   `tool_input.command` and what the command printed as one string at `tool_response`.
    ///   The sandbox's block word is armed on it ([`crate::plugin::CODEX`] has the citations),
    ///   so a raw client's refused lookup, a refused socket or a refusal it printed raises a
    ///   block in a Codex chat too. Codex has no `PostToolUseFailure`. Adding the word changes
    ///   the hooks Codex asks the operator to trust, so it asks once more.
    /// * **A child agent's end, `SubagentStop`** (#1086, read from the same source, not measured
    ///   live): a thread-spawned child's turn ends in it, naming the child's `agent_id` under the
    ///   root's `session_id`, so a Codex child reads done when it finishes rather than working
    ///   until the chat ends ([`crate::plugin::CODEX`] has the citations). Arming it is one more
    ///   hook to trust, so Codex asks once more.
    /// * `SessionStart` fires inside the FIRST TURN, not at launch (X1): an idle TUI
    ///   reported nothing at all until a prompt was typed.
    /// * A hook is inert until Codex trusts it. For these, the TUI itself asks at
    ///   startup — "Hooks need review … Trust all and continue / Continue without
    ///   trusting (hooks won't run)" — and Codex writes the answer into its own
    ///   `[hooks.state]`, keyed by event, position and a hash of the hook. The same
    ///   binary path arms the same hooks, so the operator is asked once and not once a
    ///   chat. Untrusted, they do not run and nothing says so (`codex exec`).
    ///
    /// Codex has no plugin here: the guard used to reach a Codex chat through the
    /// Python charter's Codex plugin, and now rides on the same `-c` flags as the state
    /// hooks, from the same registry ([`crate::plugin::CODEX`] out of [`crate::hookreg`]).
    ///
    /// **A plugin the project turned off, by `-c`** (SK-6, #2769): `-c
    /// 'plugins."<id>".enabled=false'` each, which codex-cli 0.163.0 takes for one session
    /// (measured, [`crate::harness_plugin::CODEX`]); 0.147.0 did not. Only off: Codex takes a
    /// plugin turned on that way as nothing, so `plugins` holds no `true` for it
    /// ([`crate::harness_plugin::chosen`]). **And a skill the project turned off**, by one `-c
    /// 'skills.config=[{name="<name>",enabled=false},…]'`, which reaches a plugin's skill
    /// (`<plugin>:<skill>`) too, and is added to the entries of the operator's own
    /// `config.toml` rather than replacing them (both measured on 0.163.0).
    ///
    /// Its skills ride on the `SessionStart` hook above: Codex can take no skills
    /// directory for one session ([`Harness::skills`]), so the chat is started with the
    /// directory of the skills its set holds ([`crate::skills::for_chat`]) in
    /// [`crate::skills::LISTED_ENV`], which a Codex hook inherits, and the briefing lists
    /// what its set holds ([`crate::skills::briefed`]). Where the set holds the plane's skills,
    /// [`crate::skills::PLANE_LISTED_ENV`] names the set's folder the host laid them out in, and
    /// the briefing lists them too, each by the `SKILL.md` purlis wrote for it there.
    ///
    /// Its sandbox, where the plane turned it on, is not here: charter's wrap runs the whole
    /// line ([`crate::sandbox::Applied::line`], and [`crate::sandbox::codex`] has the
    /// measurements). An unsandboxed Codex chat keeps Codex's own settings, as it did.
    fn arm_under(
        &self,
        kit: Kit<'_>,
        _cwd: Option<&std::path::Path>,
        plugins: &crate::harness_plugin::Chosen,
        _sandbox: Sandbox<'_>,
    ) -> StateHooks {
        let mut args = session_flags(kit.binary);
        args.extend(permission_flag(kit.binary));
        args.extend(chosen_flags(
            plugins,
            &crate::skills::skills_off_for(kit, Harness::Codex, plugins),
        ));
        // charter's MCP server (HP-7), for this session beside the operator's own servers.
        args.extend(["-c".to_owned(), crate::chattools::codex_flag(kit.binary)]);
        StateHooks::ThisSessionOnly {
            args,
            env: listed(kit),
            cannot_report: vec!["notification"],
        }
    }

    // `disarmed_by`: none. Codex's session flags are charter's own.

    /// **Measured on codex-cli 0.147.0** (FM-9), driving its input in a terminal with a stand-in
    /// model: its own `@` file search replaces the word with the path relative to the folder it
    /// works in — a folder as `src`, with no `/` — and puts a path with white space in double
    /// quotes. Codex attaches nothing for a path; its agent reads the file with its own tools, so
    /// lines are written in the `path:line` form its own instructions use for a file reference,
    /// with a range as `path:10-20`.
    fn reference(&self, reference: &crate::reference::Reference) -> String {
        crate::reference::quoted_if_spaced_or_absolute(&format!(
            "{}{}",
            reference.path(),
            reference.lines_after(":", "")
        ))
    }

    fn armed_with(&self) -> String {
        "the app arms each Codex chat with purlis's hooks; Codex asks once to trust them".to_owned()
    }

    fn plugins(&self) -> &'static dyn crate::harness_plugin::Adapter {
        &crate::harness_plugin::CODEX
    }

    fn guard_label(&self) -> &'static str {
        "codex"
    }

    /// Codex has no Skill tool: a chat briefed on its skills starts one by reading its
    /// `SKILL.md`, in a shell command that only reads (decision 9;
    /// [`purlis_skills::runs::is_a_plain_read`]). Its shell is `Bash` to a hook.
    fn skill_start(&self, tool: &str, input: &serde_json::Value) -> Option<SkillStart> {
        let command = (tool == "Bash")
            .then(|| input_text(input, "command"))
            .flatten()?;
        purlis_skills::runs::is_a_plain_read(command)
            .then(|| SkillStart::Reading(command.to_owned()))
    }

    /// **Codex 0.163.0** (research of 2026-10-20, `skills/src/mentions.rs`): `$<name>` in the
    /// prompt names a skill, and a briefed chat is told that a prompt naming a skill by its name
    /// means its list ([`crate::skills::listing`]). Codex substitutes no arguments, so they
    /// follow the mention as plain words the model reads: `name: value` for each named one, in
    /// the declared order, a value left empty said as none.
    fn skill_start_text(&self, skill: &SkillToStart<'_>) -> String {
        let mention = format!("${}", self.skill_name(skill.name, skill.origin));
        match skill.given.in_words() {
            None => mention,
            Some(words) => format!("{mention} {words}"),
        }
    }

    /// Codex lists skills within 2% of the context window, 8,000 characters where it does not
    /// know the window, cutting each description at 1,024 characters, and past that drops
    /// trailing skills without saying so (0.163.0, `ext/skills/src/render.rs`). A Codex chat is
    /// briefed on its skills, each line with its `SKILL.md`'s path, which counts; a manual-only
    /// skill's description stays out of the briefing ([`Self::skill_frontmatter`]).
    fn skill_budget(&self) -> crate::skills::Budget {
        crate::skills::Budget {
            per_description: Some(1_024),
            lists_paths: true,
            ..super::SKILL_BUDGET
        }
    }

    /// **Codex 0.163.0** (`skills/src/parser.rs:6-20`, research of 2026-10-20): its own parser
    /// reads `name`, `description` and `metadata.short-description`, and nothing else. A Codex
    /// chat is briefed on purlis's skills, though, and the model reads each `SKILL.md` as text,
    /// so no key applies there but the ones purlis reads itself. Manual-only is translated: the
    /// briefing names a manual-only skill and its file alone, to read only when the person
    /// names it ([`crate::skills::listing`]), as Codex's own `agents/openai.yaml`
    /// `policy.allow_implicit_invocation: false` keeps one of its skills out of its catalogue.
    fn skill_frontmatter(&self) -> crate::skills::Honours {
        crate::skills::Honours {
            reads: &[],
            manual: crate::skills::ManualOnly::Listing,
            agents: &[],
        }
    }

    /// **Codex 0.163.0** (`ext/skills/src/loader/metadata.rs`, `loader/mod.rs:20-21`, research of
    /// 2026-10-20, measured): a skill it finds itself, its own or a repo's, is kept out of its
    /// model's catalogue by `agents/openai.yaml` setting `policy.allow_implicit_invocation:
    /// false`, and still runs when named. Codex reads that file whole with serde and ignores all
    /// of it where it does not parse, so purlis reads it as manual-only only where its strict
    /// reader finds that one plain `false` ([`purlis_skills::handed::flag_at`]): anything else
    /// reads as the default, which lists the skill.
    fn manual_beside(&self) -> Option<crate::skills::ManualBeside> {
        Some(crate::skills::ManualBeside {
            file: "agents/openai.yaml",
            says: |text| {
                purlis_skills::handed::flag_at(text, &["policy", "allow_implicit_invocation"])
                    == Some(false)
            },
        })
    }

    /// None: Codex has no command-pattern permissions, so a rule has no file to go into, and
    /// purlis's own guard, which rides on the chat's hooks, is what applies there.
    fn command_rule(
        &self,
        _root: &std::path::Path,
        _rule: &str,
        _local: bool,
    ) -> Result<RuleFile, String> {
        Err(
            "Codex has no command-pattern permissions, so purlis's own guard is what applies \
             there"
                .to_owned(),
        )
    }

    /// charter's own wrap around the whole harness, with Codex's own sandbox off inside it
    /// (#1123; [`crate::sandbox::codex`] has the measurements).
    fn sandbox_compiler(&self) -> Option<crate::sandbox::Compiler> {
        Some(|compiled| crate::sandbox::codex::wrap(compiled).map(Form::Codex))
    }

    /// Yes: the wrap is around the whole harness, so its hooks and MCP servers run inside it.
    fn sandbox_holds_what_it_starts(&self) -> bool {
        true
    }

    /// No grade runs here until purlis can hold its chats to the grade's limits and read their
    /// spend on this harness (#3272).
    fn can_grade(&self) -> bool {
        false
    }

    /// The wrap's program first, with the profile written for where the chat opens, then the
    /// chat's whole line, with the flags that turn Codex's own sandbox off last among its flags
    /// and in front of the subcommand, session id and first message that end the line. Its
    /// traffic is pointed at charter's egress proxy, and its temp directory is its own.
    ///
    /// **Fail closed.** A flag of Codex's own in the profile's command or the chat's own words
    /// that would widen what charter hands it refuses the chat, naming where the flag is; so do
    /// no directory, no proxy, and a path a profile cannot state.
    fn sandboxed_line(
        &self,
        form: &Form,
        words: crate::sandbox::Words,
        at: &crate::sandbox::At<'_>,
    ) -> Result<crate::sandbox::Line, String> {
        let Form::Codex(wrap) = form else {
            return Err(super::adapter::not_compiled_for(Harness::Codex));
        };
        let crate::sandbox::Words {
            program,
            command,
            armed,
            charters,
        } = words;
        for (words, named, fix) in [
            (
                &command,
                "the profile's command names",
                "Take it out of the profile's command.",
            ),
            (
                &charters,
                "the chat's own arguments name",
                "Start it without that argument.",
            ),
        ] {
            if let Some(flag) = crate::sandbox::codex::loosened_by(words) {
                return Err(format!(
                    "this project runs every chat sandboxed, and {named} {flag}, which would run \
                     {} outside the sandbox purlis compiled for it, so nothing was started. \
                     {fix}",
                    Harness::Codex.title()
                ));
            }
        }
        let lead = "this project runs every chat sandboxed, and";
        let Some(cwd) = at.cwd else {
            return Err(format!(
                "{lead} a Codex chat with no directory of its own has nowhere the sandbox lets \
                 it write, so nothing was started."
            ));
        };
        let Some((confinement, tmp)) = at
            .confinement
            .and_then(|confinement| Some((confinement, confinement.tmp()?)))
        else {
            return Err(format!(
                "{lead} purlis's egress proxy was not started for this Codex chat, so nothing \
                 was started."
            ));
        };
        // D-88q, as amended 2026-10-15 (#2037): the folder of a profile for sandboxed chats
        // only, never the operator's own Codex home.
        let Some(home) = &wrap.home else {
            return Err(
                "this chat runs sandboxed, and a sandboxed Codex chat runs in the folder of \
                        a profile for sandboxed chats only, and this chat is on no such \
                        profile, so nothing was started. Make or choose a sandbox-only Codex \
                        profile in Settings › You › Profiles."
                    .to_owned(),
            );
        };
        let profile = crate::sandbox::codex::profile(
            wrap,
            cwd,
            tmp,
            &confinement.proxy_ports(),
            at.hook_socket,
        )
        .map_err(|why| crate::sandbox::seatbelt::not_started(lead, why, !at.no_opt_out))?;
        let mut charters = charters;
        let tail =
            charters.split_off(charters.len() - crate::sandbox::codex::positional_tail(&charters));
        let (program, args) = crate::sandbox::wrapper::line(
            wrap.os,
            at.purlis,
            profile,
            program,
            [
                command,
                armed.clone(),
                charters,
                crate::sandbox::codex::flags(),
                tail,
            ]
            .concat(),
        )
        .map_err(|why| crate::sandbox::seatbelt::not_started(lead, why, !at.no_opt_out))?;
        // Only now, with the line known to start (review F6): a refused chat readies no home.
        crate::sandbox::codex::prepare(wrap, cwd, &armed);
        Ok(crate::sandbox::Line {
            program,
            args,
            env: [
                crate::sandbox::seatbelt::env(&confinement.proxy_url(), tmp),
                vec![
                    (
                        crate::sandbox::codex::ROOTS_ENV.to_owned(),
                        crate::sandbox::codex::ROOTS.to_owned(),
                    ),
                    (
                        crate::sandbox::codex::HOME_ENV.to_owned(),
                        home.display().to_string(),
                    ),
                ],
            ]
            .concat(),
        })
    }
}

/// **The `-c` pair that arms Codex's `PermissionRequest`** (#1691): `purlis hook
/// permissionrequest`, for every tool, with the timeout the ask's deadline sits below. Its
/// prompt is then also an ask in the window, answered back on the hook
/// ([`crate::harness::hooked`]); unanswered, the hook decides nothing and Codex asks.
///
/// Not from the registry, as Claude Code's is not from the plugin: a hook that can allow a
/// permission rides only on the argument, never in a file a chat can write. Codex asks the
/// operator to trust it once, as it does each of purlis's hooks.
fn permission_flag(binary: &std::path::Path) -> [String; 2] {
    let mut hook = toml::Table::new();
    hook.insert("type".to_owned(), toml::Value::from("command"));
    hook.insert(
        "command".to_owned(),
        toml::Value::from(crate::plugin::command_at(binary, super::hooked::WORD)),
    );
    hook.insert(
        "timeout".to_owned(),
        toml::Value::from(i64::try_from(super::hooked::HOOK_TIMEOUT.as_secs()).unwrap_or(i64::MAX)),
    );
    let mut group = toml::Table::new();
    group.insert(
        "hooks".to_owned(),
        toml::Value::Array(vec![toml::Value::Table(hook)]),
    );
    let value = toml::Value::Array(vec![toml::Value::Table(group)]);
    ["-c".to_owned(), format!("hooks.PermissionRequest={value}")]
}

/// The `-c` pairs that arm Codex's hooks on one session, out of [`crate::plugin::codex_handlers`].
///
/// On the argument for the reason Claude Code's are: nothing is written, so nothing is left
/// behind. Each value is TOML, because that is how Codex parses a `-c` value — and it is
/// SERIALISED rather than formatted, since a value that fails to parse is not an error to
/// Codex but a literal string, which it then rejects as the wrong type and refuses to start.
///
/// The command names the binary by its absolute path: Codex has no plugin root and no
/// variable of charter's to expand, and a Codex hook runs through a shell just the same —
/// measured, a single-quoted argument holding spaces arrived as one word.
fn session_flags(binary: &std::path::Path) -> Vec<String> {
    crate::plugin::grouped(crate::plugin::codex_handlers())
        .into_iter()
        .flat_map(|(event, groups)| {
            let groups: Vec<toml::Value> = groups
                .into_iter()
                .map(|(matcher, hooks)| {
                    let hooks: Vec<toml::Value> = hooks
                        .iter()
                        .map(|hook| {
                            let table: toml::Table = [
                                ("type".to_owned(), toml::Value::from("command")),
                                (
                                    "command".to_owned(),
                                    toml::Value::from(crate::plugin::command_at(binary, hook.name)),
                                ),
                                // Codex's own default is 600 seconds (its review screen says so).
                                (
                                    "timeout".to_owned(),
                                    toml::Value::from(i64::from(hook.timeout)),
                                ),
                            ]
                            .into_iter()
                            .collect();
                            toml::Value::Table(table)
                        })
                        .collect();
                    let mut group = toml::Table::new();
                    if let Some(matcher) = matcher {
                        group.insert("matcher".to_owned(), toml::Value::from(matcher));
                    }
                    group.insert("hooks".to_owned(), toml::Value::Array(hooks));
                    toml::Value::Table(group)
                })
                .collect();
            let value = toml::Value::Array(groups);
            ["-c".to_owned(), format!("hooks.{event}={value}")]
        })
        .collect()
}

#[cfg(test)]
mod tests;
