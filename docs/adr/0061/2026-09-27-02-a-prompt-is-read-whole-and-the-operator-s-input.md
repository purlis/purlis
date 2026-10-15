# Amendment, 2026-09-27 (second): a prompt is read whole, and the operator's input drops it

**Accepted 2026-09-27**, by the operator's rulings Q28 and Q29 on SI-2.

A curation prompt exists to be read before the operator presses Enter. Two things defeated that:
harnesses draw a long paste as a placeholder, and a prompt could land after the operator had
started typing.

## Q28: one line, and never a placeholder

**purlis's own prompts are one plain line** that names the skill and the subject, and the skill
holds the procedure:

- `charter/safe-remove`: *Use purlis's safe-remove skill to remove the {subject.kind}
  {subject.name}.*
- `charter/compact`: *Use purlis's compact skill to compact and improve the {subject.kind}
  {subject.name}.*
- `charter/add-curation-action`: *Use purlis's add-curation-action skill to add a curation
  action to the persona {subject.name}.*

What the old paragraphs said beyond the skill is now in the skill: every step, the guards,
`--force` belonging to the operator, and showing the operator what was promoted, what is left
behind, and what `curation list` says afterwards. The skill is named in words, never as a
`/slash` command, so the line works in every harness that reaches purlis's skills (ADR 0063):
Claude Code through the plugin, opencode through the shim's `skills.paths`, Codex through the
`SessionStart` briefing. A Codex chat whose hooks are not trusted yet has no briefing, and so
has no skills from purlis; but such a chat shows Codex's hooks-review dialog first, and that
dialog already swallows the paste (the amendment above), so the one line loses nothing there.

**Each harness's limit is measured and lives in one place**:
`Harness::longest_paste_drawn_whole()` returns the most lines and the most characters drawn
whole, and `Harness::why_drawn_as_a_placeholder(text)` is the one judgment that `curate` and
`purlis persona lint` both call, on the text inside the paste (`curation::pasted`).

| | drawn whole | placeholder |
|---|---|---|
| Claude Code 2.1.283 | up to 800 characters and up to 3 lines | `[Pasted text #N]` at 801 characters; `[Pasted text #N +M lines]` at 4 lines |
| Codex 0.147.0 | up to 1,000 characters, any number of lines (200 measured) | `[Pasted Content N chars]` at 1,001 |
| opencode 1.18.32 | up to 150 characters and up to 2 lines | `[Pasted ~N lines]` at 3 lines or 151 characters |

Measured 2026-09-27 on the operator's machine by bracketed pastes into each harness started in
purlis's own `Session` and terminal engine at 160×50, reading the input box and never sending:
Claude Code with the real `HOME` (it needs its login to draw the input) in a scratch directory,
trusted once through its own dialog, with `--settings '{"hooks":{}}'`; Codex with a scratch
`CODEX_HOME` and a stand-in model. For both, a line feed counts among the characters (three lines
of 266 plus two line feeds, 800, whole in Claude Code; 801 collapsed), a trailing line feed
starts a line, and a character is a character, not a byte (800 `é` whole, 801 collapsed). The
opencode figures are the first amendment's; they were not re-measured here, and opencode is
never typed into.

The three one-liners, rendered for the subject `customer-onboarding-platform-migration-2026`
(100, 107 and 124 characters), were drawn whole — every word in the input, no placeholder — in
Claude Code and in Codex, and nothing was sent.

- **`curate` refuses** a prompt the harness it resolved would draw as a placeholder, before
  anything starts, in a sentence naming the harness, its limit and the prompt's size.
- **`purlis persona lint` warns** about a persona's action whose prompt, rendered for a subject
  named `LONG_SUBJECT_NAME` (43 characters) on each kind in its `on`, would be drawn as a
  placeholder by any harness a curation chat can be typed into (`Harness::ready_to_type`), and
  names which. A warning, not an error: the file is well-formed, and on another harness, or with
  a shorter subject name, it is typed. opencode's limit is not linted against while opencode is
  never typed into; it counts the moment it can be.

## Q29: the operator's input drops a prompt still waiting

**Any input the app sends a curation chat from its pane before the prompt is typed drops the
prompt** — `send_input` and `send_input_bytes`, through `Held::operator_input`. The app knows
this itself, because it is the one sending the input; nothing reads the harness's output.

- **Held until written.** A prompt stays in `Typed` until the moment it is written, on Claude
  Code too: the `SessionStart` report only starts the wait for raw keys. `Typed::type_now` takes
  it and queues the paste under one lock, and `Typed::operator_sent` drops it under the same
  lock before the operator's bytes are queued. The session's input is one queue, so the paste is
  either ahead of the operator's bytes or never written.
- **The terminal's answers are not the operator.** xterm.js answers a program's questions
  through the pane's input. Measured at start: Claude Code asks for device attributes
  (`CSI c`), the terminal's version (`CSI > 0 q`) and the kitty keyboard flags (`CSI ? u`), and
  turns focus reports on; Codex asks for the cursor's position (`CSI 6 n`), device attributes and
  both colours (`OSC 10;?`, `OSC 11;?`), and turns focus reports on. A curation chat's tab is
  opened in front, so it answers several of these — and reports focus — before its prompt is
  typed. A send made only of such answers (`curation::only_the_terminal_answering`: device
  attributes, a cursor position, focus in or out, kitty flags, a mode, status or window report,
  an OSC or DCS string) does not drop the prompt. This reads the bytes the app is sending, not
  anything the harness wrote. Shift+F3 is `CSI 1 ; 2 R`, the same bytes as a cursor-position
  answer, and does not drop it; every other key, a paste and a mouse report do.

## What this costs

- **A prompt of a persona's that says everything in a paragraph is refused on Claude Code** past
  800 characters, where it used to be typed and shown as `[Pasted text #1 +N lines]`. That is
  the point, and `persona lint` says so first.
- **A prompt with a long enough subject can pass lint and still be refused**: lint renders one
  plausible long name, and a longer one exists. The refusal names the size, so the fix is plain.
- **A focus change or the terminal's answer never drops a prompt**, and neither does Shift+F3.
  A click in the pane that the program reports as a mouse event does.

## Rejected

- **Raising a harness's limit with a setting.** Neither Claude Code nor Codex has one a chat can
  be started with; opencode's `experimental.disable_paste_summary` loses to a choice the operator
  stored in opencode.
- **Every byte from the pane drops the prompt.** Measured above, both harnesses make the pane
  answer on its own while they start, and a Codex prompt would never be typed.
- **Asking the window to say which input was a key.** xterm.js's public API hands keys and its
  own answers to the same `onData`; telling them apart there would reach into its internals.
