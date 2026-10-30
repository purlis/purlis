# Amended (2026-10-30): a sandboxed sender is pointed to the tools only where they queue, and what a sender that never came back queued is reported

Follows the amendment of the same day on queued messages (#3902). Implements #3914 and #3958.

## A sandboxed command still cannot queue (#3914)

Decided by the dispatcher on 2026-10-30, keeping the sandbox boundary: **no sandbox write grant
is added for the message queue.** In a sandboxed chat, purlis's `dispatch_*` tools queue, because
the harness runs purlis's MCP server outside the sandbox it puts a command in, and the briefings
teach the tools first. The `purlis dispatch` command keeps saying nothing was queued.

The harness check left open by that ruling:

- **Claude Code** runs purlis's MCP server outside its sandbox: its tools queue.
- **Codex and opencode** run whole inside purlis's wrap, MCP servers included
  (`HarnessAdapter::sandbox_holds_what_it_starts`), so their tools are kept from the queue as
  the command is. They cannot queue through their own MCP path.

So the refusal is said per harness (`dispatchqueue::tool_path`): the command points to the
tool only in a chat whose harness's tools queue; the tool itself, and the command under Codex or
opencode, say to send it again once the app answers. The `dispatch_tell` tool says it queues
unless the chat's sandbox holds purlis's tools too.

## A sender that never came back (#3958)

When a project opens without a chat that had messages queued, the spool clears its queue as it
drops its keys (`forget_all_but`). Before #3958 it did so with a log line only.

- **Set aside first.** Before the keys are dropped, every message in that chat's queue that
  still checks under them, and was not delivered, is written to one report in
  `spool/messages/left/`, under the same integrity denial, with whom each was for, read from the
  message's own file. What does not check is no message of the chat's, and is removed as before.
- **Kept on the dispatch it was between.** Once the project's chats and tasks are back, the app
  keeps each message, marked *not delivered*, on the running dispatch record it was between, by
  the numbers the chats had when it was queued: a follow-up or answer on the record of the task
  it names that the sender asked for, a note or question on the sender's own record as a task,
  a follow-up to all on each running record the sender asked for.
- **Otherwise, one Notice in the Inbox.** Messages no running record can keep stay as one Notice
  a sender, naming it and how many, with what each said, until the person puts it away.

Nothing a chat queued is cleared without a record the person can see.
