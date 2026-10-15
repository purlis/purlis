# Amendment, 2026-09-27: Codex is typed into once its terminal is raw and quiet; opencode is not

**Pending the operator's ruling (Q25).** Codex and opencode report `SessionStart` only with the
first prompt, so no hook marks the moment a prompt can be typed into either. This amendment
gives Codex a moment that is not its output, and records why opencode still has none.

## The rule

A Codex curation chat's prompt is held from before the chat starts (`Typed::hold_until_quiet`),
and a thread of its own asks the chat's terminal every 20 ms:

1. **Is it raw?** `Session::edits_lines`, the pty's `ICANON`, as the Claude Code path asks.
2. **Has it then written nothing for one second?** `Session::quiet_for`: the time since bytes
   last arrived from the pty, counted from the later of that and the moment the terminal was
   first seen raw, so a harness silent while still canonical is not typed into the moment it
   goes raw.

When both hold, the prompt is typed exactly as on Claude Code: one bracketed paste, line feeds
inside, every other control character taken out, nothing after `ESC [201~`. Not raw and quiet
within 15 seconds of the start, it is let go of, typed nowhere, and stderr says so. The dropping
rules are unchanged: a report of a prompt, a turn's end or the chat's end, or the chat ending or
being closed, drops it — and so does the chat's `SessionStart`, which on Codex comes inside the
first turn and so means something was already sent. Claude Code's path is unchanged.

A prompt Codex would draw as a placeholder is not typed: codex-cli 0.147.0 draws a paste over
1,000 characters as `[Pasted Content N chars]`, which the operator could not read before sending
(`Harness::longest_paste_drawn_whole`). `curate` refuses such an action before anything starts,
with the length in its sentence. purlis's own three are well under it.

## Why this is within "nothing parses harness output to decide anything"

The quiet period uses **only the fact that bytes arrived, and when**: `Session::quiet_for` is a
timestamp the reading thread sets on every read, before and without looking at what was read.
Nothing inspects a byte of it, so no wording, colour, layout or version of Codex's screen can
change when a prompt is typed, and no harness state is inferred from it: the chat's state still
comes from hooks only, and nothing but the one paste depends on this. It is the same kind of
fact as the terminal's `ICANON`, which ADR 0061 already reads: the kernel's, about the pty, not
the harness's, about itself. What it cannot tell is *what* is on the screen — a dialog that is
quiet looks like an input that is ready — and the cost of that is below.

## Measured

codex-cli 0.147.0 and opencode 1.18.32, each started exactly as the app starts it (the same
`-c hooks.*` flags, and for opencode the same `OPENCODE_CONFIG_CONTENT` shim and `OPENCODE_PURE=0`)
in purlis's own `Session` and terminal engine at 160×50, with a scratch `HOME`, `CODEX_HOME` and
`XDG_*` directories and a stand-in model server, so nothing reached a real model and neither the
operator's `~/.codex` nor `~/.config/opencode` was touched (listed before and after). A hook
logger stood in for `charter hook`. A control run that pressed Enter after the paste showed the
prompt reaching the stand-in and `userpromptsubmit` in the log, so both detectors see a send.

| | Codex 0.147.0 | opencode 1.18.32 |
|---|---|---|
| `?2004h` (bracketed paste on) | before the terminal goes raw, every run | within 1 ms of it going raw, every run |
| raw after start | 30–40 ms warm, 0.2 s cold, up to 1.2 s at load 40 | 0.63–0.94 s idle, up to 5 s at load 50 |
| longest silence after raw, before the first screen is whole | 0.16–0.18 s idle, 0.3 s with every core busy, 0.67 s at load 40 | 0.83–0.85 s idle, 1.25–1.42 s with every core busy, over 8 s at load 50 |
| a paste during that silence | landed whole (pasted 80 ms after raw, mid-draw) | **lost**: the box is drawn, then takes no keys until its boot ends |
| raw + 1 s quiet, a two-line prompt | 6 of 6: both lines in the input, one paste, nothing sent | — |
| raw + 1 s quiet, purlis's Safe remove prompt | 5 of 5 whole in the input, nothing sent | — |
| raw + 2 s or 2.5 s quiet | 27 of 27 | 20 of 20 up to load 44; **6 of 10 lost** at load 50, the boot outlasting the quiet period |

"Nothing sent" is no request at the stand-in and no `userpromptsubmit` in the hook log, for
every run. A prompt of 2 + 3 + 1 lines holding digits pasted into Codex's hooks-review dialog
did not answer it.

## What this costs

- **A quiet dialog swallows the prompt.** Codex asks once to review purlis's hooks ("Hooks need
  review") the first time it sees them. That screen is raw and quiet, the paste lands on it and
  is discarded — nothing is sent and nothing is chosen, measured — and the chat opens with an
  empty input once the operator answers. purlis cannot tell, because telling would mean
  reading the screen. The folder-trust screen animates, is never quiet, and is never typed into:
  the prompt is let go of at 15 s.
- ~~**A prompt typed while the operator types.** Nothing drops the prompt when the operator
  starts typing in the second before it is pasted; it lands after their text, still unsent.~~
  Lifted by the second amendment of 2026-09-27 below (Q29): the operator's input drops it.
- **A second later than on Claude Code.** The prompt appears about 1.3 s after the chat opens.

## Why not opencode

opencode draws its input box, goes raw, and then boots silently before the box takes a key. The
silence is its own work (plugins, the project, the file watcher; its log shows it), so it grows
with load: 0.83 s idle, 1.4 s with every core busy, and more than 8 s while this machine ran
other builds at load 50 — and a paste in it vanished, typed nowhere, with nothing to say so. The
silence has no bound purlis can know: a quiet period over the 8 s measured would keep every
opencode curation chat empty for most of ten seconds and still lose a prompt on a busier
machine, and one short enough to wait for lost six of ten at load 50. So opencode as the default profile
is still refused, now with that reason. Two findings for whoever gives it a moment:

- The shim runs inside opencode and could report when the server is ready — the one kind of
  signal that is a hook rather than a guess. That is a change to ADR 0058's shim and is not made
  here.
- opencode draws a paste of three lines or more than 150 characters as `[Pasted ~N lines]`;
  `experimental.disable_paste_summary` in the session's `OPENCODE_CONFIG_CONTENT` draws it
  whole (measured), unless the operator has toggled the summary in opencode, whose stored
  choice wins. ~~Every built-in prompt is over that, so an opencode curation chat needs it.~~
  Since the second amendment of 2026-09-27, every built-in prompt is one line under 150
  characters for a 43-character subject name, and opencode draws it whole.

## Rejected

- **A longer quiet period for everyone.** It does not make opencode safe (above), and it makes
  Codex slower for no measured gain: Codex took a paste at any moment after going raw.
- **Reading the screen for the input box.** That is parsing harness output, and it breaks on the
  next redesign of either TUI.
- **Typing without bracketed paste.** Each line feed would be Enter.
