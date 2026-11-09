# A Codex chat's briefing takes four hooks, each within what Codex keeps

**Amends [ADR 0102](../0102-a-skill-router-decides-what-a-chat-sees-and-nothing-it-hides-is-out-of-reach.md), 2026-11-09**, ticket #5935. It settles what [Codex keeps 10,000 bytes of a hook, and a Codex chat's skills take a hook of their own](2026-11-09-codex-keeps-10000-bytes-of-a-hook-and-the-skills-take-one-of-their-own.md) left open: *the rest of a Codex chat's briefing is still cut past 10,000 bytes*. It changes how a Codex chat is told its briefing (ADR 0063's briefing route). Claude Code's and opencode's briefings stay as they were.

## The problem

Codex keeps 10,000 bytes of each `SessionStart` hook's output. Past that it keeps about the first and last 4,900 and cuts the middle (codex-cli 0.147.0, measured in the amendment above). Each hook is cut on its own and becomes its own developer message. After #5841 the skills had a hook of their own, but the rest of the briefing (`purlis hook sessionstart`) was still one output. In an ordinary workspace it passes 10,000 bytes. The chat that carries this ticket was briefed with 14.0 KB. The test's fixture workspace (a persona with a long memory, a shared memory, fourteen todos, four other workspaces, and a sandboxed chat the app started) comes to 10,665 bytes without a piece, a session record, where the chat is working, an extension or a kept report. So a Codex chat lost whatever sat in the middle: the end of the persona's memory digest, the routes for work and credentials, the todos, the other workspaces.

## The decision

**A Codex chat is told its briefing in four `SessionStart` hooks, grouped by what each must keep whole.** Each hook gets its own 10,000 bytes. Codex is armed with them in this order:

| Hook | What it tells | Size in the fixture |
|---|---|---|
| `purlis hook sessionstart` | **Who and where the chat is** (`briefing::Part::Briefing`): the gate or the plane root, the persona and its memory digest, the retired persona's notes, unshared memory, the last session record, the todos, the other workspaces, where its files go, the piece, a hidden `AGENTS.md` | 6,139 |
| `purlis hook sessionstart-notes` | **How it works here** (`briefing::Part::Notes`): the one route for work, credentials, drafting purlis's own bugs, the sandbox and committing in a branch folder, or the note that it runs unsandboxed | 4,524 |
| `purlis hook sessionstart-skills` | **Its skills**, as #5841 made it | 6,312 |
| `purlis hook sessionstart-quoted` | **What others say to it, quoted** (`briefing::Part::Quoted` and the hook's own): the session record a Resume picks up, each extension's section, the reports kept for its place, and where it is working | none here; no app |

- **Every block is told once, in Claude Code's words and in Claude Code's order within its hook.** `briefing::parts` is still the whole briefing in its old order. `briefing::parts_of` gives one part's blocks of it, so nothing is written twice and nothing is reworded for Codex.
- **The briefing's hook still does what `sessionstart` did of the chat's state**: it announces the piece, keeps the heartbeat, snapshots the persona tool gate and reports the start to the app. Notes and skills only print.
- **The quoted hook does what telling others' words always did**: it takes the reports kept for the chat's place, so the next chat is not handed them again, and it tells each extension that hears it that a chat started. For a Codex chat that now happens in the quoted hook, once, and never in the briefing's.
- The three hooks after the briefing's say nothing to a chat whose harness is not Codex.
- **Which chat is told in parts** is read from the chat's harness as its start names it (`PURLIS_HARNESS`, `harness::codex::briefed_in_parts`), the one reader every hook uses. Only Codex is armed with the new words. A chat that names no harness is a terminal's own Claude Code, and it is told the whole briefing.
- **Splitting, not shortening.** Each block is still told whole and in the same words, so nothing a Claude Code chat learns is withheld from a Codex chat. Moving the important blocks to the ends of one output would still lose the middle, and the briefing grows with the plane.

### The one hook that may pass the cut, on purpose

`sessionstart-quoted` holds other parties' words: the session record a Resume picks up (up to 8,000 characters of it), extensions' sections, reports closed chats handed back (each up to 4,096 bytes, and any number may wait), and the app's picture of where the chat is working (at most 20 rows). purlis sets no bound on them together, so this hook may pass 10,000 bytes. In that order, what lands at its start is the resumed record or the first extension's section, and what lands at its end is the last report kept and where the chat is working. Codex saves the whole output to a file and names it to the chat on the message's last line, so a cut middle can still be read.

`crates/purlis-cli/tests/it/each_of_a_codex_chats_session_start_hooks_fits_what_codex_keeps.rs` holds the other three hooks under the cut in the fixture workspace. It names this hook as the exception, and drives it past the cut with three kept reports, showing the first report's start and the last report's end at its ends. The resumed record is told in this hook and not the briefing's (`a_codex_chat_is_told_the_record_it_resumes_in_its_quoted_part_and_nowhere_else`), so a Resume cannot push the briefing over.

## Rejected

- **Shortening blocks for Codex.** A second wording of each block to keep in step, and a Codex chat told less than a Claude Code chat. The blocks that grow with the plane (the memory digest, the workspaces) already show only their newest ten and five.
- **One hook per block.** Each new hook is one more to trust and one more process at the start, and three groups already keep each one well under the cut.
- **Putting the notes into the skills' hook.** The skills list may reach its budget, which Settings lets the person raise toward the hook's 10,000. Notes there would push it over.

## What it costs

- **Codex asks once more to trust purlis's hooks**, for the two new words, as it did for #5841, #1663 and #1086. A sandboxed chat trusts them automatically through `sandbox::codex::hook_trust`. An unsandboxed Codex chat on a machine that trusted the old hooks and not yet these is not told its notes or the quoted until the person answers Codex's review prompt. Until then its extensions are not told that it started, and the reports kept for its place wait for the next chat that runs the quoted hook, so none is lost.
- **Two more processes at a Codex chat's start**, each printing what the briefing's one printed before. The extensions are asked from the quoted hook within the same bound (`Bounds::SESSION_START`), so the start waits no longer on them.

**Built by #5935.**
