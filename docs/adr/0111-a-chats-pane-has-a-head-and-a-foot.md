# A chat's pane has a head and a foot

**Accepted 2026-11-04**, from the person's grill of the pane's top line (spec #5191, milestone
M70 · Pane head and foot) and their review of the prototype on `prototype/pane-head-foot`. It
replaces the ruling of #4760, on the map #4575, that a chat's pane head is one line.

## Where purlis was

#4760 ruled that a chat's pane draws everything about the chat on one line above its terminal:
the persona and its state, the name and the state button, harness · workspace, the skills
warning, where the chat came from, the `ctx` and `cache` gauge with a row of bars, then Stop, ⋯,
the bell, the two splits and Close. Together it read as noise: no part stood out, and the parts
that say what the person can do sat among parts that only describe the chat. In a narrow pane
the line dropped harness · workspace first, though the workspace is often what tells two chats
apart.

## The decision

- **A chat's pane has two lines, each with one job.** The **pane head**, above the terminal,
  says which chat this is, what it is doing, and what the person can do to it now. The **pane
  foot**, a quiet line under the terminal, says where the chat runs, where it came from, and how
  healthy it is.
- **Harness · workspace and the gauge are the foot's.** They leave the head (#5202), with the
  other-workspace badge that stands for the workspace. `ctx` carries its fill as a thin bar
  under its number, and the separate row of bars goes: one figure says what the number and the
  row said twice. Where the chat came from, the skills warning and the sandbox follow (#5204).
- **One model answers both lines.** The pane head's pure model (`paneHead.ts`) answers, for a
  pane's kind and width, the pane line's parts and the foot's parts, and the pane draws what it
  answers. The head's actions are in its answer too (#5206). A shell and a one-job program
  draw no foot and keep harness · workspace on their heads. The head's drop order loses its
  workspace step, and the foot gets its own (#5206).
- **The foot is a themable family of its own**, `pane-foot` (ADR 0103, ADR 0104), drawn in look
  A: its parts joined by `·`, in muted words, on the terminal's fill, the tab strip's height at
  the density in force, so a compact appearance makes it compact.
- **The controls guard holds each line's chips apart.** Its list of the pane's chips splits into
  the head's, `PANE_LINE_CHIPS`, and the foot's, `PANE_FOOT_CHIPS`, each read off its own row and
  held to the chrome height, so a chip cannot drift back onto the head unnoticed.
- **The foot never asks.** Its parts are facts, each a shortcut to a row that already exists in
  ⋯ and the palette (#5204). Asks stay in the notices band under the head (#4754).

## What it costs

A chat's terminal is one foot's height shorter. The foot is drawn whether or not the gauge is
known yet, so that cost is paid once and the terminal never moves when the first usage arrives.
The per-turn bars said each turn's cache share on their tooltips; that history is still in the
chat's record, and no surface draws it now.

## Amendments

The amendments to this decision are files of their own in [`0111/`](0111/), one per ruling.

## The words

**Pane foot** joins the glossary. **Pane line** narrows to the head's row of chips
(`CONTEXT.md`).
