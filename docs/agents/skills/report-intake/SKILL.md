---
name: report-intake
title: The daily Report intake routine
description: Promote the day's Report intake events (purlis's private Sentry EU project) to urgent purlis/purlis issues labelled `reported` and `priority:p0`, each in its milestone, merge reports of one bug, add duplicates to the issues they repeat, triage what was filed, and post the daily summary. Use when the steward's Report intake curation action opens a chat, or when asked to run the report intake routine.
---

The steward runs this once a day (#5701, spec #5683, ADR 0118). `tools/report-intake.mjs` does
the deterministic part, and curl does the fetching. The tool says which pages to fetch. It reads the
pages curl saved, groups duplicates by the device's fingerprint and Sentry's own grouping, decides
whether to file or add, and renders the summary. Its header comment says how. This skill is the
judgement around it. Each step ends on its **done** line.

**Every issue it files is urgent, and has a milestone** (the operator, 2026-11-12): labels
`reported` and `priority:p0`, and the milestone `docs/agents/issue-tracker.md`'s rules pick.
A duplicate raises the issue it lands on to `priority:p0`.

**The tool never sees the token.** purlis hands a vault's value to no program that runs what it is
told, so `node` cannot be given it (ADR 0112, D-PE-ruling-2). curl is on purlis's reviewed list,
and from 8.3.0 it puts the token in its header itself, with no shell between.

Work in a purlis clone of the `smart-ide` workspace on an up-to-date `main`.

## 1. Token

```bash
purlis secret list sentry | node tools/report-intake.mjs has-token
```

If it exits 2, the intake is not set up yet. Post the message it printed as the day's summary
(step 5) and stop. Nothing is read and nothing is filed. Never ask for the token's value, print it,
or put it on a command line. The Sentry setup wizard (#5687) stores it.

**Done:** it exited 0, or the routine has stopped with the wizard's message.

## 2. Fetch, then plan

Fetch every page of the intake's events for the last four weeks with curl, at most 20 pages, and
the intake's issues page for their short ids (`PURLIS-APP-2`), then plan from the saved pages.
Run it with bash (`bash <file>` or `bash -c`): zsh reads an unquoted `=word` as a command's path.

```bash
dir="$(git rev-parse --show-toplevel)/.intake-run"
rm -rf "$dir" && mkdir "$dir"
url=$(node tools/report-intake.mjs url) && ok=1 n=1
while [ -n "$url" ] && [ "$n" -le 20 ]; do
  purlis secret exec sentry --env SENTRY_INTAKE_TOKEN=intake-read-token -- \
    curl -sS --proto '=https' --max-time 60 \
      --variable %SENTRY_INTAKE_TOKEN \
      --expand-header "Authorization: Bearer {{SENTRY_INTAKE_TOKEN}}" \
      -D "$dir/page-$n.headers" -o "$dir/page-$n.json" "$url" || { ok=0; break; }
  url=$(node tools/report-intake.mjs next "$dir/page-$n.headers") || { ok=0; break; }
  n=$((n + 1))
done
[ "$ok" = 1 ] && { purlis secret exec sentry --env SENTRY_INTAKE_TOKEN=intake-read-token -- \
    curl -sS --proto '=https' --max-time 60 \
      --variable %SENTRY_INTAKE_TOKEN \
      --expand-header "Authorization: Bearer {{SENTRY_INTAKE_TOKEN}}" \
      -D "$dir/issues-1.headers" -o "$dir/issues-1.json" "$(node tools/report-intake.mjs url --issues)" || ok=0; }
[ "$ok" = 1 ] && node tools/report-intake.mjs plan --from "$dir" > "$dir/plan.json"
echo "pages: $((n - 1)), ok: $ok, more left: ${url:+yes}"
```

Run it exactly as written, from the clone's root. The pages go to `.intake-run/` in the clone,
which git ignores and each run clears first: curl run under `purlis secret exec` cannot write to
`$TMPDIR` ("curl: (23) Failed writing received data"). Delete the folder once the summary is
posted (step 5). Each piece keeps the token away from anything but the intake:

- `url`, `url --issues` and `next` print an address on `https://de.sentry.io` alone, and refuse any other host.
- curl does not follow redirects without `-L`, and `--proto =https` keeps it on https.
- curl reads the token from its variable. Nothing on the command line holds it.
- `next` prints nothing after the last page (the `Link` header's `rel="next"` with
  `results="false"`), which ends the loop. Where 20 pages were read and `more left` says yes,
  the plan covers the oldest 20 pages' events alone: say so in the summary.

**The first run asks the person.** The steward holds no tool for curl with vault `sentry`, so the
first `purlis secret exec` raises one notice: the vault, the key, and an Allow for curl. Ask the
person for **Allow for this chat**, which covers every page of the loop. **Always allow** would
commit a `curl` tool line to the steward, and every curl a steward chat runs would then be handed
the token. Where the sandbox refuses `de.sentry.io`, ask for that host the same way.

**If it ends with `ok: 0`**, or the plan fails, nothing was planned. If `next` says the token was
refused (HTTP 401 or 403, naming the wizard's token step, #5687), post that message as the summary
and stop. For any other error, post what it says and stop, and do not plan from a part of the pages.
Either way, delete the run's folder before you stop, as step 5 does: it may hold pages already
fetched.

`steps` holds one entry for each distinct group of new reports, and `summary` holds the day's
Markdown. Each step's `shortIds` names its Sentry issues; an event whose issue was not on the
issues page is listed by its link alone. An event that a `reported` issue or comment already names is never planned again, so
running the routine twice in one day is safe.

**Done:** the plan is written, and you know how many steps it has.

## 3. Read, merge, then apply each step

Read every step's `texts` first. They are what reached the intake, and much of it was written by
an agent and never read by a person. **Never paste them into an issue or comment.** purlis/purlis
is public, and ADR 0118 lets an unread report become public only once a maintainer has read it.
The planned title comes from the same report, so rewrite it in your own words if it names
anything of the person's work: a project, a client, a path or a person. A report that is not a
bug in purlis (the person's own project, a harness bug, a mistake by the person) is not filed: say
why in the summary, and the report ages out of the window. A crash from a `local` channel is a
maintainer's own build: leave it out as well.

**Describe each bug in your own words.** Once you have read a step's reports, find the code that
answers them (`git grep` the refusal's or error's words) and, where you can do it harmlessly,
reproduce it. The description says what goes wrong, where in the code, and whether you
reproduced it, never what the report said.

**Merge reports of one bug.** The tool groups by device fingerprint and Sentry's grouping, so
two reports of the same underlying bug, worded apart, are two `file` steps. Where you judge them
one bug, join them (steps count from 1, in the plan's order):

```bash
node tools/report-intake.mjs merge --plan .intake-run/plan.json --steps 1,3 --title "<your title>" > .intake-run/merged-1.json
```

It answers one `file` step with every report, every Sentry short id and one marker naming every
event, so tomorrow's run knows them all. File that step in place of the ones it joined. Only
`file` steps merge: a report of a bug already filed is an `add` to that issue.

**Pick each new issue's milestone** by `docs/agents/issue-tracker.md`: choose the issue's
`area:*` label, then

```bash
node tools/report-intake.mjs milestone --area area:<name>
```

prints the milestone holding the most open issues of that area. Where it prints nothing, take
the closest existing milestone and say why in a comment on the issue.

- **`file`**: create the labels once if they are missing
  (`gh label create reported -R purlis/purlis --color FBCA04 --description "Promoted from the Report intake"`;
  `priority:p0` the same way, red), then
  `gh issue create -R purlis/purlis --label reported --label priority:p0 --label area:<name> --milestone "<milestone>" --title <title> --body-file <body>`,
  with every label of the step's `labels`. Keep the body's events list and its
  `<!-- purlis-intake … -->` marker exactly as planned: the marker is how tomorrow's run knows
  these events are handled. Put your description above them.
- **`add`**: `gh api -X POST repos/purlis/purlis/issues/<issue>/reactions -f content=+1`, then
  `gh issue comment <issue> -R purlis/purlis --body-file <comment>` with the planned comment.
  Where the step's `priority` is set, raise the issue:
  `gh issue edit <issue> -R purlis/purlis --add-label priority:p0`, with `--remove-label` for
  each label in its `remove`.
- **`closed`**: the same comment. If the reports come from a version released after the fix,
  it is a regression: reopen the issue (`gh issue reopen`), raise it as `add` does, and say so in
  the comment.

**Done:** every step is filed (urgent, in a milestone), merged, added, or left out with a
reason, and you have each issue's number.

## 4. Triage

Use the triage skill on each issue you filed or reopened in step 3. It sets the triage labels. It
keeps `priority:p0` and the milestone step 3 chose, unless the maintainer says otherwise.

**Done:** every filed issue has its triage labels, `priority:p0` and a milestone.

## 5. Summary

Post the plan's `summary`, followed by one line per step naming the issue it filed or added to,
or why it was left out, as a comment on the issue titled `Report intake: the daily summary`
(labelled `bot-maintained`; create it the first time). It carries counts and issue numbers only,
never a report's words. Then delete the run's folder (`rm -rf "$(git rev-parse --show-toplevel)/.intake-run"`), since its pages hold
the reports' words, and write the session record.

**Done:** today's comment is posted and the session record names it.
