// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Claude Code's adapter (ADR 0073, FD-13): the bundled plugin, loaded for one chat alone, with
//! the session's own `--settings`.

use super::adapter::{
    HarnessAdapter, RuleFile, Sandbox, SkillStart, SkillToStart, input_text, slash_with_words,
};
use super::{Harness, Kit, StateHooks, words};
use crate::sandbox::Form;

pub mod project;

/// Claude Code's adapter.
pub struct ClaudeCode;

/// The one Claude Code adapter.
pub static ADAPTER: ClaudeCode = ClaudeCode;

impl HarnessAdapter for ClaudeCode {
    fn harness(&self) -> Harness {
        Harness::ClaudeCode
    }

    /// **The bundled plugin, loaded for this session alone** (`crate::plugin` has the
    /// measurements). Its `hooks.json` holds every hook — the six that report state and the
    /// Bash guard — and names the binary by `$CHARTER_HOOK_BINARY`, so that is handed over in
    /// the environment. Without the plugin there is nothing to arm: a chat reads `unknown`,
    /// rather than being half-armed from a second place.
    ///
    /// Its skills take [`crate::skills::Route::Plugin`]: the built-in skills of its set
    /// ([`crate::skills::for_chat`]) are the plugin's own `skills/`, which `--plugin-dir`
    /// already loads; the plane's are a second `--plugin-dir`, the folder of links the host laid
    /// out for them ([`crate::skills::linked`]), where the set holds any.
    ///
    /// `--settings` MERGES with the settings already in force rather than replacing them
    /// (measured on claude 2.1.276), and a key it names wins over the project's (measured on
    /// 2.1.280). It carries the plugin pins, the project's plugin choice, the status line where
    /// charter may fill it, Smart close's allow and the allows for charter's five read-only
    /// tools, and the sandbox where the plane turned it on.
    ///
    /// **A sandboxed chat loads its project's own Claude Code settings only where they hold
    /// nothing but what [`project::KEPT`] names** (ADR 0067 as amended 2026-10-15). Otherwise it
    /// is started on the person's settings and purlis's alone (`--setting-sources user`), and
    /// the project's deny and ask rules ride on `--settings` instead ([`project`]).
    fn arm_under(
        &self,
        kit: Kit<'_>,
        cwd: Option<&std::path::Path>,
        plugins: &crate::harness_plugin::Chosen,
        sandbox: Sandbox<'_>,
    ) -> StateHooks {
        let Some(plugin) = kit.plugin else {
            return StateHooks::None;
        };
        let skills = crate::skills::for_chat(kit, Harness::ClaudeCode);
        debug_assert!(
            skills
                .built_in_dir()
                .is_none_or(|dir| dir == plugin.join(crate::skills::DIR_IN_BUNDLE)),
            "a Claude Code chat's built-in skills are its plugin's own: {:?}",
            skills.built_in_dir()
        );
        // Claude Code carries its sandbox in the same `--settings`, as compiled for the folder
        // the chat runs in (its manifests, #1336). A form of another kind is not one, and the
        // chat is armed with nothing rather than started without it.
        let form = sandbox.applied().map(|applied| applied.form_in(cwd));
        let sandbox = match &form {
            None => None,
            Some(crate::sandbox::Form::ClaudeCode(settings)) => Some(settings),
            Some(_) => return StateHooks::None,
        };
        // Read for a sandboxed chat alone: one that is not keeps loading what it always did.
        let screened = sandbox.map(|_| project::screen(cwd));
        // The skills it can be handed as turned off: never a built-in skill's, nor a plugin's
        // ([`crate::skills::skills_off_for`]).
        let handed = crate::harness_plugin::Chosen {
            plugins: plugins.plugins.clone(),
            skills_off: crate::skills::skills_off_for(kit, Harness::ClaudeCode, plugins)
                .into_iter()
                .collect(),
        };
        let mut args = words(["--plugin-dir", &plugin.display().to_string()]);
        // The plane's skills, as a plugin of their own (ADR 0063 as amended 2026-10-20).
        if let Some(linked) = crate::skills::linked(&skills, kit) {
            args.extend(words(["--plugin-dir", &linked.display().to_string()]));
        }
        args.extend(
            screened
                .as_ref()
                .map(project::Screened::setting_sources)
                .unwrap_or_default(),
        );
        StateHooks::ThisSessionOnly {
            args: [
                args,
                words([
                    // charter's MCP server (HP-7), before `--settings`: `--mcp-config` takes every
                    // word up to the next flag, and the chat's own words end the line.
                    "--mcp-config",
                    &mcp_config(kit),
                    "--settings",
                    &settings(
                        kit.binary,
                        cwd,
                        crate::footerclaim::status_line(cwd).free(),
                        &handed,
                        sandbox,
                        &denied_tools(kit),
                        screened.as_ref(),
                    ),
                ]),
            ]
            .concat(),
            env: vec![(
                crate::plugin::BINARY_ENV.to_owned(),
                kit.binary.display().to_string(),
            )],
            cannot_report: Vec::new(),
        }
    }

    // `disarmed_by`: none. Claude Code loads `--plugin-dir` whatever else it is told.

    /// **Measured on Claude Code 2.1.288**, by reading the parser in its own bundle (FM-9): a
    /// mention is `@` at the start of the input or after white space, then either `"…"` or a run
    /// of non-space characters that ends on a word character; `#L<first>` or `#L<first>-<last>`
    /// after the name names lines; a folder's mention hands the model its listing. So a name with
    /// white space, or one ending on another character, is quoted, and a folder ends in `/` as
    /// its own completion writes it. A `#` in the path would be read as the start of a line
    /// range, and a `"` cannot be quoted, so such a path is handed over in plain words, which the
    /// agent reads with its own tools.
    fn reference(&self, reference: &crate::reference::Reference) -> String {
        if reference.path().contains('#') {
            return reference.plain();
        }
        let body = format!(
            "{}{}",
            reference.path_with_slash(),
            reference.lines_after("#L", "")
        );
        let ends_on_a_word = body
            .trim_end_matches('/')
            .chars()
            .last()
            .is_some_and(|c| c.is_ascii_alphanumeric() || c == '_');
        if ends_on_a_word && !body.chars().any(char::is_whitespace) {
            format!("@{body}")
        } else if !body.contains('"') {
            format!("@\"{body}\"")
        } else {
            reference.plain()
        }
    }

    fn armed_with(&self) -> String {
        format!(
            "the app arms each chat with its own plugin, {}",
            crate::plugin::LOADED_AS
        )
    }

    fn plugins(&self) -> &'static dyn crate::harness_plugin::Adapter {
        &crate::harness_plugin::CLAUDE_CODE
    }

    fn guard_label(&self) -> &'static str {
        "claude-code"
    }

    /// Claude Code's `Skill` tool, which names the skill in `skill` (a plugin's own as
    /// `plugin:name`).
    fn skill_start(&self, tool: &str, input: &serde_json::Value) -> Option<SkillStart> {
        (tool == "Skill")
            .then(|| input_text(input, "skill"))
            .flatten()
            .map(|skill| SkillStart::Named(skill.to_owned()))
    }

    /// Every skill of a chat's set reaches Claude Code as a plugin's (ADR 0063 as amended
    /// 2026-10-20): a built-in one as purlis's plugin's, `purlis:<name>`, and one of the plane's
    /// as the folder of links' plugin's, `skills:<name>` ([`crate::skills::LINKED_PLUGIN`]). The
    /// namespaced name is the one that cannot be taken by a person's own skill of that name.
    /// A harness or repo skill is loaded by Claude Code itself, by its own name.
    fn skill_name(&self, name: &str, origin: crate::skills::Origin) -> String {
        use crate::skills::Origin;
        match origin {
            Origin::BuiltIn => format!("{}{name}", crate::names::SKILL_NAMESPACE.write),
            Origin::Project | Origin::Workspace | Origin::Persona => {
                format!("{}:{name}", crate::skills::LINKED_PLUGIN)
            }
            Origin::Harness | Origin::Repo => name.to_owned(),
        }
    }

    /// **Claude Code 2.1.304** (research of 2026-10-20, `skills.md` "Invoke a skill" and a
    /// measured `-p` run): `/<plugin>:<name>` at the start of the message runs the skill, and
    /// what follows is its arguments, `$ARGUMENTS` raw and `$N` (and each `arguments:` name) split
    /// as shell words, quotes kept. So each value is one shell word, in the declared order.
    fn skill_start_text(&self, skill: &SkillToStart<'_>) -> String {
        let head = format!("/{}", self.skill_name(skill.name, skill.origin));
        slash_with_words(&head, skill.given)
    }

    /// Claude Code lists skills within 1% of the context window (`skillListingBudgetFraction`),
    /// about 8,000 characters on a 200k-token window for names, descriptions and their lines,
    /// cutting each description at 1,536 characters and dropping descriptions past the budget
    /// (2.1.304, `settings-reference.md`, `skills.md`). It leaves a manual-only skill out itself
    /// ([`Self::skill_frontmatter`]).
    fn skill_budget(&self) -> crate::skills::Budget {
        crate::skills::Budget {
            per_description: Some(1_536),
            ..super::SKILL_BUDGET
        }
    }

    /// **Claude Code 2.1.304** (`skills.md` "Frontmatter reference", research of 2026-10-20):
    /// every key a skill may set, a plugin's skills included, and `disable-model-invocation`
    /// keeps the skill out of the model's choice and its description out of the listing.
    /// `allowed-tools` and `hooks` are among them, which is why purlis never hands either over
    /// ([`crate::skills::handed_text`]); `license`, `compatibility` and `metadata` are accepted and
    /// ignored.
    fn skill_frontmatter(&self) -> crate::skills::Honours {
        use crate::skills::Field;
        crate::skills::Honours {
            reads: &[
                Field::WhenToUse,
                Field::ManualOnly,
                Field::UserInvocable,
                Field::AllowedTools,
                Field::DisallowedTools,
                Field::Model,
                Field::Effort,
                Field::Context,
                Field::Agent,
                Field::Hooks,
                Field::Paths,
                Field::Shell,
                Field::Background,
            ],
            manual: crate::skills::ManualOnly::Read,
            // Its own agents (`sub-agents.md`, 2.1.304), which carry no settings of a project's.
            agents: &["general-purpose", "Explore", "Plan"],
        }
    }

    /// Claude Code fires `UserPromptExpansion` as a `/<name>` the person sent expands into its
    /// skill, naming it ([`Self::prompt_started`]).
    fn typed_start_named(&self) -> bool {
        true
    }

    /// **Claude Code 2.1.304** (`hooks.md` "UserPromptExpansion", and a measured payload of
    /// 2026-10-20): `{"hook_event_name": "UserPromptExpansion", "expansion_type":
    /// "slash_command", "command_name": "<plugin>:<name>", ...}`. A skill's command is its name
    /// as [`Self::skill_name`] says it.
    fn prompt_started(&self, payload: &serde_json::Value) -> Option<String> {
        let said = |key: &str| payload.get(key).and_then(serde_json::Value::as_str);
        (said("hook_event_name") == Some("UserPromptExpansion")
            && said("expansion_type") == Some("slash_command"))
        .then(|| said("command_name"))
        .flatten()
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(str::to_owned)
    }

    /// Claude Code's `Skill` tool, by the plugin the skill came in ([`Self::skill_name`]):
    /// `purlis:<name>` for a built-in skill, `skills:<name>` for the plane's. Claude Code applies
    /// a skill's `disallowed-tools` and its fork when it starts the skill itself.
    fn narrowed_start(&self, name: &str, origin: crate::skills::Origin) -> Option<String> {
        Some(self.skill_name(name, origin))
    }

    /// `permissions.<bucket>` of `.claude/settings.json`, or with `local` of
    /// `.claude/settings.local.json`, which git does not carry. Every rule
    /// [`crate::guardcmd::as_rule`] writes is one Claude Code reads.
    fn command_rule(
        &self,
        root: &std::path::Path,
        rule: &str,
        local: bool,
    ) -> Result<RuleFile, String> {
        use crate::scaffold::settings;
        let path = root.join(if local {
            ".claude/settings.local.json"
        } else {
            settings::SETTINGS
        });
        let at = path.clone();
        let rule = rule.to_owned();
        Ok(RuleFile::new(path, move |bucket, dry_run| {
            settings::ensure_rule(&at, bucket, &rule, dry_run)
        }))
    }

    /// A `sandbox` object and `permissions.deny` rules ([`crate::sandbox::claude`]).
    fn sandbox_compiler(&self) -> Option<crate::sandbox::Compiler> {
        Some(|compiled| crate::sandbox::claude::settings(compiled).map(Form::ClaudeCode))
    }

    /// No: Claude Code's sandbox confines the commands its Bash tool runs, and its hooks and
    /// MCP servers run in its own process's reach, outside it ([`crate::sandbox::claude`]).
    fn sandbox_holds_what_it_starts(&self) -> bool {
        false
    }

    /// The line as it is: Claude Code's sandbox rides in the `--settings` charter hands it (in
    /// `armed`), beside the project settings its arming decided to load ([`project`], ADR 0067
    /// as amended 2026-10-15), and there it is allowed the chat's hook socket, which only the
    /// place the chat opens knows (`at`).
    /// Its own flags are not asked about yet.
    fn sandboxed_line(
        &self,
        form: &Form,
        words: crate::sandbox::Words,
        at: &crate::sandbox::At<'_>,
    ) -> Result<crate::sandbox::Line, String> {
        match form {
            Form::ClaudeCode(compiled) => Ok(crate::sandbox::Line {
                program: words.program,
                args: [
                    words.command,
                    reporting_on(
                        words.armed,
                        compiled,
                        at.hook_socket,
                        at.confinement.map(crate::sandbox::Confinement::proxy_ports),
                    )?,
                    words.charters,
                ]
                .concat(),
                env: Vec::new(),
            }),
            _ => Err(super::adapter::not_compiled_for(Harness::ClaudeCode)),
        }
    }
}

/// `armed` with the sandbox and deny rules in its `--settings` allowed to reach `socket`
/// ([`crate::sandbox::claude::Settings::reporting_on`]), and its network sent through
/// purlis's proxy on `ports` where the chat has them (#1665,
/// [`crate::sandbox::claude::Settings::through`]); or as it is with neither.
///
/// **Fail closed.** A `--settings` that is missing, is not JSON, or does not carry `compiled`
/// as [`ClaudeCode::arm_under`] wrote it refuses the chat: neither the socket nor the ports are
/// ever added to a sandbox this adapter did not compile.
pub(crate) fn reporting_on(
    mut armed: Vec<String>,
    compiled: &crate::sandbox::claude::Settings,
    socket: Option<&std::path::Path>,
    ports: Option<[u16; 2]>,
) -> Result<Vec<String>, String> {
    if socket.is_none() && ports.is_none() {
        return Ok(armed);
    }
    let not_carried = || {
        "this Claude Code chat's settings do not carry the sandbox compiled for it, so nothing \
         was started"
            .to_owned()
    };
    let at = armed
        .iter()
        .position(|word| word == "--settings")
        .map(|flag| flag + 1)
        .filter(|&at| at < armed.len())
        .ok_or_else(not_carried)?;
    let mut settings: serde_json::Value =
        serde_json::from_str(&armed[at]).map_err(|_| not_carried())?;
    if settings.get("sandbox") != Some(&compiled.sandbox) {
        return Err(not_carried());
    }
    // The compiled deny rules lead the list; the operator's rule twins may follow them.
    let deny = settings
        .pointer_mut("/permissions/deny")
        .and_then(serde_json::Value::as_array_mut)
        .filter(|deny| {
            deny.len() >= compiled.deny.len()
                && deny
                    .iter()
                    .zip(&compiled.deny)
                    .all(|(had, rule)| had == rule)
        })
        .ok_or_else(not_carried)?;
    let mut reporting = compiled.reporting_on(socket);
    if let Some(ports) = ports {
        reporting = reporting.through(ports);
    }
    deny.extend(
        reporting.deny[compiled.deny.len()..]
            .iter()
            .map(|rule| serde_json::json!(rule)),
    );
    settings["sandbox"] = reporting.sandbox;
    armed[at] = settings.to_string();
    Ok(armed)
}

/// The settings a Claude Code chat is started with, as JSON on the argument.
///
/// On the argument and not in a file: a file would have to be written somewhere, cleaned up
/// when the chat ends, and cleaned up again after an app that crashed. What is in it is a
/// plugin id and a path — nothing secret, so `ps` showing it costs nothing.
///
/// **One hook, the permission hook, and no other.** Every hook that reports or guards is the
/// bundled plugin's (`hooks/hooks.json`), so a chat has one place those are declared and none is
/// armed twice. The permission hook ([`permission_hook`]) can allow, so it is never put in a
/// file a chat can write; it rides on this argument instead.
fn settings(
    binary: &std::path::Path,
    cwd: Option<&std::path::Path>,
    may_fill_the_footer: bool,
    plugins: &crate::harness_plugin::Chosen,
    sandbox: Option<&crate::sandbox::claude::Settings>,
    denied_tools: &[String],
    screened: Option<&project::Screened>,
) -> String {
    let mut settings = serde_json::Map::new();
    // The project's own choice of Claude Code's plugins first (charter-app#274, ADR 0050): a
    // session `enabledPlugins` wins over the project's and the user's for the keys it names,
    // and leaves every other plugin to them.
    let mut enabled: serde_json::Map<String, serde_json::Value> = plugins
        .plugins
        .iter()
        .map(|(id, on)| (id.clone(), (*on).into()))
        .collect();
    // Then the pins, written last so nothing above can move them
    // ([`crate::harness_plugin::CLAUDE_CODE`] holds them and says why). The operator's ruling of
    // 2026-09-23: a chat the app starts turns the Python charter's plugin off for itself, so the
    // project files that enable it for the operator's own terminal sessions do not give an app
    // chat two sets of hooks and two `handoff` skills. And the bundled plugin pinned on: a
    // project file can turn a `--plugin-dir` plugin off by its id, a chat can write that file,
    // and the plugin carries the Bash guard. Measured on 2.1.280: this `true` wins over a
    // project's `false`.
    for pin in ADAPTER.plugins().pinned() {
        enabled.insert(pin.id.to_owned(), pin.on.into());
    }
    settings.insert(
        "enabledPlugins".to_owned(),
        serde_json::Value::Object(enabled),
    );
    // **The person's own skills the project turned off** (SK-6, #2769): `skillOverrides`, one
    // name to `"off"` each, for this session alone. Measured on 2.1.304: `--settings
    // '{"skillOverrides":{"langfuse":"off"}}'` took `langfuse` out of the init event's skills.
    // It does not reach a plugin's skill (`<plugin>:<skill>`), which goes only with its plugin,
    // so none is written, and nor is a built-in skill's name ([`crate::skills::skills_off_for`]).
    // Not written at all where none is off, so the operator's own overrides stand.
    let overrides: serde_json::Map<String, serde_json::Value> = plugins
        .skills_off
        .iter()
        .map(|name| (name.clone(), "off".into()))
        .collect();
    if !overrides.is_empty() {
        settings.insert(
            "skillOverrides".to_owned(),
            serde_json::Value::Object(overrides),
        );
    }
    // **Only where nothing else fills it.** The key is one value and the flag is the last
    // writer, so arming it where the operator has their own would stop theirs running
    // ([`crate::footerclaim`], measured on 2.1.280). Where charter does not arm it, the chat
    // records no turns and its `ctx`/`cache` gauge stays dark — which `doctor` reports.
    if may_fill_the_footer {
        settings.insert("statusLine".to_owned(), status_line(binary));
    }
    // **The one command that ends a Smart close, pre-allowed** (SI-8e, the
    // operator's ruling of 2026-09-28): a chat asked to write its record must not stop on a
    // permission prompt for `purlis session record`. Only `allow`, and no `ask`, `deny` or
    // mode, because Claude Code merges a session's permission rules with the user's and the
    // project's rather than replacing them, and its `deny` and `ask` outrank an `allow` — so
    // the operator's own rules all still stand (measured on 2.1.283 in ADR 0064: a project
    // `deny` still refused, a user `allow` still allowed, and a compound command that holds the
    // record command beside another was still asked about).
    //
    // **Its twin on charter's MCP server, `session_record`, the same way** (#1332): the one
    // operation, through the other entrance (ADR 0067 §2), so the skill that calls the tool
    // stops on no prompt either.
    //
    // **And the five read-only tools of charter's own MCP server** (V79, #1050, amending
    // SI-8e): a chat reading its own todos, memory, records or change status does not stop on
    // a prompt. `persona_where` is allowed with them (V98a, #1450): it reads the app's own
    // record of the open chats and answers what the chat is told at its start. Each by its
    // full name, never the server as a whole, so the other writes and `ask_operator` still
    // ask, and an operator's `ask` or `deny` for any of them still wins.
    //
    // **And a dispatch, by its command and by its two tools** (operator's ruling V98b,
    // D-T59-prealllow, amending ADR 0064): the harness's prompt is not what consents to a
    // dispatch. purlis's own dispatch grant is, asked of the person by the app, the same on
    // every harness (spec #1434, decision 4), so the harness asking as well would ask twice
    // for one thing and, for a chat's own persona, ask where nothing is at stake. **A handoff
    // is a dispatch in handoff mode** (#1444), so its command is allowed on the same terms.
    let mut allow = vec![
        SMART_CLOSE_ALLOW.to_owned(),
        format!(
            "mcp__{}__{}",
            crate::chattools::SERVER,
            crate::chattools::SESSION_RECORD
        ),
    ];
    allow.extend(DISPATCH_ALLOW.iter().map(|rule| (*rule).to_owned()));
    // And what a chat asks after a task it dispatched, each by its own name (D-T59-j8).
    allow.extend(DISPATCH_TASK_ALLOW.iter().map(|rule| (*rule).to_owned()));
    allow.extend(HANDOFF_ALLOW.iter().map(|rule| (*rule).to_owned()));
    // And purlis's read-only commands, by name, beside their tools (reported 2026-10-09).
    allow.extend(READ_ALLOW.iter().map(|rule| (*rule).to_owned()));
    // And a commit the app makes for a sandboxed chat in its branch folder (#1476).
    allow.extend(WORKTREE_COMMIT_ALLOW.iter().map(|rule| (*rule).to_owned()));
    allow.extend(
        crate::chattools::DISPATCH_TOOLS
            .iter()
            .map(|tool| format!("mcp__{}__{tool}", crate::chattools::SERVER)),
    );
    allow.extend(
        crate::chattools::PRE_ALLOWED
            .iter()
            .map(|tool| format!("mcp__{}__{tool}", crate::chattools::SERVER)),
    );
    // **And a chat's browser tools, each by name** (#2245, ruled 2026-10-16, amending ADR
    // 0064): the person's answer to the chat's browser Ask is the consent, which the app asks
    // at the first call and holds in front of every one, as the dispatch grant is for a
    // dispatch. On the same arguments as the `--mcp-config` that hands the chat the server
    // that serves them, so only a chat that may have a browser carries them.
    allow.extend(
        crate::browsertools::run_on_the_browser_ask()
            .map(|tool| format!("mcp__{}__{tool}", crate::chattools::SERVER)),
    );
    let mut permissions = serde_json::json!({ "allow": allow });
    // **The sandbox, where the plane turned it on** (ADR 0067), and the deny rules that keep
    // Claude Code's own Read and Edit tools out of what its sandbox denies a command
    // ([`crate::sandbox::claude`]). A deny outranks the allow above, and they name different
    // things. Absent otherwise, never `enabled: false`: an unsandboxed chat is left to the
    // operator's own settings, as it was.
    if let Some(sandbox) = sandbox {
        settings.insert("sandbox".to_owned(), sandbox.sandbox.clone());
        permissions["deny"] = serde_json::json!(sandbox.deny);
    }
    // **The tools the chat's persona denies it** (#1451, D-1451-18): its `disallowed-tools:`,
    // which the generated sub-agent carried as `disallowedTools:`. A deny outranks every
    // allow, here and in the project's own settings, so the chat cannot be given them back by
    // a file it can write. Sandboxed or not: the rule is the persona's, not the sandbox's.
    if !denied_tools.is_empty() {
        let mut rules: Vec<serde_json::Value> = permissions
            .get("deny")
            .and_then(serde_json::Value::as_array)
            .cloned()
            .unwrap_or_default();
        rules.extend(denied_tools.iter().cloned().map(serde_json::Value::String));
        permissions["deny"] = serde_json::Value::Array(rules);
    }
    // **The project's own deny and ask rules, where its files are not loaded** (ADR 0067 as
    // amended 2026-10-15): they can only make the chat refuse or ask more, so they still apply,
    // after the compiled rules, which lead the list.
    if let Some(screened) = screened.filter(|screened| !screened.loads_the_project()) {
        for (bucket, rules) in [("deny", &screened.deny), ("ask", &screened.ask)] {
            if rules.is_empty() {
                continue;
            }
            let mut had: Vec<serde_json::Value> = permissions
                .get(bucket)
                .and_then(serde_json::Value::as_array)
                .cloned()
                .unwrap_or_default();
            had.extend(rules.iter().cloned().map(serde_json::Value::String));
            permissions[bucket] = serde_json::Value::Array(had);
        }
    }
    // **The operator's ask and deny on charter's tools, under the server's new name**
    // (D-RN8-12). The server is `purlis` now (#1266), so a project's or a layer's rule on
    // `mcp__charter__<tool>` matches nothing; its `mcp__purlis__<tool>` twin rides here, beside
    // the rule, so the operator's ask or deny still wins (ADR 0064). Only asks and denies, which
    // can only make a chat ask more; an allow gets no twin.
    let (ask, deny) = crate::scaffold::settings::mcp_rule_twins_in(&project_settings_files(cwd));
    for (bucket, twins) in [("ask", ask), ("deny", deny)] {
        if twins.is_empty() {
            continue;
        }
        let mut rules: Vec<serde_json::Value> = permissions
            .get(bucket)
            .and_then(serde_json::Value::as_array)
            .cloned()
            .unwrap_or_default();
        rules.extend(twins.into_iter().map(serde_json::Value::String));
        permissions[bucket] = serde_json::Value::Array(rules);
    }
    settings.insert("permissions".to_owned(), permissions);
    settings.insert("hooks".to_owned(), permission_hook(binary));
    serde_json::Value::Object(settings).to_string()
}

/// The servers a chat is started with, as `--mcp-config` takes them: purlis's own
/// ([`crate::chattools::claude_code_config`]) and, for a chat that runs as a persona, that
/// persona's ([`crate::personaverbs::chatstart::servers`], #1451 D-1451-17). A credentialed
/// server is there only once this machine approved it, wrapped in `secret exec`; purlis's own
/// is written last, so no entry of a persona's file can take its name.
fn mcp_config(kit: Kit<'_>) -> String {
    let mut config: serde_json::Value =
        serde_json::from_str(&crate::chattools::claude_code_config(kit.binary))
            .unwrap_or_else(|_| serde_json::json!({ "mcpServers": {} }));
    let Some(of) = kit.persona else {
        return config.to_string();
    };
    let state = crate::personaverbs::state_dir(of.root);
    let theirs =
        crate::personaverbs::chatstart::servers(of.root, &state, of.persona, kit.binary).started;
    if let Some(servers) = config["mcpServers"].as_object_mut() {
        let own = std::mem::take(servers);
        servers.extend(theirs);
        servers.extend(own);
    }
    config.to_string()
}

/// The tools the chat's persona denies it ([`crate::personaverbs::chatstart::denied_tools`]).
fn denied_tools(kit: Kit<'_>) -> Vec<String> {
    kit.persona.map_or_else(Vec::new, |of| {
        crate::personaverbs::chatstart::denied_tools(of.root, of.persona)
    })
}

/// The Claude Code settings files a chat at `cwd` reads from its project: `.claude/settings.json`
/// and `.claude/settings.local.json` in `cwd` and in each folder above it, up to the nearer of
/// the project's root (a project manifest) and the first `.git`. A `cwd` in no project and no
/// repository is read alone: the walk never climbs to `/` or into the home directory.
fn project_settings_files(cwd: Option<&std::path::Path>) -> Vec<std::path::PathBuf> {
    let Some(cwd) = cwd else {
        return Vec::new();
    };
    let files = |dir: &std::path::Path| {
        [".claude/settings.json", ".claude/settings.local.json"].map(|file| dir.join(file))
    };
    let mut out = Vec::new();
    for dir in cwd.ancestors() {
        out.extend(files(dir));
        if crate::names::has_manifest(dir) || dir.join(".git").exists() {
            return out;
        }
    }
    files(cwd).to_vec()
}

/// Claude Code's `PermissionRequest` hook, pointed at `charter hook permissionrequest` for THIS
/// session only (HP-6): the harness's permission prompt is also an ask in charter's window, and
/// the operator's answer there goes back on the hook ([`crate::harness::hooked`]). No matcher,
/// so every tool's prompt is asked; the timeout is the one the ask's deadline sits below.
///
/// **Armed on the argument, never in the plugin.** A hook that can allow a permission must not
/// be taken from a file a chat can write (`plugin::tests`). Nothing it does allows by itself:
/// with no answer from the window it prints nothing, and the pane asks as it always did.
fn permission_hook(binary: &std::path::Path) -> serde_json::Value {
    serde_json::json!({
        "PermissionRequest": [{
            "hooks": [{
                "type": "command",
                "command": format!(
                    "{} hook {}",
                    crate::plugin::shell_quoted(&binary.display().to_string()),
                    super::hooked::WORD
                ),
                "timeout": super::hooked::HOOK_TIMEOUT.as_secs(),
            }]
        }]
    })
}

/// The permission rule a Claude Code chat the app starts carries for Smart close: `purlis
/// session record`, with any arguments, runs without asking (SI-8e, ADR 0064). Beside it, the
/// read-only charter tools of [`crate::chattools::PRE_ALLOWED`] (V79).
pub const SMART_CLOSE_ALLOW: &str = "Bash(purlis session record *)";

/// The permission rules a Claude Code chat the app starts carries for **purlis's read-only
/// commands that only answer from the app's own record of the chat** (reported 2026-10-09,
/// on the terms of V98a and ADR 0064): a task whose brief ran `purlis persona where` stopped
/// on the harness's prompt in a tab nobody was at, and waited there on the person.
///
/// Each is the command twin of a tool already in [`crate::chattools::PRE_ALLOWED`]: one
/// operation, two entrances, allowed the same way (as `session_record` and its command are).
/// **The whole line, with no wildcard**: the command takes no argument a chat would pass, so a
/// line with anything more after it is asked about, and so is every other `persona` word.
pub const READ_ALLOW: [&str; 1] = ["Bash(purlis persona where)"];

/// The permission rule a Claude Code chat the app starts carries for **a commit the app makes
/// for it in the branch folder it stands in** (#1476, ADR 0067 as amended for #1055, on the
/// terms of ADR 0064 and V98b): `purlis worktree commit`, by its two words.
///
/// The commit asks no approval (ADR 0067's decision 6): it is the chat's own work in its own
/// folder on its own branch, as `git commit` in a clone is, which no rule asks about either. A
/// chat that is not sandboxed, or in a clone, is told to use git, and nothing is committed.
/// **Alone only**: a call that runs anything beside it, or writes a file beside it, is asked
/// about as a whole ([`crate::dispatchguard::rider_ask`]), and an operator's own `ask` or
/// `deny` still wins. Every other `worktree` word still asks.
pub const WORKTREE_COMMIT_ALLOW: [&str; 1] = ["Bash(purlis worktree commit *)"];

/// The permission rules a Claude Code chat the app starts carries for a dispatch (V98b,
/// amending ADR 0064): the task, by each flag its line can start with, and its report.
/// **Consent is the dispatch grant**, which purlis asks the person for itself; this only stops
/// the harness asking beside it.
///
/// **Each spelling by name, and no bare trailing wildcard.** `purlis dispatch *` would also
/// allow every subcommand `dispatch` grows later, which nobody has ruled on: a new one is added
/// here by name when it lands. A flag-first line cannot reach a subcommand (the command's
/// arguments and its subcommands exclude each other), so these four are the task and the
/// report and nothing else. An operator's own `ask` or `deny` still wins, as for
/// [`SMART_CLOSE_ALLOW`].
pub const DISPATCH_ALLOW: [&str; 4] = [
    "Bash(purlis dispatch --name *)",
    "Bash(purlis dispatch --to *)",
    "Bash(purlis dispatch --profile *)",
    "Bash(purlis dispatch report *)",
];

/// The permission rules a Claude Code chat the app starts carries for **what it asks after a
/// task it dispatched, and what a task sends the chat that asked** (#1441, #1442, D-T59-j8,
/// amending ADR 0064 under V98b): each subcommand by its own name, pinned, so a word added to
/// `purlis dispatch` later still asks until it is ruled in.
///
/// **Each reaches only what the app's own record lets its caller reach**
/// (`crate::dispatched::owned`, `crate::dispatchtalk::up`): `wait`, `cancel`, `tell` and
/// `answer` name a task the chat itself dispatched, and are refused for any other chat;
/// `list` lists those tasks; `read` reads what one of them said and ran (#1981, #2084); `note` and `ask` name nobody, and go to the one chat that
/// dispatched the caller. None starts a chat. `--wait` is the dispatch itself, waited on: it
/// is matched only where the flag comes first, and asked about otherwise.
pub const DISPATCH_TASK_ALLOW: [&str; 9] = [
    "Bash(purlis dispatch --wait *)",
    "Bash(purlis dispatch wait *)",
    "Bash(purlis dispatch list)",
    "Bash(purlis dispatch read *)",
    "Bash(purlis dispatch cancel *)",
    "Bash(purlis dispatch tell *)",
    "Bash(purlis dispatch note *)",
    "Bash(purlis dispatch ask *)",
    "Bash(purlis dispatch answer *)",
];

/// The permission rules a Claude Code chat the app starts carries for a handoff (#1444, on
/// V98b's terms): the handoff, by each flag its line can start with, and its report back
/// (`purlis handoff report …`). A handoff is a dispatch in handoff mode, and **consent is the
/// dispatch grant**: none for the chat's own persona, and for another the app asks the person
/// once, with the brief in front of them, and starts nothing until they allow it. This only
/// stops the harness asking beside that.
///
/// **Each spelling by name, and no bare trailing wildcard**, for [`DISPATCH_ALLOW`]'s reason
/// (D-T59-18): `purlis handoff *` would also allow whatever the command grows later. So the
/// spelling the handoff skill writes puts a flag first,
/// `purlis handoff --name "<task>" <workspace>`, and a line that starts with the workspace is
/// the harness's own to ask about, as any command it has no rule for is. A project that still
/// carries the `ask` rule an older `init` wrote keeps being asked until the `handoff-rule` fix
/// removes it, and an operator's own `ask` or `deny` still wins.
pub const HANDOFF_ALLOW: [&str; 6] = [
    "Bash(purlis handoff --name *)",
    "Bash(purlis handoff --report *)",
    "Bash(purlis handoff --persona *)",
    "Bash(purlis handoff --create *)",
    "Bash(purlis handoff --vision *)",
    "Bash(purlis handoff report *)",
];

/// Claude Code's `statusLine`, pointed at `charter statusline` — for THIS session only.
///
/// **This is how a chat's `ctx`/`cache` history gets written at all, and without it the app's
/// gauge is a renderer with nothing to render.** Claude Code hands `context_window` — the
/// context percentage and the cache numbers — to its `statusLine` command and to nothing
/// else: no hook payload carries them (charter ADR 0019, measured). `charter statusline`
/// is the command that writes them down (`usage::record`), on every render, whether or not
/// it draws anything. And since charter 0.57.0 (#895) nothing puts that command in a
/// session's settings any more, while the app arms only hooks — so, measured on 2026-09-22,
/// the operator's own plane had not had a turn recorded since 2026-09-04, and every chat the
/// app started ran with no gauge on any surface.
///
/// **What the chat SEES does not change by default.** Inside the app `charter statusline`
/// prints an empty line (ADR 0019 transposed, `purlis-cli/src/statusline.rs`), which is
/// the footer the app's chats were already meant to have — and ADR 0029's per-chat checkbox,
/// which sets `CHARTER_FOOTER=show`, draws charter's footer instead. That checkbox was a
/// switch on a command nothing invoked; this is what makes it one.
///
/// **It is armed only where the operator fills the line with nothing**, and that is the
/// operator's own ruling of 2026-09-22. `--settings` merges key by key and `statusLine` is one
/// key, so arming it over somebody's own command would stop theirs from running, silently, for
/// every chat the app starts — measured on 2.1.280, four cells, one launch each
/// ([`crate::footerclaim`], which holds the measurement and the rule). Where something else
/// fills it, charter arms nothing, leaves their configuration alone, and lets `doctor` say why
/// the gauge is dark. Nothing here wraps or chains their command: charter would then own its
/// failures and its latency, every turn.
///
/// **What it costs otherwise, said:** nothing is written to any file, their own `claude` in a
/// terminal is untouched, and a command costs one process per footer render — which Claude
/// Code runs at startup and once per submitted turn, not on a timer (measured: zero
/// invocations over thirty idle seconds).
fn status_line(binary: &std::path::Path) -> serde_json::Value {
    serde_json::json!({
        "type": "command",
        "command": format!(
            "{} statusline",
            crate::plugin::shell_quoted(&binary.display().to_string())
        ),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_reference_is_written_as_claude_codes_own_mention() {
        use crate::reference::{Lines, Reference};
        let r = |path, lines, folder| ADAPTER.reference(&Reference::for_tests(path, lines, folder));
        assert_eq!(r("src/main.rs", None, false), "@src/main.rs");
        assert_eq!(r("src", None, true), "@src/");
        assert_eq!(
            r(
                "src/main.rs",
                Some(Lines {
                    first: 10,
                    last: 20
                }),
                false
            ),
            "@src/main.rs#L10-20"
        );
        assert_eq!(
            r("src/main.rs", Some(Lines::one(7)), false),
            "@src/main.rs#L7"
        );
        assert_eq!(
            r("my dir/note one.txt", Some(Lines::one(2)), false),
            "@\"my dir/note one.txt#L2\""
        );
        assert_eq!(r("lib/c++", None, false), "@\"lib/c++\"");
        assert_eq!(r("out-", None, true), "@\"out-/\"");
        assert_eq!(r("/abs/x.rs", None, false), "@/abs/x.rs");
        assert_eq!(r("a#b.rs", None, false), "a#b.rs");
    }

    /// Claude Code 2.1.288 turns a paste beginning with `!` into an empty input into bash mode,
    /// and reads `/…` as a slash command: no reference begins with either, its plain-words
    /// fallback (a `#` or a `"` in the path) included.
    #[test]
    fn a_reference_never_begins_as_bash_mode_or_a_slash_command() {
        use crate::reference::{Reference, starts_safely};
        let r = |path| ADAPTER.reference(&Reference::for_tests(path, None, false));
        assert_eq!(r("!cmd${IFS}x#y"), "./!cmd${IFS}x#y");
        assert_eq!(r("!cmd"), "@./!cmd");
        assert_eq!(r("/abs/a#b"), "\"/abs/a#b\"");
        for path in [
            "!x",
            "!x#y",
            "/abs/x",
            "/abs/a#b",
            "/abs/a\"b c",
            "#x",
            "-x",
        ] {
            assert!(starts_safely(&r(path)), "{path} -> {}", r(path));
        }
    }

    fn adapter() -> &'static dyn HarnessAdapter {
        &ADAPTER
    }

    #[test]
    fn a_sandbox_compiled_for_another_harness_arms_no_claude_code_chat() {
        // ADR 0067, fail closed: a Codex sandbox handed to the Claude Code adapter would
        // otherwise start a Claude Code chat with no sandbox at all. The adapter itself
        // refuses it, not only `Harness::state_hooks`.
        let (plane, codex) = crate::harness::testing::sandbox_compiled_for(Harness::Codex);
        let codex = codex.expect("starts");

        let hooks = adapter().arm(
            Kit {
                skills: None,
                laid_out: None,
                binary: std::path::Path::new("/bin/charter"),
                plugin: Some(std::path::Path::new("/app/plugin")),
                persona: None,
            },
            Some(plane.path()),
            &crate::harness_plugin::Chosen::new(),
            Some(&codex),
        );
        assert_eq!(hooks, StateHooks::None);
    }

    #[test]
    fn the_claude_code_adapter_arms_claude_code() {
        assert_eq!(adapter().harness(), Harness::ClaudeCode);
    }

    #[test]
    fn the_claude_code_adapter_arms_a_chat_with_the_bundled_plugin_for_that_chat_alone() {
        let empty = tempfile::tempdir().expect("a directory");
        let hooks = adapter().arm(
            Kit {
                skills: None,
                laid_out: None,
                binary: std::path::Path::new("/bin/charter"),
                plugin: Some(std::path::Path::new("/app/plugin")),
                persona: None,
            },
            Some(empty.path()),
            &crate::harness_plugin::Chosen::new(),
            None,
        );
        let StateHooks::ThisSessionOnly {
            args,
            env,
            cannot_report,
        } = hooks
        else {
            panic!("armed per session");
        };
        assert_eq!(args[..2], ["--plugin-dir", "/app/plugin"]);
        assert!(args.contains(&"--settings".to_owned()), "{args:?}");
        assert_eq!(
            env,
            [("CHARTER_HOOK_BINARY".to_owned(), "/bin/charter".to_owned())]
        );
        assert!(cannot_report.is_empty());
    }

    #[test]
    fn a_claude_code_chat_is_handed_charter_s_mcp_server_for_that_chat_alone() {
        // HP-7: `--mcp-config` loads a server for this session and writes nothing.
        let StateHooks::ThisSessionOnly { args, .. } = adapter().arm(
            Kit {
                skills: None,
                laid_out: None,
                binary: std::path::Path::new("/bin/charter"),
                plugin: Some(std::path::Path::new("/app/plugin")),
                persona: None,
            },
            None,
            &crate::harness_plugin::Chosen::new(),
            None,
        ) else {
            panic!("armed per session");
        };
        let at = args
            .iter()
            .position(|arg| arg == "--mcp-config")
            .expect("--mcp-config");
        let config: serde_json::Value = serde_json::from_str(&args[at + 1]).expect("JSON");
        assert_eq!(
            config["mcpServers"]["purlis"],
            serde_json::json!({"type": "stdio", "command": "/bin/charter", "args": ["mcp"]})
        );
        // `--mcp-config` takes every word up to the next flag, so a flag follows it and the
        // chat's own words that end the line are never read as a config.
        assert!(args[at + 2].starts_with("--"), "{args:?}");
    }

    /// The words a chat as `persona` in `plane` is armed with: its `--mcp-config` and its
    /// `--settings`, parsed.
    fn armed_as(
        plane: &crate::personaverbs::tests_plane::Plane,
        persona: &str,
    ) -> (serde_json::Value, serde_json::Value) {
        let StateHooks::ThisSessionOnly { args, .. } = adapter().arm(
            Kit {
                skills: None,
                laid_out: None,
                binary: std::path::Path::new("/bin/charter"),
                plugin: Some(std::path::Path::new("/app/plugin")),
                persona: Some(crate::harness::As {
                    root: plane.root(),
                    persona,
                }),
            },
            Some(plane.root()),
            &crate::harness_plugin::Chosen::new(),
            None,
        ) else {
            panic!("armed per session");
        };
        let word = |flag: &str| {
            let at = args.iter().position(|arg| arg == flag).expect(flag);
            serde_json::from_str::<serde_json::Value>(&args[at + 1]).expect("JSON")
        };
        (word("--mcp-config"), word("--settings"))
    }

    #[test]
    fn a_persona_chat_is_started_with_its_persona_s_approved_mcp_servers_and_no_other() {
        // #1451, D-1451-17: the generated sub-agent carried these servers, and it is retired.
        // The chat that runs as the persona is started with them, on the same `--mcp-config`
        // that carries purlis's own server, for that chat alone.
        use crate::personaverbs::tests_plane::{OPS_APPROVED, Plane};
        let plane = Plane::daily_with_ops();

        // Nobody approved the two that take a credential: only the plain one is started.
        let (config, _) = armed_as(&plane, "ops");
        let servers = config["mcpServers"].as_object().expect("servers");
        let mut names: Vec<&str> = servers.keys().map(String::as_str).collect();
        names.sort_unstable();
        assert_eq!(names, ["purlis", "status"]);
        assert_eq!(
            servers["purlis"],
            serde_json::json!({"type": "stdio", "command": "/bin/charter", "args": ["mcp"]})
        );

        // Approved, as it was for the sub-agent: each is run through `secret exec` of the
        // persona's vault by purlis's own binary, and no credential is in the config.
        crate::personaverbs::mcp::approve(
            plane.root(),
            &plane.state(),
            "ops",
            &OPS_APPROVED.map(String::from),
        )
        .expect("recorded");
        let (config, _) = armed_as(&plane, "ops");
        let servers = config["mcpServers"].as_object().expect("servers");
        assert_eq!(servers.len(), 4, "{servers:?}");
        assert_eq!(servers["grafana"]["command"], "/bin/charter");
        assert_eq!(
            servers["grafana"]["args"].as_array().expect("args")[..3],
            ["secret", "exec", "ops"].map(serde_json::Value::from)
        );
        assert_eq!(servers["gsc"]["args"][3], "--file");
        let text = config.to_string();
        assert!(
            !text.contains("\"secrets\"") && !text.contains("secret_files"),
            "{text}"
        );

        // Another persona's chat in the same project gets none of them.
        let (config, _) = armed_as(&plane, "steward");
        assert_eq!(config["mcpServers"].as_object().expect("servers").len(), 1);
    }

    #[test]
    fn a_persona_chat_is_denied_the_tools_its_persona_disallows() {
        // #1451, D-1451-18: `disallowed-tools:` was the sub-agent's deny-list. It is the
        // chat's now, as deny rules of its own settings, which outrank every allow.
        use crate::personaverbs::tests_plane::Plane;
        let plane = Plane::fixture("minimal");
        plane.write(
            "personas/reviewer/persona.md",
            "---\nname: reviewer\ndisallowed-tools: Write, Edit, Bash(git push:*)\n---\n",
        );

        let (_, settings) = armed_as(&plane, "reviewer");
        assert_eq!(
            settings["permissions"]["deny"],
            serde_json::json!(["Write", "Edit", "Bash(git push:*)"])
        );
        // The allows purlis pre-approves are still there, and a persona with no deny-list
        // adds no deny rule.
        assert!(settings["permissions"]["allow"].as_array().is_some());
        let (_, settings) = armed_as(&plane, "steward");
        assert_eq!(settings["permissions"].get("deny"), None);
    }

    #[test]
    fn a_browser_tool_a_persona_disallows_is_denied_beside_the_browser_tools_allow() {
        // #2245: the browser tools are allowed on the chat's browser Ask, and a persona's
        // `disallowed-tools:` still takes one away: its deny outranks the allow beside it.
        use crate::personaverbs::tests_plane::Plane;
        let plane = Plane::fixture("minimal");
        plane.write(
            "personas/reviewer/persona.md",
            "---\nname: reviewer\ndisallowed-tools: mcp__purlis__browser_click\n---\n",
        );

        let (_, settings) = armed_as(&plane, "reviewer");
        assert_eq!(
            settings["permissions"]["deny"],
            serde_json::json!(["mcp__purlis__browser_click"])
        );
        let allow = settings["permissions"]["allow"].as_array().expect("allow");
        assert!(allow.contains(&serde_json::json!("mcp__purlis__browser_click")));
        assert!(allow.contains(&serde_json::json!("mcp__purlis__browser_open")));
    }

    /// Whether a rule of `allow` lets `command` run in Claude Code without asking: a `Bash(…)`
    /// rule by its glob, as the dispatch guard's tests match one.
    fn bash_allowed(allow: &[serde_json::Value], command: &str) -> bool {
        allow
            .iter()
            .filter_map(serde_json::Value::as_str)
            .any(|rule| {
                rule.strip_prefix("Bash(")
                    .and_then(|rule| rule.strip_suffix(')'))
                    .is_some_and(|glob| crate::pypath::fnmatch(command, glob))
            })
    }

    #[test]
    fn a_task_chat_runs_the_read_only_purlis_commands_its_brief_runs_without_asking() {
        // Reported 2026-10-09: a task whose brief ran `purlis persona where` and `purlis
        // dispatch list` stopped on the harness's permission prompt in a tab nobody was at,
        // and waited there on the person. A task chat is armed as any chat running as its
        // persona is, so this is the settings a task gets.
        use crate::personaverbs::tests_plane::Plane;
        let plane = Plane::fixture("minimal");
        let (_, task) = armed_as(&plane, "steward");
        let allow = task["permissions"]["allow"].as_array().expect("allow");

        for command in [
            "purlis persona where",
            "purlis dispatch list",
            "purlis dispatch note \"half way\"",
            "purlis dispatch ask \"which cluster?\"",
            "purlis dispatch report --outcome done \"all green\"",
        ] {
            assert!(bash_allowed(allow, command), "{command:?} asks: {allow:?}");
        }
        for tool in [
            "persona_where",
            "dispatch_list",
            "dispatch_read",
            "dispatch_report",
        ] {
            let rule = serde_json::json!(format!("mcp__purlis__{tool}"));
            assert!(allow.contains(&rule), "{rule} asks");
        }

        // Never more than any other chat the app starts, the one that asked included: a task
        // is armed with the same allows, by the same code.
        let empty = tempfile::tempdir().expect("a directory");
        let StateHooks::ThisSessionOnly { args, .. } = adapter().arm(
            Kit {
                skills: None,
                laid_out: None,
                binary: std::path::Path::new("/bin/charter"),
                plugin: Some(std::path::Path::new("/app/plugin")),
                persona: None,
            },
            Some(empty.path()),
            &crate::harness_plugin::Chosen::new(),
            None,
        ) else {
            panic!("armed per session");
        };
        let at = args
            .iter()
            .position(|arg| arg == "--settings")
            .expect("--settings");
        let asker: serde_json::Value = serde_json::from_str(&args[at + 1]).expect("JSON");
        assert_eq!(asker["permissions"]["allow"], task["permissions"]["allow"]);
    }

    #[test]
    fn a_read_allowed_by_name_reaches_nothing_beside_it() {
        // `purlis persona where` takes no argument a chat would pass, so its rule is the whole
        // line: anything after it, or another word of `persona`, still asks.
        let allow: Vec<serde_json::Value> = READ_ALLOW
            .iter()
            .map(|rule| serde_json::json!(rule))
            .collect();
        assert!(bash_allowed(&allow, "purlis persona where"));
        for asks in [
            "purlis persona where --now 2026-10-07T14:00:00",
            "purlis persona where; touch x",
            "purlis persona remember x",
            "purlis persona",
        ] {
            assert!(!bash_allowed(&allow, asks), "{asks:?}");
        }
    }

    #[test]
    fn a_brokered_commit_is_allowed_by_its_two_words_and_no_other_worktree_word_is() {
        // #1476: the commit asks no approval, and the harness asking beside it stopped a task
        // in a tab nobody was at. Every other `worktree` word still asks.
        let allow: Vec<serde_json::Value> = WORKTREE_COMMIT_ALLOW
            .iter()
            .map(|rule| serde_json::json!(rule))
            .collect();
        assert!(bash_allowed(&allow, "purlis worktree commit -m one --all"));
        assert!(bash_allowed(
            &allow,
            "purlis worktree commit -m one src/a.rs"
        ));
        for asks in [
            "purlis worktree add thing piece",
            "purlis worktree remove piece",
            "purlis worktree",
            "purlis commit -m one",
        ] {
            assert!(!bash_allowed(&allow, asks), "{asks:?}");
        }
    }

    /// What a Claude Code chat in a project laid out with `manifest` and, when there is one, a
    /// `.claude/settings.json` holding `project`, starts with: its arguments, and its
    /// `--settings` parsed. Sandboxed as a start decides, at the level the manifest gives.
    fn started_in(manifest: &str, project: Option<&str>) -> (Vec<String>, serde_json::Value) {
        let dir = tempfile::tempdir().expect("a project");
        let root = dir.path().canonicalize().expect("a real folder");
        let off = manifest == OFF;
        let manifest = if off { FLOOR } else { manifest };
        std::fs::write(root.join("charter.toml"), manifest).expect("charter.toml");
        if let Some(project) = project {
            std::fs::create_dir_all(root.join(".claude")).expect(".claude");
            std::fs::write(root.join(".claude/settings.json"), project).expect("settings");
        }
        let machine = crate::sandbox::Machine {
            env: crate::secrets::Env::of(&[("PATH", "/opt/none")]),
            home: None,
            os: crate::sandbox::Os::MacOs,
        };
        let applied = match crate::sandbox::decide(
            Harness::ClaudeCode,
            &root,
            &machine,
            &|_| true,
            None,
            None,
        ) {
            Ok(Some(crate::sandbox::Decided::Sandboxed(applied))) if !off => Some(applied),
            _ => None,
        };
        let StateHooks::ThisSessionOnly { args, .. } = adapter().arm(
            Kit {
                skills: None,
                laid_out: None,
                binary: std::path::Path::new("/bin/charter"),
                plugin: Some(std::path::Path::new("/app/plugin")),
                persona: None,
            },
            Some(&root),
            &crate::harness_plugin::Chosen::new(),
            applied.as_ref(),
        ) else {
            panic!("armed per session");
        };
        let at = args
            .iter()
            .position(|arg| arg == "--settings")
            .expect("--settings");
        let settings = serde_json::from_str(&args[at + 1]).expect("JSON");
        (args, settings)
    }

    const ON: &str = "[sandbox]\nmode = \"on\"\negress = [\"model-providers\"]\n";
    const FLOOR: &str = "schema = 1\n";
    /// A chat started with no sandbox, as an `off` starts it: armed with none.
    const OFF: &str = "off";

    /// A project file holding keys beyond what a sandboxed chat keeps, beside rules it does.
    const BEYOND: &str = r#"{
        "permissions": {
            "allow": ["Bash(make *)"],
            "deny": ["Bash(rm -rf *)"],
            "ask": ["Bash(git push *)"]
        },
        "hooks": {"Stop": [{"hooks": [{"type": "command", "command": "true"}]}]},
        "sandbox": {"enabled": false}
    }"#;

    fn limited_to_the_person_s_settings(args: &[String]) -> bool {
        args.windows(2)
            .any(|pair| pair[0] == "--setting-sources" && pair[1] == "user")
    }

    #[test]
    fn a_sandboxed_chat_loads_no_project_settings_beyond_what_it_keeps() {
        // ADR 0067 as amended 2026-10-15, at the full sandbox and at the floor.
        for manifest in [ON, FLOOR] {
            let (args, _) = started_in(manifest, Some(BEYOND));
            assert!(
                limited_to_the_person_s_settings(&args),
                "{manifest}: {args:?}"
            );
            // Before `--mcp-config`, which takes every word up to the next flag.
            let sources = args.iter().position(|a| a == "--setting-sources");
            let mcp = args.iter().position(|a| a == "--mcp-config");
            assert!(sources < mcp, "{args:?}");
        }
        // A chat at `off` is not sandboxed, and loads what it always did.
        let (args, _) = started_in(OFF, Some(BEYOND));
        assert!(
            !args.iter().any(|arg| arg == "--setting-sources"),
            "{args:?}"
        );
    }

    #[test]
    fn a_sandboxed_chat_in_a_project_holding_only_what_it_keeps_loads_the_project() {
        for manifest in [ON, FLOOR, OFF] {
            for project in [
                None,
                Some(r#"{"permissions": {"deny": ["Bash(rm -rf *)"]}, "model": "sonnet"}"#),
            ] {
                let (args, _) = started_in(manifest, project);
                assert!(
                    !args.iter().any(|arg| arg == "--setting-sources"),
                    "{manifest} {project:?}: {args:?}"
                );
            }
        }
    }

    #[test]
    fn the_project_s_deny_and_ask_rules_are_carried_and_nothing_else_of_it() {
        for manifest in [ON, FLOOR] {
            let (_, settings) = started_in(manifest, Some(BEYOND));
            let rules = |bucket: &str| -> Vec<String> {
                settings["permissions"][bucket]
                    .as_array()
                    .map(|rules| {
                        rules
                            .iter()
                            .filter_map(|rule| rule.as_str().map(str::to_owned))
                            .collect()
                    })
                    .unwrap_or_default()
            };
            assert!(rules("deny").contains(&"Bash(rm -rf *)".to_owned()));
            assert!(rules("ask").contains(&"Bash(git push *)".to_owned()));
            assert!(!rules("allow").contains(&"Bash(make *)".to_owned()));
            // purlis's own hook only, and its own sandbox as compiled.
            let hooks = settings["hooks"].as_object().expect("hooks");
            assert_eq!(hooks.keys().collect::<Vec<_>>(), ["PermissionRequest"]);
            assert_eq!(settings["sandbox"]["enabled"], true);
        }
    }

    #[test]
    fn nothing_turns_a_claude_code_chat_s_hooks_off_behind_charter() {
        let command = ["claude".to_owned(), "--bare".to_owned()];
        assert_eq!(adapter().disarmed_by(&command, &[]), None);
    }

    #[test]
    fn doctor_says_a_claude_code_chat_is_armed_with_the_bundled_plugin() {
        assert_eq!(
            adapter().armed_with(),
            "the app arms each chat with its own plugin, purlis@inline"
        );
    }

    #[test]
    fn the_claude_code_adapter_hands_a_chat_claude_code_s_own_plugins() {
        assert_eq!(adapter().plugins().harness(), "claude");
    }

    #[test]
    fn a_chat_outside_any_project_reads_only_its_own_folders_settings() {
        // D-RN8-12's walk stops at the nearer of the project root and the first `.git`, and a
        // folder in neither never climbs: not to `/`, not into the home directory.
        let dir = tempfile::tempdir().expect("a directory");
        let chat = dir.path().join("loose/chat");
        std::fs::create_dir_all(&chat).expect("the chat's folder");
        assert_eq!(
            project_settings_files(Some(&chat)),
            [
                chat.join(".claude/settings.json"),
                chat.join(".claude/settings.local.json")
            ]
        );
        assert!(project_settings_files(None).is_empty());
    }

    #[test]
    fn the_walk_stops_at_the_nearer_of_the_git_root_and_the_project_root() {
        let dir = tempfile::tempdir().expect("a directory");
        let project = dir.path().join("project");
        let repo = project.join("workspaces/alpha/repo");
        let chat = repo.join("src");
        std::fs::create_dir_all(&chat).expect("folders");
        std::fs::create_dir_all(project.join(".git")).expect("the project's git");
        std::fs::write(project.join("charter.toml"), "schema = 1\n").expect("manifest");
        std::fs::create_dir_all(repo.join(".git")).expect("the clone's git");
        let read = project_settings_files(Some(&chat));
        assert_eq!(read.last(), Some(&repo.join(".claude/settings.local.json")));
        assert!(!read.iter().any(|p| p.starts_with(project.join(".claude"))));

        let layer = project.join("workspaces/alpha");
        let read = project_settings_files(Some(&layer));
        assert_eq!(
            read.last(),
            Some(&project.join(".claude/settings.local.json"))
        );
        assert_eq!(read.len(), 6, "{read:?}");
    }
}
