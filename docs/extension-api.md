# The extension API

An extension is a program purlis asks one question at a time. This page records **version 1 of
the Skill studio's capabilities**: the three words an extension can rely on, the messages each
one exchanges, and how they may change. The reasons are in
[ADR 0101](adr/0101-the-skill-studio-is-built-from-six-extension-capabilities-and-an-extension-that-declares-one-is-confined.md)
and [ADR 0053](adr/0053-an-extension-is-granted-capabilities-one-at-a-time.md).

The words are known only where the Skill studio is on, until the studio's finish line (#2786).
Where it is off, a manifest that names one is refused like any unknown word.

## What version 1 holds

| Word | Since protocol | What purlis does for the extension |
|---|---|---|
| `grader` | 4 | Asks it to grade a skill's trigger tests, and runs every chat it asks for. |
| `studio-panel` | 3 | Asks it for a panel on a skill's tab, answered in panel blocks. |
| `skill-router` | 3 | Asks it which skills a chat's listing hides, and, if it reads prompts, which skills suit a prompt. |

**Three studio words are not in version 1.** `skill-source`, `skill-lint` and `skill-template`
work but are not fully wired, so the approval prompt calls them experimental and they may change
shape with no protocol promise:

| Word | What is missing |
|---|---|
| `skill-source` | Installing and pinning from it (#3089), and showing in the studio's Browse (#3092). |
| `skill-lint` | Its findings in the install review (#3090). |
| `skill-template` | Landing as a draft skill (#3091). |

## How version 1 changes

`version` in a manifest is the protocol the extension speaks, and purlis asks each extension in
the protocol its manifest names.

- **A change to what a request or an answer holds is a new protocol.** Protocol 4 stays as it is.
- **purlis keeps answering every earlier protocol**, so an extension written for version 1 keeps
  working as purlis moves on.
- **Every answer is read against a closed shape.** A key purlis does not know refuses the answer,
  so an extension cannot add a field of its own.
- **A new kind of panel block is not a new protocol.** A block is added for every panel, and an
  extension that does not answer it sees no change.

The messages below are recorded in
[`tests/fixtures/extension-api/v1.json`](../tests/fixtures/extension-api/v1.json), and
purlis-core's tests hold each build to that file. It changes only with a new protocol.

## The manifest

```json
{
  "version": 4,
  "id": "house-studio",
  "name": "House studio",
  "capabilities": ["grader", "studio-panel", "skill-router"],
  "contributes": {
    "runs": "bin/house-studio",
    "grader": { "title": "House grader" },
    "studio-panel": [{ "id": "checks", "title": "Checks" }],
    "skill-router": { "title": "House router", "reads-prompts": true }
  }
}
```

- A manifest with `grader` is version 4 at least. One with only the other two may be version 3.
- `studio-panel` lists at most 8 panels.
- `reads-prompts` is optional and false where absent. The approval prompt says when it is true,
  and the router still reads no prompt until the person turns that on.
- `hosts`, a top-level list of at most 16 names, is what the program may reach. A host that
  serves a model is refused.

## An extension with one of these words is confined

An extension that declares any studio word runs confined for every question, in the same sandbox
a chat gets. It reads its own folder, the system's programs and what a question hands it. It
writes its state folder and a temporary folder. It reaches only the hosts it declares, and never
a model: every chat it needs is one purlis runs. Where purlis cannot confine it, it does not load.

## Questions and answers

Each question is one JSON line:

```json
{ "charter": 4, "extension": "house-studio", "studio": "skill-router", "ask": { "ask": "…" }, "writes": [] }
```

`charter` is the protocol, and every answer repeats it. `writes` is always empty for a confined
extension. An answer is one line of JSON, or `{"charter": 4, "error": "<sentence>"}`. An answer
has 5 seconds, unless this page says otherwise.

### A panel

Asked with `ask` `panel`, naming `panel`, `skill` and the skill's text `files`, each `path`,
`text` and `executable`. Answered with `blocks`, drawn from the panel vocabulary: `list`, `note`,
`chart`, `facts` and `table`.

### A router

**Which skills a chat's listing hides.** Asked with `ask` `route` at a chat's start: `skills`
(each `name`, `description` and `pinned`), the `budget`, and what the rest of the listing spends,
`besides`, each in `skills` and `chars`. Answered with `hidden`, each a `skill` and a one-line
`why`. It has 100 ms, from before its program starts. It never sees the prompt or the chat.

**Which skills suit a prompt.** Asked with `ask` `prompt`, only of a router that declares
`reads-prompts` and that the person has turned on for reading prompts on the Skill studio's
Routing page. It is handed the `prompt`, cut to 16 KiB, and the `skills` its chat's list leaves
out, each `name` and `line`. Answered with `pointed`, at most 5 of those names. It has 100 ms too.

If a router fails or is late at a chat's start, the built-in router routes the chat. If it fails
or is late on a prompt, the prompt goes on with no pointer. The routing record says so either way.

### A grade

A grade is one question of many lines. Asked with `ask` `grade` and a `request`: the `skill`, its
`triggers` (each `query` and `should_trigger`), the `arms` to compare (`with`, `without`,
`previous`), and `runs`, how many times each test runs in each arm. `outcomes` is empty in version
1, because a Grader scores trigger tests alone. A person who ticks outcome evals grades with
purlis's own runner.

Each line the Grader answers is one of two:

- **A run-chat request**, `{"charter": 4, "run_chat": {"arm": "with", "prompt": "…"}}`, on one of
  the grade's arms and the query of one of its tests. purlis runs the chat and writes back
  `chat_ran`: `chat`, `started` (the skills that started), `tokens`, `cost_usd`, `ms`, and `end`,
  one of `turn`, `timed-out`, `not-started` (with a fixed `why`), `cancelled`, `asked`, `over-cap`
  or `over-limit`.
- **Its grade report**, `{"charter": 4, "report": {"triggers": […], "arms": […], "delta": …}}`,
  which ends the question. What a grade spent, how it ended and who graded it are purlis's to say,
  so a report that carries them is refused.

Each answer has 5 seconds from the line before it, and a Grader has no deadline while purlis runs
a chat. A line is at most 512 KiB. A chat past the plan, a run past `runs`, or a report that does
not hold together ends the grade, and what ran is scored by purlis's own runner. When the grade
has reached a limit, the next chat asked for is answered as not run so the Grader can report. A
second one ends the grade.
