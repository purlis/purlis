---
name: purlis-curation-action
description: When a person wants a purlis persona to offer a chat in the Curate menu, with a prompt typed, write its curation action. Use when asked to add, change or remove a curation action or Curate menu entry. Not for writing a skill (purlis-skill-studio) or a persona (purlis-persona).
---

# Adding a curation action

A **curation action** is a chat the person opens from a workspace, a persona or the plane,
with a prompt already typed into it. The person reads the prompt and presses Enter; nothing
sends it for them. The persona that declares the action is the one that runs it.

Each is one file, `personas/<persona>/curation/<id>.md`:

```markdown
---
label: Review open work
on: workspace
runs-in: subject
---

Use the review skill on the workspace {subject.name}.
```

- `label` — what the menu shows. Required, one line.
- `on` — the kinds of subject it is offered on: `workspace`, `persona`, `plane`, comma-separated.
  Required.
- `runs-in` — `subject` (the workspace's directory, `personas/<name>/`, or the plane root) or
  `plane`. Without it, a workspace action runs in the workspace and the others at the plane
  root.
- The body is the prompt. It may use exactly four variables: `{subject.kind}`,
  `{subject.name}`, `{subject.path}` and `{plane.root}`. They are filled in as plain text;
  any other `{word}` is an error.

## 1. Ask what it is for

Ask the person, with `ask_operator`, what the chat should do, which subjects it belongs on, and
where it should run. Draft the prompt as the person would want to read it before pressing Enter: **one short,
plain line** that says what to do and on which subject. When the work has steps, put them in a
skill and name the skill in words in that line — never as a `/slash` command, which only one
harness reads.

Keep it short because the harness shows a long paste as a placeholder nobody can read, and
purlis then opens no chat for it: Claude Code over 800 characters or at 4 lines, Codex over
1,000 characters. `purlis persona lint` warns when a prompt, filled in for a long subject name,
would be shown that way. Done when the person approves the label, the kinds and the prompt.

## 2. Write it

```bash
purlis persona curation add <persona> <id> --label "<label>" --on workspace,persona [--runs-in plane] <<'PROMPT'
<the prompt>
PROMPT
```

`<id>` is lowercase letters, digits, `.`, `_` and `-`. The command writes the file only when it
reads back with no error, and never over one that is there: to change an action, remove it and
add it again.

```bash
purlis persona curation remove <persona> <id>
```

purlis's own actions are `charter/safe-remove`, `charter/compact`,
`charter/add-curation-action` and `charter/create-theme`. A persona's action with one of their ids or labels is refused,
so choose another.

## 3. Check it

```bash
purlis persona curation list <persona>
purlis curation show workspace:<name>     # or persona:<name>, or plane
purlis persona lint <persona>
```

`curation show` prints what a subject is offered, in order, each with who runs it, where, and
the prompt as it will be typed. Show the person what `curation list` says. Done when the new
action is listed there with no warning, and `persona lint` has no warning about it. The
file is committed with the persona and travels with the plane's next save.
