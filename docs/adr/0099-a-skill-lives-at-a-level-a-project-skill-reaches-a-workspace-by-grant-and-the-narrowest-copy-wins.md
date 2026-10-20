# A skill lives at a level, a project skill reaches a workspace by grant, and the narrowest copy wins

**Accepted 2026-10-20**, from the operator's spec #2762 (M65, Skill studio), whose decisions were
grilled with the operator on 2026-10-20; ticket #2765 (SK-2). It widens
[ADR 0063](0063-every-harness-a-chat-runs-on-is-handed-charters-skills-by-its-own-route.md): its
three routes now carry the plane's skills as well as the built-in ones, which its amendment of
2026-10-20 in [`0063/`](0063/) records.

Amended 2026-10-20 by #2767, which settled the turn-off list and the git rule for local skills: [`0099/`](0099/).

**Built by #2765** (project skills at the plane root, on every harness, and a read-only Skill
studio), then #2767 (grants, workspace and local skills), #2768 (persona skills), #2769 (harness
and repo skills) and #2782 (lifecycle states).

## Where purlis was

purlis managed one kind of skill, its own: the built-in skills in its plugin's `skills/`, handed
to each harness by ADR 0063's route. Every other skill a chat had came from somewhere purlis
could not see: the person's harness folders, harness plugins, a repo's committed
`.claude/skills`, or the plane's `.claude/skills`, which is copied into every clone as it is and
which only Claude Code reads. A project could not say where a skill applies, a workspace could
not have one of its own, and a persona's `skills:` list was only compared with use.

## The decision

### 1. A skill is an open SKILL.md folder, at a level

A skill is a folder in the open SKILL.md format, unchanged: a `SKILL.md` whose frontmatter names
a `name` and a `description`, and whatever scripts, references and assets beside it. purlis adds
nothing inside `SKILL.md`. What purlis alone knows about a skill (its grant now; its source and
pin, owners, lifecycle state and pinned flag as their tickets land) is kept **beside** the
folder, in `<name>.toml` next to `<name>/`. The one exception is `evals/evals.json` inside the
folder, an existing open convention (#2779).

The levels are plane folders:

| Level | Folder | Who gets it |
|---|---|---|
| Built-in | `skills/` in purlis's bundled plugin | every chat; it cannot be turned off or edited |
| Project | `skills/<name>/` at the plane root | chats its grant reaches (§2) |
| Workspace | `workspaces/<ws>/skills/<name>/` | that workspace's chats |
| Persona | `personas/<p>/skills/<name>/` | that persona's chats, in any workspace |

Each plane level can also be **local**: kept on this machine and never committed, so the team
never gets it, and shared by moving it to the level's `skills/`. Where a local skill lives and
how purlis tells it is not committed are settled by #2767, under the same live/local rule as
`charter.local.toml` (ADR 0048).

**Harness skills** (the person's harness folders and harness plugins) and **repo skills** (what a
clone commits, such as its `.claude/skills`) are listed and can be turned off where the harness
allows, and purlis never writes them. **purlis never writes into a harness's own skills
folders**: every route below hands a chat its skills for that chat alone.

### 2. A project skill reaches a workspace only by its grant

A project skill's `grant`, in `skills/<name>.toml`, is one of:

- `"root"`, the default, also what a missing file says: chats at the project's root only (the
  plane root; the spec calls it the master workspace, a word CONTEXT.md avoids). Nothing is
  shared until the project shares it.
- `"all"`: every workspace's chats too.
- a list of workspace names: those workspaces' chats too.

A workspace may turn off a project skill granted to it, and can never turn on one it was not
granted; where it records that is settled by #2767. A persona's skills and
the skills a persona names reach its chats whatever the workspace was granted (#2768): a persona
brings its tools with it when it is dispatched.

### 3. The narrowest copy wins a same-name tie, and purlis decides it

When two levels hold a skill of the same `name`, a chat gets one copy: **persona over workspace
over project**, and **local over shared** at the same level. The copy left out says which copy
hid it.

**purlis resolves the tie before any harness sees it**, and hands each harness exactly one copy
of a name. The harnesses disagree on their own (research of 2026-10-20 in the smart-ide
workspace, `refs/skills-research/harness-skill-mechanics.md`): Claude Code prefers a personal
skill over a project one, Codex keeps both copies, and opencode picked either of two same-named
skills at random over eight identical runs. A rule only holds where one copy reaches the harness.

**A built-in skill's name is never taken**, an addition this record makes (the spec says the
narrower copy wins and is silent on built-in skills; the call was delegated to SK-2). A plane
skill named as a built-in one is left out, "a built-in skill has this name": a curation action
that names `safe-remove` must get purlis's, in every project.
**A harness or repo skill's tie is the harness's** where purlis cannot keep the harness from
reading it: both load, and the studio marks the clash and offers to rename the plane copy
(#2769).

### 4. Every skill a chat does not get says why, in one line

The chat's skill set (`purlis-skills`' `skill_set`, the one place skills are worked out) returns
every skill it gets with its route and its level, and every skill it does not get with one
reason: not a skill (and what its `SKILL.md` lacks), not granted to this workspace, turned off
in this workspace, hidden by a narrower copy, a built-in skill has this name, archived, hidden by
the router, missing on this machine, or not supported by this harness. No state is left without
a sentence.

### 5. A running chat keeps the skills it started with, except where its harness reloads

How each harness takes a change to its skills during a chat, on the versions installed on
2026-10-20, for the routes ADR 0063 and its amendment use. Claude Code and opencode were measured;
Codex's row is inferred from source and the route, and says why:

| Harness | Through purlis's route | How it is known |
|---|---|---|
| Claude Code 2.1.304 | **No reload.** A skill added to, or edited in, a `--plugin-dir` plugin during the chat is not seen: a new skill is unknown, and an edited one answers with the body it had at the start. Typing `/reload-plugins` into the chat reloads both. | Twice, independently: a `stream-json` chat with a symlinked skill edited between turns kept its old body and description; the research run saw `Unknown skill` for a new one and the old body for an edited one, and both after `/reload-plugins`. |
| Codex 0.163.0 | **The list, no; the files, yes.** The briefing that lists the skills runs once, at `SessionStart`, so a skill added during the chat is not listed and a changed description is not shown. The model reads a skill's `SKILL.md` with its own tools when the skill applies, so an edit to a listed skill's body is read. | **Inferred from source, not measured.** The briefing is a `SessionStart` hook (ADR 0063) and its listing gives each file's path, not its text. Codex's own skill watcher (`app-server/src/skills_watcher.rs`) watches only its own roots, which purlis does not write. A live run was tried on 2026-10-20 and could not start: the sandbox the measuring chat ran in refused Codex's in-process app-server, and the machine's Codex sign-in had been revoked. #2770 measures it before it offers Codex a restart. |
| opencode 1.18.33 | **No reload.** Neither a new skill nor an edited one reaches a running chat: discovery is cached per directory and the body is cached with it. | The research run: a skill created mid-chat was `not found`, and an edited one returned its old body, in one `opencode run`. opencode's own built-in `customize-opencode` skill says the same: config, skills included, "is loaded once when opencode starts and is not hot-reloaded". |

So a chat whose skills changed is told so and offered a restart on the same conversation
(#2770), on every harness. Claude Code's `/reload-plugins` is the one in-chat reload; whether
purlis types it for the person, rather than restarting, is #2770's to decide.

## What was rejected

- **Writing the plane's skills into each harness's folders** (`.claude/skills`, `.agents/skills`,
  `.codex/skills` of each clone). Those are the person's repositories; a copy is a second source
  that goes stale, and the harness would then decide the ties.
- **Keeping purlis's metadata in `SKILL.md`'s frontmatter.** claude.ai and the Skills API reject
  keys outside the open standard, and a skill exported or installed elsewhere would carry purlis's
  state with it.
- **A `[skills]` table in `charter.toml` for grants.** One file every grant edits makes every two
  pull requests that grant a skill conflict, and a moved skill would leave its grant behind.
- **Letting a narrower plane skill override a built-in one.** The curation actions and the
  briefing name built-in skills, and an override would change what they do in one project only.
- **Letting each harness break ties.** It gives three different answers for one plane.

## Consequences

- A project skill works in every harness from its first save, at the plane root, and reaches a
  workspace only once the project grants it.
- `skills/` at the plane root, `workspaces/<ws>/skills/` and `personas/<p>/skills/` become
  reserved folders (`docs/plane-format.md`); #2767 adds `skills` to the names an extension's
  workspace folder may not take.
- The mirror of the plane's `.claude/skills` into clones retires with the migration (#2784), which
  moves those skills to project skills granted `"all"`.
