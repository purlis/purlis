# Amended 2026-11-10: a timed-out ask stays answerable, and what Quiet holds open

Decisions 15 and 13 are built (#6007), with these particulars. They take up the table in this
ADR's [Quiet amendment](2026-11-10-quiet-is-built-and-which-expiring-asks-it-can-hold-open.md),
which left holding open to this one.

## A timed-out ask

- **It stays listed, and says so.** The registry carries when its wait ended
  (`Shown.timed_out`, seconds since 1970), never beside a live end (`Shown.expires`). Its row's
  reason line is "Timed out. Answer, and the chat is told to try again."
- **Its place** (`app/src/asksOrder.ts`): after the live Expiring asks and before an ask that
  blocks other chats, the longest wait first. Inside one chat's group: its Expiring asks, then its
  timed-out ones, then the rest.
- **Only where an answer still reaches the chat** is an ask kept timed out. Which those are, and
  what "told" means for each:

| Ask | What ends its wait | Kept timed out? | What answering it does, and how the chat is told |
| --- | --- | --- | --- |
| Permission prompt, Claude Code (`PermissionRequest`) | Claude Code kills the hook at 60 s | **Yes**, as a lapsed hook ask (`harness::hooked::lapsed`) | The answer waits for the chat's same request (the same action, and for an edit the same change): its next `PermissionRequest` for it is answered at once by its hook, and no one is asked. The chat is told on its next turn, in `askoutcome`'s fixed words, through the road every Ask's answer takes (`UserPromptSubmit`'s context, or a line typed into the chat when it waits for one): "Allowed by the person after it timed out — if it did not run, make the same request again and it is allowed at once", or "Denied … — don't make it again". |
| Permission prompt, Codex (`PermissionRequest`) | Codex kills the hook at 60 s | **Yes**, as above | As above: Codex's next `PermissionRequest` for the same call is answered at once, allow or deny, the only words its schema takes; told on its `UserPromptSubmit`. |
| Permission prompt, opencode (`permission.asked`, through purlis's shim) | the shim stops the hook at its ask time | **Yes**, as above | As above: the shim's next `permission.asked` for the same permission and patterns is answered at once with opencode's own reply word; told through the shim's `chat.message` context. A command or an edit of opencode's is still never allowed from the window (#1691), late or not. |
| Sandbox host consent (a held connection) | purlis's own hold, 60 s, in the proxy | **Yes**, since #1666 kept its ask; now marked (`taskblocks::Blocks::gave_up`) | An Allow applies at once, and the hosts that gave up are told to retry, as before ("reaching `api.example.com:443` — Allowed for this chat — retry."). |
| Run grant (`purlis secret exec`) | its command's hold, 90 s (7.5 s on Codex) | **Yes**: the core holds it open past its command | As before: the next run of it passes, and a chat no command waits for is told on its next turn (`RunAnswered::told`). The row now says it timed out. |
| Secret ask (a vault value) | as a run grant | **Yes**, as a run grant | As before: told "Filled by the person — run the command that waited on it again." |
| A question to the person (`AskUserQuestion`), a form, an unsaid nudge | the hook's kill | **No** | A later question is not the same request: it is asked anew, or not at all. |
| Command confirmation in the window | taken back as a no at 2 minutes | **No** | The command has ended with its no; nothing waits to be told. |
| Browser login, secret fill, pool tool | the waiting `purlis` command's read deadline | **No** | The command has given up first, so nothing is there to answer. |

- **A lapsed permission ask goes** when it is answered, when its chat asks the same again (the new
  ask is the one to answer), when its chat ends, or after 30 minutes (`lapsed::LAPSED_FOR`); a
  chat keeps at most its newest few. **An answer waiting for the same request** is spent once,
  and goes after 30 minutes (`PRE_ANSWERED_FOR`) or with its chat. Both are in memory only, so
  no tier is named (ADR 0069).
- **The harness's own prompt still waits in the pane.** On all three harnesses, purlis's
  permission hook runs beside the harness's own prompt, and killing the hook leaves that prompt
  waiting: only the pane answers it. So a chat whose permission ask timed out is also listed
  waiting in its terminal, beside the timed-out row, and is not hidden by it. The timed-out row's
  answer reaches the chat on the same request again, or on its next turn, as the table says; it
  never types into the pane.
- **What answering one moves.** Nothing on the chat's board: the chat may still wait on its
  pane's prompt.

## What Quiet holds open

- **Where purlis owns the timer, Quiet holds the ask open.** That is the sandbox host consent: its
  held connection waits on past its minute while the person is Quiet, at most 15 minutes in all
  (`sandbox::asks::MOST_HELD_OPEN`). As Quiet ends, one past its minute times out at once and
  stays answerable as above. The program's own connect timeout and its harness's command timeout
  still bound it.
- **The session host is told** with a new reach change, `hold_asks_open` on `change_reach`
  (protocol 1.83, `KNOWS_HOLD_ASKS_OPEN`), sent to each chat's confinement as Quiet moves, and to
  a chat's as it starts while Quiet. The window's own proxy takes the same call
  (`sandbox::Board::hold_open`). A host before 1.83 is not sent it, and its connections time out
  at their minute as before.
- **While it is held open, it is not Expiring**: its row shows no countdown, though its hold is
  still said, so the window reads it again.
- **Where the harness owns the timer, Quiet cannot hold it**, and the ask falls back to the
  timed-out path above: a permission prompt (the harness's hook kill), a run grant and a Secret
  ask (the harness's command timeout). A confirmation, a login, a fill and a pool tool are not
  held open either: each is ended by a command that gives up first.
