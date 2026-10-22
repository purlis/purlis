# A skill is handed only what narrows a chat, and each harness its translation

**2026-10-22**, ticket #2771 (SK-8), amending
[the routes carry the plane's skills](2026-10-20-the-routes-carry-the-plane-s-skills.md) and
§2 of [without a skill format, a harness finds and loads skills over MCP](2026-10-20-without-a-skill-format-a-harness-finds-and-loads-skills-over-mcp.md),
under the operator's spec #2762 (decisions 18 and 58) and the delegated decisions recorded on
#2771.

## A set's folder holds a skill's own folder where its `SKILL.md` must differ

The 2026-10-20 record hands each plane skill by a link to its folder, never a copy. That still
holds for every skill whose `SKILL.md` reaches the harness as written. **A skill whose `SKILL.md`
must reach the harness otherwise is a folder of its own in the set's folder**: the `SKILL.md`
purlis hands over (`purlis_skills::handed_text`), and a link to each other entry of the skill's
folder, so its scripts and references are the plane's own files. The set's digest covers that
text and those entries, so an edit makes a new folder and a running chat's never changes. A
`SKILL.md` that is a link, or cannot be read, is linked as before.

What purlis leaves out of the `SKILL.md` it hands:

- **`allowed-tools` and `hooks`, on every harness.** On Claude Code, `allowed-tools` pre-approves
  tools while the skill runs, even in an untrusted `-p` run, and `hooks` runs commands on the
  chat's events. Both widen a chat, and a skill may only narrow one (decision 58). The other
  harnesses ignore both, so leaving them out changes nothing there.
- **The description of a manual-only skill, where that is how the harness keeps one out.**
  opencode 1.18.33 has no manual-only key, but lists only a skill with a description and still
  runs one by `/name` (measured, research of 2026-10-20).

`disallowed-tools` and `context: fork` pass through: they narrow. Where a harness does not apply
them, the skill's badge says "Narrowing not applied".

## Each harness says what it reads

`HarnessAdapter::skill_frontmatter` declares the keys a harness applies to a skill purlis hands
it, and how it keeps a manual-only skill out of its model's choice (`ManualOnly`):

| Harness | Reads | Manual-only |
|---|---|---|
| Claude Code | every key of its frontmatter reference | `Read`: `disable-model-invocation` itself |
| Codex | nothing but what purlis reads (a briefed chat reads `SKILL.md` as text) | `Listing`: the briefing names the skill and its file alone, to read only when the person names it |
| opencode | nothing but `name` and `description` | `WithoutDescription`: handed without its description |

The name, the description and the arguments are purlis's own on every harness: it lists the
first two, and Use skill… asks for the arguments (decision 61).

## The compatibility badge

Every skill a chat gets from purlis has a badge on each harness, and a harness or repo skill on
its own harness (`purlis_skills::compat`): **full**, **partial** with what is left out, carried
over by purlis's equivalent, not applied or read by nothing, or **widens the chat** for a harness
or repo skill whose harness applies `allowed-tools` or `hooks`, which purlis cannot rewrite.
`license`, `compatibility` and `metadata` are for a reader and count for nothing.

## Not built yet

- Codex's own skills marked manual-only by their `agents/openai.yaml` sidecar, which the badge of
  a Codex skill does not read yet.
- The install review's lint and badge, which come with skill sources (SK-10).
