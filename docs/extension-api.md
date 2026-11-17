# The extension API

An extension is a program purlis asks one question at a time. This page records **versions 1 and
2 of the Skill studio's capabilities**: the three words an extension can rely on, the messages
each one exchanges, and how they may change. The reasons are in
[ADR 0101](adr/0101-the-skill-studio-is-built-from-six-extension-capabilities-and-an-extension-that-declares-one-is-confined.md)
and [ADR 0053](adr/0053-an-extension-is-granted-capabilities-one-at-a-time.md).

## What versions 1 and 2 hold

| Word | Since protocol | What purlis does for the extension |
|---|---|---|
| `grader` | 4 | Asks it to grade a skill's trigger tests, and runs every chat it asks for. |
| `studio-panel` | 3 | Asks it for a panel on a skill's tab, answered in panel blocks. |
| `skill-router` | 3 | Asks it which skills a chat's listing hides, and, if it reads prompts, which skills suit a prompt. |

**Three studio words are in neither version.** `skill-source`, `skill-lint` and `skill-template`
work but are not fully wired, so the approval prompt calls them experimental and they may change
shape with no protocol promise:

| Word | What is missing |
|---|---|
| `skill-source` | Nothing the studio needs: it installs, pins and updates through purlis's review (#3089), shows in Sources (#3092), and installs from `purlis skill install --extension` and updates in Review updates (#5760). It joins version 1 by an amendment of its own. A skill's files whose paths are one on a disk that ignores case are refused. Its files answer may hold `grade`, the object a git source's `<folder>.grades.json` holds, which the install review shows as the source's (#4346). |
| `skill-lint` | Its findings in the install review (#3090). |
| `skill-template` | Landing as a draft skill (#3091). |

## How the API changes

`version` in a manifest is the protocol the extension speaks, and purlis asks each extension in
the protocol its manifest names.

- **A change to what a request or an answer holds is a new protocol.** Each protocol stays as it
  is once recorded.
- **purlis keeps answering every earlier protocol**, so an extension written for version 1 keeps
  working as purlis moves on.
- **Every answer is read against a closed shape.** A key purlis does not know refuses the answer,
  so an extension cannot add a field of its own.

| API version | Protocol | Recorded in | What changed |
|---|---|---|---|
| 1 | 4 | [`v1.json`](../tests/fixtures/extension-api/v1.json) | The first freeze. |
| 2 | 5 | [`v2.json`](../tests/fixtures/extension-api/v2.json) | A route question gives each skill its `chars`, and the index's `index` (#4680). |

purlis-core's tests hold each build to every one of those files, and none changes once recorded.
The messages below are version 2's, under `"charter": 5`; a version 1 extension is asked the
same messages under `"charter": 4`, with no `chars` on a route question's skills and no `index`.

## The manifest

```json
{
  "version": 5,
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
  serves a model is refused, and so is any address a model provider's hosts resolve to when the
  program connects.

## An extension with one of these words is confined

An extension that declares any studio word runs confined for every question, in the same sandbox
a chat gets. It reads its own folder, the system's programs and what a question hands it. It
writes its state folder and a temporary folder. It reaches only the hosts it declares, and never
a model: every chat it needs is one purlis runs. Where purlis cannot confine it, it does not load.

## Questions and answers

Each question is one JSON line:

```json
{ "charter": 5, "extension": "house-studio", "studio": "skill-router", "ask": { "ask": "…" }, "writes": [] }
```

`charter` is the protocol, and every answer must repeat it. `writes` is always empty for a
confined extension. `settings` is there too, with the person's values, if the manifest declares
settings. An answer is one line of JSON, or `{"charter": 5, "error": "<sentence>"}`. An answer
has 5 seconds, unless this page says otherwise.

### An event's answer may offer a view

An extension that declares `events` is told what happened, after it happened, with `event` and
the `workspace` it happened in. It answers `{"charter": <its protocol>}`, or an `error`. Since
#4769 the answer may also carry one `offer`, naming one of the views its own manifest declares:

```json
{ "charter": 2, "offer": { "view": "stats", "key": "steward", "chat": 4 } }
```

- `view` is required, and must be one of the extension's declared views. Any other view refuses
  the whole answer, as a row offering an undeclared action does.
- `key` is what the view is about, a persona's name. It is optional, and empty means the whole
  plane's view.
- `chat` is optional: the chat the offer is about, by its number.
- Nothing else goes in an offer, and nothing says where the view would go.

An offer opens nothing. The window shows it as one quiet note, on the chat's pane band while that
chat is on screen and in the Notices centre, and asks the program nothing until the person
presses Open or Open beside. A second offer of the same view and subject, while one stands or the
view is open, adds nothing. Only an event the window tells is shown: one the command line or a
chat's start tells has no window to show it in. The key is additive: an answer without it means
what it always meant.

### A panel

Asked with `ask` `panel`, naming `panel`, `skill` and the skill's text `files`, each `path`,
`text` and `executable`. Answered with `blocks`, drawn from the panel vocabulary: `list`, `note`,
`chart`, `facts` and `table`.

### A router

**Which skills a chat's listing hides.** Asked with `ask` `route` at a chat's start: `skills`
(each `name`, `description`, `pinned` and `chars`), the `budget`, what the rest of the listing
spends, `besides`, and what the index costs, `index`, each in `skills` and `chars`. Answered with `hidden`, each a `skill` and a
one-line `why`. It has 100 ms, from before its program starts. It never sees the prompt or the
chat.

A skill's `chars` is what it costs of the budget's `chars` on the chat's harness, counted as
`besides` is. On a harness that counts whole lines, such as Claude Code, that is its name, its
description, its `when_to_use` and the line around them, so a router that adds description lengths
under-counts there. When a router hides any skill, purlis lists its index skill in their place,
so that costs `index` too. A router keeps within `budget` when `besides`, the `chars` of each skill
it leaves listed and, if it hides any, `index` add up to no more than it, and to no more skills.
Protocol 4 has no `chars` on a skill and no `index`: a router of version 1 is asked as it was.

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
1, because a Grader scores trigger tests alone. A plan with outcome evals ticked cannot start on a
Grader: the person unticks them or picks purlis's own runner.

Each line the Grader answers is one of two:

- **A run-chat request**, `{"charter": 5, "run_chat": {"arm": "with", "prompt": "…"}}`, on one of
  the grade's arms and the query of one of its tests. purlis runs the chat and writes back
  `chat_ran`: `chat`, `started` (the skills that started), `tokens`, `cost_usd`, `ms`, and `end`,
  one of `turn`, `timed-out`, `not-started` (with a fixed sentence beside it, `why`), `cancelled`,
  `asked`, `over-cap` or `over-limit`. `chat`, `tokens` and `cost_usd` are `null` where there is
  none to say, as for a chat that did not run.
- **Its grade report**, `{"charter": 5, "report": {"triggers": […], "arms": […], "delta": …}}`,
  which ends the question. What a grade spent, how it ended and who graded it are purlis's to say,
  so a report that carries them is refused.

**A report has to hold together**, or the grade ends as the Grader's failure. A chat counts as run
only if its `end` is `turn` or `timed-out`.

- Each of `triggers` is a test of the grade in one of its arms: `query`, `should_trigger`, `arm`,
  `runs` (its chats that ran), `triggered` (at most `runs`), `rate`, `pass`, `chats`, and `asked`
  (optional, its chats that ended `asked`).
- `rate` is `triggered` over `runs`, and `null` at no runs. `pass` is `null` exactly when `rate`
  is.
- `chats` lists every chat asked for, by its id or `null`, and names each chat of the grade at
  most once. Its length, and `runs` plus `asked`, are each at most the request's `runs`.
- Each of `arms` is one arm, once: `arm`, `pass_rate`, `tokens`, `seconds`, `chats`, `not_run`.
  `chats` is purlis's own count of chats that ran, and the sum of its tests' `runs`. `chats` plus
  `not_run` is the number of chats its tests list.
- `pass_rate`, `tokens` and `seconds` are `null` or `mean`, `stddev`, `min` and `max`, with `min`
  no more than `mean` and `mean` no more than `max`. A `pass_rate` is a share, from 0 to 1.

Each answer has 5 seconds from the line before it, and a Grader has no deadline while purlis runs
a chat. A line is at most 512 KiB. A chat past the plan or a run past `runs` ends the grade too, and
what ran is scored by purlis's own runner. When the grade
has reached a limit, the next chat asked for is answered as not run so the Grader can report. A
second one ends the grade.

## A Check reporter (protocol 6)

**`check-reporter` turns one tool's output into SARIF** (ADR 0117, #5410). It is not a studio
word: it is known on every purlis, and an extension that declares it is watched, not confined, like
any extension. It has no shape under `contributes`, and it needs a program (`runs`) and `version` 6:

```json
{
  "version": 6,
  "id": "mypy-sarif",
  "name": "mypy to SARIF",
  "capabilities": ["check-reporter"],
  "contributes": { "runs": "bin/mypy-sarif" }
}
```

It is asked when a check's output is reported through it, by the operator, a script, CI or a chat:

```
purlis check report <branch> <output> --reporter mypy-sarif --status <the check's exit status>
```

**The question** is one JSON line, with what the check printed as text (anything not UTF-8
replaced), at most 8 MiB, and the status it exited with:

```json
{ "charter": 6, "extension": "mypy-sarif", "check": { "output": "a.py:3: error: …\n", "status": 1 }, "writes": [] }
```

**The answer is not a line of JSON.** It is what the program prints on stdout, which must be one
SARIF 2.1.0 document, and the program must exit 0. Its stdout is at most 512 KiB, and it has 5
seconds. A program that exits with another status gave no answer: purlis quotes what it printed on
stderr. Each tool the SARIF names replaces that tool's earlier results for the branch.

**A bad answer never stops the review.** SARIF purlis cannot read, a failed exit or a late answer is
kept as the reporter's fault, which the review draws as a Notice naming the extension. The results
recorded before stay, and its next answer that is recorded takes the fault away. The probe that
proves all of this is `crates/check-probe`.

## Queries (protocol 7)

**`queries` answers an artifact's page reading the extension's data** (ADR 0122 §9, #5971). It is
known on every purlis, and an extension that declares it is watched, not confined. It needs a
program (`runs`) and `version` 7, and declares each query under `contributes.queries`:

```json
{
  "version": 7,
  "id": "slack",
  "name": "Slack",
  "capabilities": ["queries"],
  "state": "state",
  "contributes": {
    "runs": "bin/slack",
    "queries": [
      {
        "name": "unread",
        "description": "Unread messages in the workspace's channels.",
        "scope": "workspace",
        "fresh_seconds": 60,
        "parameters": { "type": "object", "properties": { "channel": { "type": "string" } } },
        "result": { "type": "object" },
        "widens": { "channel": "a channel the workspace does not follow" }
      }
    ]
  }
}
```

A page reads it as `slack.unread@1`. **The question** is one JSON line; `workspace` is `null` for
a plane-scoped query, and `params` hold only what the query declares, of its types:

```json
{ "charter": 7, "extension": "slack", "query": { "name": "unread", "workspace": "alpha", "params": { "channel": "eng" } }, "writes": [] }
```

**The answer** is `{ "charter": 7, "data": … }`, with `"cursor": "<text>"` when there is a next
page, or an `error`. It has 5 seconds; a late one is the page's `timeout`.

**An answer left in the facts file is used without starting the program** while it is younger
than `fresh_seconds`, for a query asked with no parameters: `<state>/facts.json` holds
`{"workspaces": {"<ws>": {"queries": {"<name>": {"data": …, "at": <unix seconds>}}}}}`, and
`{"queries": {"<name>": …}}` for a plane-scoped query. A page subscribed to the query is asked
again each `fresh_seconds`. The probe that proves this is `query-probe`, the extension probe's
second manifest.

## Artifact templates (protocol 8)

**`artifacts` ships pages a person may pin to a workspace** (ADR 0122 §9, #5973). It is known on
every purlis, needs `version` 8 and no program, and declares at most 8 templates under
`contributes.artifacts`, each a folder inside the extension holding its `index.html`:

```json
{
  "version": 8,
  "id": "slack",
  "name": "Slack",
  "capabilities": ["artifacts", "queries"],
  "contributes": {
    "runs": "bin/slack",
    "artifacts": [
      { "name": "inbox", "title": "Slack inbox", "folder": "artifacts/inbox", "icon": "mail" }
    ],
    "queries": [ … ]
  }
}
```

`name` is the template's id after `slack.`; `title` is what the pin is called unless the person
names it; `icon`, one of purlis's icons, is optional. **purlis asks the program nothing about a
template**: protocol 8 changes no question or answer, and an extension that also answers queries
is asked them as in protocol 7.

`purlis artifact templates` lists what this machine's approved extensions offer the workspace, and
`purlis artifact pin --template slack.inbox` **copies the template into the workspace's
`artifacts/slack.inbox/` and pins it there** as any page is pinned: refused when it is over the size
cap or holds something shaped like a secret, never written over a folder already there, and its
script approved by each person on their own machine. Its data files are not copied. From then on
it is the workspace's page, shared with it; a teammate without the extension sees it with
`needs-extension` where its queries' data would be. **Not on Windows**, where purlis starts no
extension. The probe that proves this is `artifact-probe`, the extension probe's third manifest.
