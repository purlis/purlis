---
name: purlis-smart-close
description: When this purlis chat is ending, write its session record for the next chat to resume from, and fold what lasts into workspace.md. Use when asked to smart close, wrap up, call it a day or write a session record. Not for tidying memory (purlis-tidy-memory) or a new chat (purlis-handoff).
---

# Smart close

The chat is ending. Leave behind a **session record**: a summary the next chat reads to pick up
where this one stopped. A summary, never the transcript. The last command below is what ends this
chat, so run it last, once.

## 1. Review the chat

Read back over this conversation. Collect, for the record:

- **Goal** — what this chat set out to do.
- **Done** — what changed: files, commits, pull requests, decisions carried out.
- **Decisions** — what was decided, and why.
- **Open** — what is unfinished, blocked or still to ask.
- **How to resume** — the first thing the next chat does, and where.

Keep credentials out: name the vault a secret lives in, never its value. Done when you can write
each of the five in a few lines.

## 2. Fold what lasts into workspace.md

Put what should outlive this chat where the `purlis-remember` skill says a fact goes: that
skill holds the rule, so load it if this chat has not. In a workspace, most of it is the
charter. purlis writes its `workspace.md` for this chat, since a sandboxed chat is refused the
file itself: a lasting decision through the `workspace_section` tool with section `decisions`,
a term this work coined with section `glossary`, a changed goal through the `workspace_vision`
tool. Each `workspace_section` call adds one entry and keeps what is there. A harness without
purlis's tools sets the goal with `purlis workspace vision "<the goal>"`. Leave `## Sessions`
alone; that line is purlis's. Done when each lasting decision, term, goal change or lesson is
where that rule puts it, or there was none.

## 3. Write the record

Title it in one line. Name the branch folders this chat worked in that you are not standing in, as
`<repo>/<piece>`; purlis reads their branches from git. The body is exactly these five sections,
in this order, each with at least one line:

```markdown
## Goal

<what this chat set out to do>

## Done

<what changed>

## Decisions

<what was decided, and why>

## Open

<what is left; "Nothing." if nothing>

## How to resume

<the first step for the next chat>
```

Call purlis's `session_record` tool with `title`, `body` and, where there are any, `pieces`.
purlis writes the record for this chat, so it works from a sandboxed chat and from a repo. A
harness without purlis's tools runs the command instead, with the same title and body:

```bash
purlis session record --title "<one-line title>" --piece <repo>/<piece> <<'RECORD'
<the five sections>
RECORD
```

A refusal says what to fix: fix it and write the record again. Done when the answer names the
record's path. Then stop: when the person started this close, the app ends this chat once this
turn ends, and its pane keeps the last screen. When the answer says the chat will not end by itself,
tell the person the record is written and the chat is theirs to end.
