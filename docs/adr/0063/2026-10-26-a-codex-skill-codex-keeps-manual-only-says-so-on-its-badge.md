# A Codex skill Codex keeps manual-only says so on its badge

**2026-10-26**, ticket #3050 (SK-39), amending
[ADR 0063 as amended 2026-10-22](2026-10-22-a-skill-is-handed-only-what-narrows-a-chat-and-each-harness-its-translation.md),
under the operator's spec #2762. It builds the first item of that record's "Not built yet".

## The badge reads Codex's sidecar for a skill Codex finds itself

Codex 0.163.0 keeps a skill it finds itself, the person's own or a repo's, out of its model's
catalogue when `agents/openai.yaml` beside its `SKILL.md` sets
`policy.allow_implicit_invocation: false` (`ext/skills/src/loader/metadata.rs` at
`rust-v0.163.0`; measured: such a skill is left out of the catalogue and still runs when named).
Its frontmatter's `disable-model-invocation` does nothing there.

- `HarnessAdapter::manual_beside` names the file and how a text of it says manual-only. Only the
  Codex adapter has one; the default is none. The adapter stays pure: the host reads the file,
  through the skill's folder held from its root, never through a link, and at most
  `MOST_BYTES` of it.
- The file is read only for a skill from outside the plane (`Origin::handed` false), on the
  harness whose adapter names it. A skill purlis hands Codex is briefed, which Codex does not
  discover, so its sidecar is never read, as the 2026-10-22 record says.
- **It fails closed.** The file is read by purlis-skills' one strict YAML reader
  (`handed::flag_at`, the frontmatter's reader): an anchor, an alias, a tag, a key set twice in
  any mapping, more than one document, or anything but a plain `false` at that key reads as
  not manual-only. Codex reads the whole file with serde and ignores all of it where it does not
  parse, so the badge claims manual-only only where Codex surely reads it so. A key is compared
  by its text, however it is quoted, so `policy` and `"policy"` in one mapping are a key set
  twice.
- **What remains, one way:** a file purlis reads that Codex refuses for a value of the wrong type
  under another key (`interface`, `dependencies`, or `policy.products`, whose values are a list
  of Codex's own product names) still badges as manual-only, while Codex lists the skill.
  Nothing widens a chat by it.
- **And the other way, on purpose:** a sidecar Codex reads but purlis refuses, such as a link
  (which purlis never follows) or a capitalised `False` (serde reads it, the strict
  reader does not), badges as automatic while Codex keeps the skill manual-only. The badge then
  says less than Codex does, never more.
- The badge says **"Runs only when asked for by name."** wherever a harness keeps a skill out of
  its model's choice itself: by Codex's sidecar, and by `disable-model-invocation` on Claude Code,
  which reads that key. A skill purlis hands by its translation keeps the line it had ("Carried
  over by purlis's equivalent"). The frontmatter key Codex does not read is no longer listed as
  ignored once the sidecar keeps the skill manual-only.

## The budget is unchanged

A chat's skill budget counts the chat's set, the skills purlis lists to it
([ADR 0102 as amended 2026-10-21](../0102/2026-10-21-the-skill-budget-and-use-skill-as-built.md)).
A skill Codex finds itself is in Codex's own catalogue, never in that set (`for_use_skill` asks
for the set with no harness or repo skills), so it never cost anything there, manual-only or
not. Nothing changes in the budget.
