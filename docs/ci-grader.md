# The CI grader

purlis's built-in skills are graded in CI by the **CI grader** (spec #5714;
[ADR 0105](adr/0105-a-grade-is-real-chats-purlis-runs-cost-first-scored-by-skill-runs-and-recorded-against-the-skills-version.md)
as amended 2026-11-08, "built-in skills are also graded headless in CI"). It runs
Claude Code, Codex and opencode in their print modes on a throwaway runner, reads which skills
each chat started from the harness's machine-readable output, and holds each built-in to its
**trigger bar**. It is not a Grade in the Skill studio: nothing of it is shown in the window or
written to an eval record.

This page is for whoever writes a built-in skill's tests. The scoring is
`purlis_skills::grade::ci`; it launches nothing, so it is tested offline on recorded output in
`crates/skills/src/grade/ci/recorded/`.

It reads what a harness printed only to score a test, never to move a run: no purlis run exists
in it (ADR 0076 holds), as the amendment says.

## A built-in skill's two files

Beside `SKILL.md`, under `evals/`:

- `evals/triggers.json`: the trigger tests, in Skill studio's own format, unchanged
  ([plane format](plane-format.md)).
- `evals/sources.json`: the skill's tier, its neighbours, and where each prompt came from.

```json
{
  "tier": "strict",
  "neighbours": ["purlis-browser"],
  "prompts": [
    {"query": "use the deploy token from the vault to push the image", "source": "mined, paraphrased"},
    {"query": "log in to the staging dashboard with the saved password", "source": "written",
     "belongs_to": "purlis-browser"}
  ]
}
```

| Key | Meaning |
|---|---|
| `tier` | `strict` (recall at least 95%, false triggers at most 5%) or `standard` (at least 85%, at most 5%). Required. |
| `neighbours` | the built-ins closest to this one, never itself. |
| `prompts` | one entry for each trigger test, by its exact `query`, with a `source` note that is not empty: mined from a transcript (paraphrased and scrubbed), a correction, a Report, or written. |
| `belongs_to` | on a prompt the skill should **not** start: the neighbour it belongs to. Leave it out when the prompt belongs to no built-in. |

A skill whose `sources.json` is missing, names no tier, or is malformed (not JSON, a key it does
not know, a prompt with no source or two, a source for no prompt, an empty note, a `belongs_to`
that is not a neighbour or sits on a prompt the skill should start) is **reported, not scored**,
with the sentence saying what is wrong.

## How it scores

- **A start** is a tool call, never text: the harness's own Skill tool (Claude Code's `Skill`,
  opencode's `skill`), a plain read of a briefed `SKILL.md` on Codex, or purlis's `skill_load`.
  A plugin's namespace is no part of the name, and a call that failed started nothing.
- **A run that does not read** (not the harness's JSON, a chat that never began, or one that
  failed before starting anything) is no run, never a "did not start".
- **Each prompt runs three times** and starts a skill where half the runs that read or more
  started it, as a Grade counts.
- **Whose a prompt is**: the skill's own when it should start it, else its `belongs_to`, else
  nobody's. A built-in that starts on a prompt that is not its own has a false trigger, whichever
  scored skill's tests the prompt is in. The prompt's own skill has a miss.
- **Recall** is the share of the skill's own prompts that should start it that did. **False
  triggers** is the share of its own prompts that should not start it that did: the share its bar
  holds, the same whether it is graded alone or beside every other built-in. Its starts on other
  built-ins' prompts are listed among its failing prompts and counted beside the share, and each
  is a miss for the built-in the prompt is of, whose recall its bar holds.
- **Crowded and bare** runs are scored apart. Only the crowded score holds a skill to its bar,
  and only where every one of its prompts ran among the crowd. The bare score is reported beside
  it, so a wording problem can be told from a crowding problem.

A verdict names the harness, its version and model, the bar, both scores and each failing
prompt with what started instead:

```text
purlis-secrets on Claude Code 2.1.319 (claude-sonnet-4-5-20250929): below its strict bar, recall at least 95% and false triggers at most 5%.
Crowded: recall 50% (1 of 2), false triggers 0% (0 of 2).
Bare: recall 100% (2 of 2), false triggers 0% (0 of 2).
Failing prompts:
- crowded, missed: "use the deploy token from the vault to push the image" (started it in 1 of 3 runs; started purlis-browser, env-files instead)
```
