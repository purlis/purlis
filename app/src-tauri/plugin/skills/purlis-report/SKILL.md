---
name: purlis-report
description: When purlis itself misbehaves — a command, tool or window fails, is wrong or slow, blocks its own work, or you work around it, or a purlis skill should have started — draft a report for the person. Not for their own project, the harness, permissions (purlis-permissions) or editing a skill (purlis-skill-studio).
---

# Drafting a purlis bug report

You draft; the person files. A draft goes to the person's Inbox, where they read it, edit it and
choose to file it or not. Your part ends when the draft is in their Inbox and you have told them
it is there.

## Is it purlis's?

It is purlis's in any of these six cases:

1. A purlis command or purlis tool fails, panics, or does something its help or docs say it
   does not.
2. purlis blocks its own work: its guard or sandbox refuses a step that purlis itself told you
   to take.
3. The purlis window shows something false: a state, a count, a Notice or a label that does not
   match what is true.
4. The person says purlis is broken or slow, or asks why it did something.
5. You are working around purlis: retrying a purlis command in another form, doing by hand what
   a purlis command should do, or avoiding a purlis feature because it misbehaves.
6. A purlis skill should have started and did not: the person had to name it, or you found it
   only after doing its work another way.

It is not purlis's when the fault is:

- **in your project**, the person's own code, tests, build or repos;
- **the harness's own**: Claude Code, Codex or opencode misbehaving in a way purlis plays no
  part in;
- **the person's own mistake**: a typo, a wrong path, a setting they chose. Help them fix it.

When unsure, draft. The person dismisses a wrong draft in one click; a bug nobody drafts is
lost.

## Write the draft

One draft per bug. The first line is the title: what went wrong, in plain words. Then:

- **What you did**: the command or tool call, as you ran it.
- **What happened**: the exit code and the error, in a few lines.
- **What you expected**, and where that expectation came from (help, docs, a skill, the person).
- **What you did instead**, for case 5.
- **Harness**: Claude Code, Codex or opencode, and its version if you know it.

Quote only the lines that show the fault: logs, transcripts, secrets and the person's code stay
out. purlis adds its own version and the OS, and scrubs what it knows to be private.

### 6. A skill that should have started

Case 6 needs three lines, so the miss can become a trigger test:

```text
Skill: purlis-<name>
Harness: <Claude Code, Codex or opencode, with its version if you know it>
Words: "<what the person said that should have started it, word for word>"
```

## Send it to the Inbox

```bash
purlis report bug --title "<title>" --stdin <<'EOF'
<the draft>
EOF
```

For a failed purlis command, add `--call "<the command>" --exit <code>`. A missing feature is
`purlis report feature` with the same flags.

The command answers where the draft went. Tell the person in one line that a draft is in their
Inbox and what it is about, then carry on with the work. If it says the person has not allowed
Agent drafts, tell them what you would have reported, in two lines, and carry on.
