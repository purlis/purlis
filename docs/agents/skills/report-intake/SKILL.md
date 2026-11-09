---
name: report-intake
title: The daily Report intake routine
description: Promote the day's Report intake events (purlis's private Sentry EU project) to purlis/purlis issues labelled `reported`, add duplicates to the issues they repeat, triage what was filed, and post the daily summary. Use when the steward's Report intake curation action opens a chat, or when asked to run the report intake routine.
---

The steward runs this once a day (#5701, spec #5683, ADR 0118). `node tools/report-intake.mjs plan`
does the deterministic part: it reads the intake, groups duplicates by the device's fingerprint
and Sentry's own grouping, decides file or add, and renders the summary. Its header comment says
how. This skill is the judgement around it. Each step ends on its **done** line.

Work in a purlis clone of the `smart-ide` workspace on an up-to-date `main`.

## 1. Token

```bash
purlis secret list sentry
```

If `intake-read-token` is not listed, the intake is not set up yet. Run
`node tools/report-intake.mjs plan`, which says so and exits 2, post that message as the day's
summary (step 5), and stop. Nothing is read and nothing is filed. Never ask for the token's value,
print it, or put it on a command line. The Sentry setup wizard (#5687) stores it.

**Done:** the key is listed, or the routine has stopped with the wizard's message.

## 2. Plan

```bash
purlis secret exec sentry --env SENTRY_INTAKE_TOKEN=intake-read-token -- \
  node tools/report-intake.mjs plan > "$TMPDIR/intake-plan.json"
```

`steps` holds one entry for each distinct group of new reports, and `summary` holds the day's
Markdown. An event that a `reported` issue or comment already names is never planned again, so
running the routine twice in one day is safe. If the plan says the token was refused, post that
as the summary and stop.

**Done:** the plan is written, and you know how many steps it has.

## 3. Read, then apply each step

Read every step's `texts` first. They are what reached the intake, and much of it was written by
an agent and never read by a person. **Never paste them into an issue or comment.** purlis/purlis
is public, and ADR 0118 lets an unread report become public only once a maintainer has read it.
The planned title comes from the same report, so rewrite it in your own words if it names
anything of the person's work: a project, a client, a path or a person. A report that is not a
bug in purlis (the person's own project, a harness bug, a mistake by the person) is not filed: say
why in the summary, and the report ages out of the window.

- **`file`**: create the label once if it is missing
  (`gh label create reported -R purlis/purlis --color FBCA04 --description "Promoted from the Report intake"`),
  then `gh issue create -R purlis/purlis --label reported --title <title> --body-file <body>`.
  Keep the body's events list and its `<!-- purlis-intake … -->` marker exactly as planned: the
  marker is how tomorrow's run knows these events are handled. Add a short description above
  them in your own words.
- **`add`**: `gh api -X POST repos/purlis/purlis/issues/<issue>/reactions -f content=+1`, then
  `gh issue comment <issue> -R purlis/purlis --body-file <comment>` with the planned comment.
- **`closed`**: the same comment. If the reports come from a version released after the fix,
  it is a regression: reopen the issue (`gh issue reopen`) and say so in the comment.

**Done:** every step is filed, added, or left out with a reason, and you have each issue's number.

## 4. Triage

Use the triage skill on each issue you filed or reopened in step 3. It sets the labels and the
milestone, as `docs/agents/issue-tracker.md` asks.

**Done:** every filed issue has its triage labels.

## 5. Summary

Post the plan's `summary`, followed by one line per step naming the issue it filed or added to,
or why it was left out, as a comment on the issue titled `Report intake: the daily summary`
(labelled `bot-maintained`; create it the first time). It carries counts and issue numbers only,
never a report's words. Then write the session record.

**Done:** today's comment is posted and the session record names it.
