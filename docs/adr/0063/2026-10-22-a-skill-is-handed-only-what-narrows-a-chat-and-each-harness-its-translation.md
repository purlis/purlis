# A skill is handed only what narrows a chat, and each harness its translation

**2026-10-22**, ticket #2771 (SK-8), amending
[the routes carry the plane's skills](2026-10-20-the-routes-carry-the-plane-s-skills.md) and
§2 of [without a skill format, a harness finds and loads skills over MCP](2026-10-20-without-a-skill-format-a-harness-finds-and-loads-skills-over-mcp.md),
under the operator's spec #2762 (decisions 18 and 58) and the delegated decisions recorded on
#2771.

## A set's folder holds each skill's own folder, and purlis writes its `SKILL.md`

The 2026-10-20 record hands each plane skill by a link to its folder, never a copy. That no
longer holds: a link hands the harness whatever the folder holds, links and nested `SKILL.md`
files included, and the skill's own frontmatter as written. **Each plane skill is now a folder of
its own in the set's folder** (`skills::handout`): the `SKILL.md` purlis writes for it
(`purlis_skills::handed_text`), and a link to each plain file of the skill's folder, in folders
of their own as the skill has them. Every folder, file and link is made relative to the handle of
the folder it is made in. The set's digest covers each text and entry, so an edit makes a new
folder and a running chat's never changes.

**purlis parses, it does not splice.** It reads the frontmatter, between a first line that is
`---` and the next line that is `---`, with a YAML parser (`yaml-rust2`). It then writes a new
frontmatter of the keys it keeps, each value written out again, and the skill's body after it.
So a `---` inside a value, a quoted or indented key, a flow mapping or an explicit `? key` can
never hand a harness a key purlis did not choose to hand.

**It fails closed.** A skill is handed to no chat when its frontmatter does not parse, is not one
mapping of plain names, reuses part of itself (an anchor, an alias, a merge key), carries a tag,
sets a name twice, or spells a widening key another way (`allowed_tools`, `allowedTools`). The
same holds when a kept key has a value it cannot take, or the skill's folder or `SKILL.md` is a
link. The chat's tab says which and why (ADR 0099 as amended 2026-10-22).

**Nothing is handed through a link.** An entry of the skill's folder that is a link is never
linked to, so every link handed resolves to a plain file inside the skill's own folder. **Only
the top `SKILL.md` is the skill's**: a nested one is left out, so opencode, which scans
`skills/**/SKILL.md`, never reads one as a skill of its own.

What purlis leaves out of the `SKILL.md` it writes:

- **`allowed-tools` and `hooks`, on every harness.** On Claude Code, `allowed-tools` pre-approves
  tools while the skill runs, even in an untrusted `-p` run, and `hooks` runs commands on the
  chat's events. Both widen a chat, and a skill may only narrow one (decision 58).
- **Every key purlis knows no harness to read.**
- **`agent`, unless the skill forks (`context: fork`) as one of the harness's own agents**
  (`Honours::agents`). A repo's or the person's subagent can carry its own `permissionMode` and
  `hooks`, so forking as one would widen the chat.
- **The description of a manual-only skill, where that is how the harness keeps one out.**
  opencode 1.18.33 has no manual-only key, but lists only a skill with a description and still
  runs one by `/name` (measured, research of 2026-10-20).

`disallowed-tools` and `context: fork` pass through: they narrow. Where a harness does not apply
them, the skill's badge says "Narrowing not applied".

**Every route hands the same text.** A Codex chat's briefing names each plane skill by the
`SKILL.md` purlis wrote for it in the set's folder (`PURLIS_SKILLS_PLANE` names that folder), so
the briefing route takes no edit in by itself any more (`Harness::skills_reload`). `skill_load`
hands the same text, as the chat's harness would read it.

## Each harness says what it reads

`HarnessAdapter::skill_frontmatter` declares the keys a harness applies to a skill purlis hands
it, and how it keeps a manual-only skill out of its model's choice (`ManualOnly`):

| Harness | Reads | Manual-only |
|---|---|---|
| Claude Code | every key of its frontmatter reference | `Read`: `disable-model-invocation` itself |
| Codex | nothing but what purlis reads (a briefed chat reads `SKILL.md` as text) | `Listing`: the briefing names the skill and its file alone, to read only when the person names it. Codex's own `agents/openai.yaml` sidecar is never read for a briefed skill, which Codex does not discover |
| opencode | nothing but `name` and `description` | `WithoutDescription`: handed without its description |

The name, the description and the arguments are purlis's own on every harness: it lists the
first two, and Use skill… asks for the arguments (decision 61).

## The compatibility badge

Every skill a chat gets from purlis has a badge on each harness, and a harness or repo skill on
its own harness, the ones a persona names included (`purlis_skills::compat`). The badge says one
of four things:

- **Full.**
- **Partial**, with what is left out, carried over by purlis's equivalent, not applied, or read by
  nothing.
- **Handed to no chat**, for a skill purlis refuses.
- **Widens the chat**, for a harness or repo skill whose harness applies `allowed-tools` or
  `hooks`, or that purlis cannot read. purlis cannot rewrite those, so the studio marks them
  and shows what the built-in lints say of them.

`license`, `compatibility` and `metadata` are for a reader and count for nothing. Only the editor
names a key; the badge, the Notice and a row's lint never do.

## What this does not close

- **A linked file can be swapped after lay-out.** Each link handed resolves to a plain file of the
  skill's own folder when the set's folder is made. Someone who can write the skill's folder
  later can put a link to elsewhere in that file's place, and the harness follows it. The skill's
  source is author-controlled: whoever can write it can already change what its `SKILL.md` and
  scripts say. Copying the files instead would close the swap, at the cost of edits to a running
  chat's scripts and of disk for large skills; that is left for when a skill source is not the
  project's own (SK-10).
- **An agent outside the harness's own list is left out**, so a skill that forks as a project's or
  person's own agent runs in the chat instead; its badge says so.

## Not built yet

- Codex's own skills marked manual-only by their `agents/openai.yaml` sidecar, which the badge of
  a Codex skill does not read yet.
- The install review's lint and badge, which come with skill sources (SK-10).
