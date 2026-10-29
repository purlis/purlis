# Harnesses

A **harness** is the agent program a chat runs: Claude Code, Codex or opencode. The app starts
each, arms each chat it starts with purlis's hooks and its Bash guard for that session alone,
and installs nothing into any of them.

Chats are started from the app's window. On the command line, `purlis harness list` shows
what a chat can be started on:

```
  NAME          KIND      COMMAND                                  FROM
  claude        claude    claude                                   built-in
  opencode      opencode  opencode                                 built-in
  codex         codex     codex                                    built-in
  claude-ccs    claude    ccs work                                 charter.local.toml
* claude-work   claude    CLAUDE_CONFIG_DIR=~/.claude-work claude  charter.local.toml
  codex-pinned  codex     npx -y @openai/codex@0.140.0             charter.local.toml
```

- **NAME** — what the profile is called; `*` marks `[harness] default`.
- **KIND** — which harness program it launches.
- **COMMAND** — the environment it sets, then its command. A control character in either is
  shown escaped, never interpreted: the file is one a chat can write, and a carriage return
  could otherwise redraw the line to show a different command.
- **FROM** — `built-in`, or the file that declared it.

Then `refused:`, one line per profile purlis would not read and why, and a warning while
git would commit the file:

```
refused:
  claude-work: git would commit charter.local.toml, so purlis reads nothing in it until it is ignored — purlis doctor --fix local-ignore adds /charter.local.toml to .gitignore.
! to use the profiles in charter.local.toml: purlis reinit
```

A `purlis <profile>` command that starts a chat from a terminal is not in this version yet
(#1608).

`purlis profile list` is the same listing with a **SIGN-IN** column: the sign-in purlis can
see a profile has. A subscription is signed in through the harness's own flow and purlis never
looks at it, so a profile shows `unknown` unless purlis can tell: `local` (or another kind) for a
harness that supports that kind alone, `cloud` where the profile's `env` or `[chat_env] pass`
switches the harness to a cloud (`CLAUDE_CODE_USE_BEDROCK=1`), and `api-key` where they pass a
provider's key (`ANTHROPIC_API_KEY`):

```
  NAME         KIND      SIGN-IN  COMMAND                           FROM
  claude       claude    unknown  claude                            built-in
  opencode     opencode  unknown  opencode                          built-in
  codex        codex     unknown  codex                             built-in
  claude-aws   claude    cloud    CLAUDE_CODE_USE_BEDROCK=1 claude  charter.local.toml
```

## Two accounts of one harness, or one version pinned

A harness profile is a named way to launch one harness: its kind, its command, its
environment. Every harness purlis knows is a built-in profile named after itself. Two
Claude Code accounts, or a Codex pinned to an older release, are profiles you declare in
`charter.local.toml`, a file purlis keeps out of git. The shape, what is refused and why,
and how the file is kept uncommitted are in
[control-plane.md](control-plane.md#harness--profiles-and-the-default).

**A wrapper around a harness is a profile too.** If you reach Claude Code through something
else — `ccs work`, which picks the account a chat is billed to, or a script that runs a
binary kept off your `PATH` — declare the wrapper as the command:

```toml
[harness.claude-ccs]
kind = "claude"
command = ["ccs", "work"]
```

**A profile can name its icon** (#2922): `icon = "spark"` (or `braces`, `terminal`) draws
it with one of the harness glyphs, and `icon = "mine.svg"` with an SVG of your own in the
`icons` folder of purlis's directory in your config home (`~/.config/purlis/icons/mine.svg`),
which is never shipped or committed. purlis reads the SVG as shapes and nothing else, and
draws it in the text's colour, as every icon is drawn. A profile's icon appears only beside
its name, never where purlis asks you something. A
profile that names no icon shows the monogram of its kind (`C`, `OC`, `CX`); the built-ins
are drawn with the glyphs purlis draws for them, never a vendor's mark.

Pick `claude-ccs` in the app's new-chat picker and the chat runs `ccs work`. It is still the
Claude Code harness, because the kind says so and not the program's name: the chat carries
`$PURLIS_HARNESS`, purlis's hooks and a session id the app can resume it by. The program
purlis looks for before it starts anything is the wrapper — a missing `ccs` is refused and
named, and a `claude` your `PATH` cannot see is the wrapper's to find.

**The wrapper has to pass its arguments on**: the app arms a chat with words it puts after
the profile's whole command — `--plugin-dir <bundled plugin> --settings <json>` for Claude
Code, `-c hooks.<Event>=…` for Codex (see *Per profile — armed at launch*, below) — and its
own session words after those. A `ccs work` profile starts as
`ccs work --plugin-dir … --settings … --session-id <id> --name <name>`, so the wrapper reads
its own `work` first, and the rest has to reach `claude`. A wrapper that swallows them
starts a chat with none of purlis's hooks and none of its guard.

**There is no `purlis harness add`.** A profile is added by editing `charter.local.toml`. A
chat can run a command as easily as it can edit a file, so a command could never stand for
your approval of what a profile runs; it would only save typing.

**What does stand for your approval is the launch itself.** The first time you pick a
profile you declared — and again whenever its command or environment changes — the picker
shows you what it runs and starts it only once you press *Approve and start*. Built-ins never
ask. See [*A new or changed command asks once*](control-plane.md#a-new-or-changed-command-asks-once).

## What each harness lets purlis offer

**What differs is not what purlis enforces — it is what each harness lets purlis
offer:**

| | how purlis reaches a chat | how it updates | what it cannot carry |
| --- | --- | --- | --- |
| Claude Code | the app's own plugin, `charter`, loaded into each chat it starts with `--plugin-dir` — nothing installed | with the app | — |
| Codex | purlis's hooks, armed on each chat's command line as `-c hooks.<Event>=…` — nothing installed; Codex asks once to trust them | with the app | no status bar; no command-pattern permissions, so a `guard ask` rule does not ask in a Codex chat: purlis writes it nowhere Codex reads, and purlis's own hook does not ask for it either (it refuses only another spelling of a rule the project's Claude Code or opencode settings, or your own Claude Code settings, hold, and lets the spelling the rule names run); no per-workspace config — Codex reads a project `.codex/config.toml` only once the project is trusted, and purlis writes none (a project `.codex/skills/` **is** read — a skills surface, not config); no prompt in front of `purlis handoff` — purlis's hook still refuses a handoff from a sub-agent, and the app decides every other one under the dispatch grant; a word when it stops mid-turn for your approval (its `PermissionRequest`, which the window can answer) and none when it waits on anything else; a persona's `mcp.json` servers are not started for its chat, and a persona that declares `disallowed-tools:` is refused a chat here (`purlis docs show personas`) |

Claude Code's row is empty because nothing purlis offers is out of reach there, not
because purlis fills every surface it has: the app sets Claude Code's `statusLine` for a
chat only where the settings in force fill it with nothing, so a footer the operator wired
stays theirs. That is a choice, not a ceiling.

The plugin, the hooks it declares and the `purlis` they call ship inside the app's bundle,
and the app's updater moves all of them together
([updating.md](https://github.com/purlis/purlis/blob/main/docs/updating.md)).
Nothing is installed into Claude Code or Codex, so there is nothing there to keep in step,
and nothing is written into the repos you work in beyond purlis's generated layer (below).

### Resume: the session each chat is linked to

Every chat is linked to one harness session, and when the app reopens a chat it resumes that
conversation only where one exists. How each harness is linked was read on these versions:

| Harness | Link | Offered when | Resumes with | Limits |
| --- | --- | --- | --- | --- |
| Claude Code 2.1.272 | the id the app hands it (`--session-id <uuid> --name <name>`); follows `/clear`; ignores a nested `claude` | after the first prompt (the transcript exists) | `--resume <id> --name <name>` | the in-session `/resume` hides the current session; the name shows in a fresh `claude --resume` picker, and in `/resume` from every other session |
| Codex 0.147.0 | the first id it reports in each start | after its first turn (it reports nothing before) | `codex resume <id>` | after `/new`, resume offers the start's first conversation; read from source, not run |

A profile whose own command already names a session (`--resume`, say) gets none of purlis's
session words: two on one command line is not a harness anyone has measured, and the one you
wrote is the one you meant.

### The name a session is started under

**Claude Code is the only harness purlis names.** The app passes the chat's name with
`--name` at every start and every resume, and never types `/rename` into a running harness,
so a rename reaches Claude Code at that chat's next start or resume, and not
before. Codex names no flag for it (openai/codex#14482 is open), so a Codex chat carries its
name in the app's own window only, and `/rename` inside it is yours to type.

**Which harness sent a report is decided by the report, never by the environment it
inherited.** A harness nested in a chat's shell inherits `PURLIS_HARNESS` and `CLAUDE_PID`,
so neither can say. A Claude Code report proves itself by `CLAUDE_CODE_SESSION_ID` equal to
its payload's `session_id`, and then counts only from the `CLAUDE_PID` that adopted the chat's
id. **purlis recognises a Codex report by the exact key set of Codex's SessionStart input**,
read from source at codex-cli 0.147.0. A Codex that adds a field is recognised as no report.
The failure is closed: no link is recorded, so resume is simply not offered for that chat
until purlis is updated — a missing resume is the only sign.

## Wiring, and when it happens

**`purlis init` installs no software.** It writes the plane's own files — its settings, the
`ask` rule in front of `purlis handoff` — and nothing into a harness's config folder or
anywhere outside the plane. `purlis reinit` does the same. purlis's hooks and guard reach a
chat because the app started it: every chat is armed at launch, for that session alone
(*Per profile — armed at launch*, below), so there is no install to run, repair or keep up
to date.

**Plus one generated layer per workspace, and only for Claude Code.** Claude Code reads
project settings from the session's working directory and does not walk up, so a chat
launched in `workspaces/<ws>/` would otherwise get none of the plane's ask/deny rules, none
of its `enabledPlugins` and none of its `env`. purlis writes the plane's copy into
`workspaces/<ws>/.claude/settings.json`, records what it wrote in a `.purlis-generated`
sidecar, and never touches a file whose hash it cannot vouch for. purlis's own plugin does
not depend on this file: it arrives on the command line. `purlis workspace reinit` is the
repair.

`purlis doctor`'s **`session layer`** row answers the other half — *can a session started in
this directory see any of that?* Three artefacts, three discovery rules, all measured on
Claude Code 2.1.259: `.claude/settings.json` is read from the session's own directory with
no walk-up, `.claude/agents/` and `.claude/skills/` walk up but stop at the git root, and
`CLAUDE.md` walks up and is not git-bounded. So "purlis is set up here" was never one
fact, and the row names which part is missing and which rule decided.

Codex gets nothing here and says why: a workspace **directory** is not a config scope for it,
so two workspaces on one machine cannot be made to differ. It does read something from a
project — `.codex/skills/` — and, once the project is trusted, a project `.codex/config.toml`
(both measured on codex-cli 0.147.0, the second in #354), which purlis does not write. The app arms purlis's hooks on each Codex
chat's command line (`-c hooks.*`) instead, so no file in a directory carries them. purlis
writes nothing machine-global on the operator's behalf.

**A clone gets the same layer, plus what the walk-up could not carry there.**
`workspaces/<ws>/<repo>/` is a repo of its own, so a session inside it loses the settings
*and* the plane's `.claude/agents/` — the walk-up stops at that git root. purlis writes
both, and hides them in the clone's own `.git/info/exclude`: per-checkout, never
committed, not itself tracked, and the one file a guest may write. purlis never edits the
clone's `.gitignore`, hides only the exact paths it generated (never a `.claude/` glob,
which would take your own untracked files with it), never touches a file it did not write,
and removes its files and its exclude block when the workspace goes. `git status` in your
repo is unaffected, and nothing purlis wrote can be staged. Linked worktrees included —
their `info/exclude` is the main repo's, which is also why removal is not just a
`rm -rf`.

**And it is every harness's layer.** What a git boundary cuts off is spelled by each
harness in purlis's registry, so purlis's own code names none of it:

| harness | carried into a checkout | binary |
|---|---|---|
| Claude Code | `.claude/agents`, `.claude/skills` | 2.1.259 |
| Codex | `.codex/skills` | 0.147.0 |

What purlis mirrors is **capability** — agents, skills, commands — and three things are
deliberately absent:

- **A harness's config file.** Copying one would put an `allow` in force in a repository
  nobody granted it in. That is why Claude Code's settings arrive **generated** instead:
  `permissions.ask` and `permissions.deny` from the plane's shared file into a workspace's
  and a clone's `.claude/settings.json`, and from its local file into a clone's own
  `.claude/settings.local.json` only, never `permissions.allow`. Claude Code reads the local
  file at the git root (measured on 2.1.267), which a workspace directory shares with the
  plane and a clone does not — see [workspaces.md](workspaces.md) for how purlis keeps that
  file hidden once Claude Code writes its own approvals into it.
- **A project `.codex/config.toml`**. Codex reads it only once the project is trusted, and
  then it can carry hooks, sandbox and MCP settings (measured on codex-cli 0.147.0, #354) — so copying one would put config in force
  in a repository nobody granted it in, the same reason as above. The app arms purlis's hooks
  on each Codex chat's command line (`-c hooks.*`) instead.
- **`CLAUDE.md` or any equivalent**, because a guest hides its own files and does not
  narrate the host's.

### Per profile — armed at launch

A profile points a harness at another config folder, and **purlis's guard does not live in
that folder**, so moving it moves nothing of purlis's. The app arms every chat it starts on
the command line, for that session alone, and writes nothing into any config folder:

- **Claude Code** gets `--plugin-dir <the bundled plugin>`: the app's own plugin,
  `purlis`, with every hook purlis answers — the state hooks and the Bash guard — and
  the `handoff`, `working-in-a-clone`, `update`, `persona`, `secrets`, `browser`,
  `safe-remove`, `compact` and `add-curation-action` skills, which reach the model as
  `purlis:<skill>`, and the app's MCP server, `purlis` (tools `mcp__purlis__<tool>`).
  Beside it, `--settings` carries `enabledPlugins` with
  `purlis@inline` pinned on — a project file a chat can write could otherwise turn it
  off — and a plugin named `charter@charter` turned off, so a plane whose settings enable an
  older purlis plugin for your terminal sessions does not give an app chat two sets of hooks
  and two `handoff` skills. The Python charter's plugin is *named* `charter` too, and Claude
  Code loads one plugin per name: turned off, it cannot take the bundled one's place, and the
  bundled one pinned on cannot be swapped for it (measured on claude 2.1.282). Every id
  the plugin had before it was renamed `purlis` — `charter@inline`, `charter-app@inline` and
  the installed copy's `charter@charter-app` — is turned off as well: they are named
  `charter`, a different name from `purlis`, so nothing else keeps them from loading beside
  it, and a file that still names one is told the new one. `--settings` merges with the settings in force and wins where it
  names a key (measured on claude 2.1.276 and 2.1.280), and it is for that session only.
- **Codex** gets `-c hooks.<Event>=[…]` for the four state events Codex fires
  (`SessionStart`, `UserPromptSubmit`, `Stop`, `SessionEnd`) and the Bash guard on
  `PreToolUse`, each pointing at the app's `purlis` by its absolute path. Codex lists them as
  "Session flags" and runs them beside any hooks of your own in `$CODEX_HOME/config.toml`
  (measured on codex-cli 0.147.0). They are inert until trusted: the Codex TUI asks at
  startup, and records the answer in its own `[hooks.state]`, so one binary path is asked
  about once, not once a chat. Codex has no way to take a skills directory for one session,
  so the chat is started with `PURLIS_SKILLS_DIR` naming the bundle's `skills/`, and its
  `SessionStart` briefing lists purlis's skills with the path to each `SKILL.md` (ADR 0063).
- **opencode** gets purlis's shim, named in `OPENCODE_CONFIG_CONTENT` with a folder of
  purlis's skills as its option: the shim carries every hook, and adds that directory to the
  skills opencode discovers, beside the operator's own `skills.paths` (ADR 0058, ADR 0063).
  The folder is laid out for the chat in purlis's data folder, and holds the built-in skills
  under names of their own, `purlis-handoff`, `purlis-working-in-a-clone`, `purlis-secrets` and
  so on, as Claude Code knows them as `purlis:<skill>` (ADR 0099 as amended 2026-10-26).
  opencode reads every skill by its bare name, the ones a repo holds in `.agents/skills` and
  your own alike, so a skill of yours named `secrets` is another skill beside
  `purlis-secrets`. A chat types one as `/purlis-<skill>`.

**Every chat is also handed purlis's MCP server**, `purlis mcp`, the same way on each harness
and for that chat alone: Claude Code on `--mcp-config`, Codex on
`-c mcp_servers.charter={…}`, and opencode under `mcp` in the same `OPENCODE_CONFIG_CONTENT`.
Your own MCP servers stay beside it. Its tools act on the workspace the chat works in (or the
project root), which the server reads by the same rules a `purlis` command in the chat does,
and none takes a workspace, a project or a path:

| Tool | What it does |
| --- | --- |
| `todo_list`, `todo_add`, `todo_done` | The workspace's open todos; marking one done writes its trace to memory and keeps it as done, to reopen with `purlis ws todo reopen` (ADR 0106) |
| `memory_search`, `memory_add` | The workspace's memory |
| `persona_remember` | One memory of the chat's own persona, or of shared memory; in a chat purlis started, purlis writes it for the chat, and the tool never writes it itself there |
| `persona_where` | Where the chat is working, from purlis's own record: the chat that asked for it, that chat's other tasks, and the other chats running as its persona, by name, workspace and state; never another chat's content |
| `session_record_list`, `session_record_read` | The session records where the chat works, by file name |
| `workspace_vision`, `workspace_section` | For a write outside a smart close, which keeps its own way of writing `workspace.md`: the `## Vision` of the workspace's `workspace.md`, replaced, and one entry added to its `## Context & decisions` (`decisions`) or `## Glossary` (`glossary`), with what is there kept; in a chat purlis started, purlis writes it for the chat, and the tool never writes it itself there |
| `change_status` | The workspace's cross-repo changes, from their records; the forge is not asked |
| `dispatch`, `dispatch_list`, `dispatch_read`, `dispatch_report` | A task for a persona, which purlis starts as a chat of its own under this one (waiting for its report when asked to), the list of the tasks this chat dispatched, what one of them said last and the commands it ran, and the one report a task's chat sends back; purlis decides and starts the chat, and the tool never does |
| `ask_operator` | A question put to you through the harness's own prompt; the model never answers it |
| `browser_open`, `browser_navigate`, `browser_snapshot`, `browser_find`, `browser_click`, `browser_double_click`, `browser_hover`, `browser_type`, `browser_press`, `browser_select`, `browser_drag`, `browser_scroll`, `browser_back`, `browser_wait_for_text`, `browser_pages`, `browser_switch_page`, `browser_screenshot`, `browser_new`, `browser_close`, `browser_login`, `browser_downloads`, `browser_save_download`, `browser_upload` | A real browser the session host runs for the chat (#2013): a snapshot of a page with a reference for each element, actions on those references, and a screenshot, which comes back as an image. An action asked for its `changes` also answers the page's lines new or gone since it was last read, and a snapshot marks an element beyond what the page, or a box or frame it is in, shows `(out of view)` (#2137, #2564). No tool runs script. `browser_login` names one of the persona's declared browser logins, which purlis performs in the browser itself from the vault, so the login never reaches the chat (#2016). A download lands in the browser's holding folder, which no chat reads, and the chat is told its name, size and origin; `browser_save_download` writes it into the chat's folder only once you have said yes for its origin, and `browser_upload` takes files of the chat's own folder alone, following no link (#2020). The chat's first browser tool asks you what its browser may reach (this project's sites and its dev servers, also the open web, or Keep off), and nothing opens until you answer; purlis decides and starts the browser, and the tool never does. Your answer is the consent for the chat's browser tools, so a Claude Code chat runs them without Claude Code's own prompt; your own `ask` or `deny` for one still wins (#2245). `browser_upload` and `browser_save_download` are the exceptions: each moves a file across the chat's folder, so Claude Code asks you at every call, naming the files (#2020) |
| `theme_preview` | The theme specimen, a fixed page of sample content (every control, a chat pane, an editor with syntax, a terminal with ANSI colour, a Settings page, a dialog and a Notice), drawn by the purlis app in a hidden window of its own in the theme, appearance and option values asked for, alone or two side by side, and returned as a picture (#2330). A theme is named by its id, as `purlis theme list` names it: a built-in, a theme folder of the chat's own workspace, the project or your own, an extension's theme in force there, or `draft:<folder>`, the chat's own theme draft. It is drawn as the window draws it where the chat works, tinted with that workspace's colour and in the theme's own fonts (#2536). Purlis, a preview theme, and any theme with a high-contrast dark can be drawn in it too (#2709). It never shows your window, chats, paths or terminal output. Drawn only for a chat purlis started, while the app runs. A Claude Code chat runs it without asking (ADR 0064, amended 2026-10-17) |

A chat purlis sandboxes is not offered its harness's own browser tools: a Claude Code chat's
`--settings` deny its Chrome integration (`mcp__claude-in-chrome`), at either level. Codex and
opencode offer none of their own.

The server runs where each harness runs its MCP servers, which for Claude Code and Codex is
outside the sandbox its commands run in, so what it may do is the tool set above and nothing
more: no tool reads a vault, the environment, the audit or a forge. Every store a tool reads or
writes must sit inside `workspaces/<ws>/` with no link on the way and no link inside it,
symbolic or hard, or the call is refused and nothing is written (V74). Each store is opened
one component at a time without following a link and is then used only through that open
directory, so a link swapped in while a tool runs cannot carry a read or a write anywhere else.
On a platform where purlis cannot do that yet (Windows), the tools refuse. A tool that writes
re-reads `charter.toml` first, so a project that has become read-only to this purlis (FR-24)
is not written by a server that started before it did. `ask_operator` shows your harness
*This chat asks: …*, and its answer field says not to type a password or secret there. Under every harness the server
is handed the chat's connection to the app, the hook socket, chat number, token and harness its
hooks already carry (`PURLIS_HOOK_SOCKET`, `PURLIS_CHAT`, `PURLIS_CHAT_TOKEN`,
`PURLIS_HARNESS`), and the app checks each of its asks as it checks a line of the chat's (#1996).
Claude Code and opencode hand a server their whole environment. Codex hands a server only the
variables it is told to, so it is told those four and the ones that say where the chat works
(`PURLIS_ROOT`, `PURLIS_WORKSPACE`, `PURLIS_PLANE_ROOT_SESSION`, `PURLIS_PERSONA`,
`PURLIS_SESSION_ID`), by name only: Codex reads each value from its own environment. An ACP
agent is handed the same in `session/new`. A Codex call may take an hour, because
`ask_operator` waits on a person.

While `ask_operator` waits on you, the chat shows in needs you, waiting on a form, and leaves it
once you answer. Claude Code says so itself, as it does for any MCP server's form. For a harness
with no such notice the server says it on the chat's hook channel, only once the question has
really gone to the harness to show, and says its end on every way out: answered, declined,
dropped, or the session over. It says no word of the question, and the app takes it as it takes
any hook of the chat's, so a server cannot make a chat look as if it waits on you. Under Codex
the server says it; opencode (1.18.33) offers MCP servers no form to ask with, so
`ask_operator` says it cannot ask there.

Every chat also carries `$PURLIS_HARNESS` (the registry's name for its kind, whatever the
profile is called), `$PURLIS_HARNESS_PROFILE` and `$PURLIS_ROOT` in its environment, and a
`PATH` with the app's own `purlis` first unless the profile sets its own.

**What still stops a chat on a profile**, each said before anything opens:

- **An opencode profile that would load no plugin** — one passing `--pure`, or setting
  `OPENCODE_PURE` or `OPENCODE_CONFIG_CONTENT` (ADR 0058).
- **A profile purlis may not run a command for** — not approved yet, or declared in a
  `charter.local.toml` git would commit. Nothing is run and nothing is written.

**`purlis doctor` shows a row per profile and runs nothing to fill it.** A profile it may
ask about reads OK — *the app arms each chat with its own plugin, purlis@inline*, or for Codex
*the app arms each Codex chat with purlis's hooks; Codex asks once to trust them* — with the
program it found. What can still be wrong is whether the harness can be found at all, and
that is a warning naming the directories searched and the fix: an absolute command in
`charter.local.toml`. `purlis doctor --preflight`, which the SessionStart hook runs, shows no
profile row.

**The one place a chat can still be unarmed:** a build of the app that carries no plugin — a
development build without one — starts a Claude Code chat with nothing added, and the chat
reads `unknown` rather than being half-armed from somewhere else.

## `charter statusline`

Claude Code's footer command. Inside the app it prints an empty line — the window already
draws the plane — and still records the turn's token usage. Run anywhere else, it draws the
plane's identity row and says in its body which parts it does not draw yet: repos, personas
and the session (#997). `--watch` is refused with a reason (exit 1): there is no repaint yet, and
one frame would pass for a watch that stopped. `--interval` is still parsed, so the refusal is
purlis's and not a usage error.
