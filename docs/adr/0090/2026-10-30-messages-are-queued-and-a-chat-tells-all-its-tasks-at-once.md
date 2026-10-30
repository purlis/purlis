# Amended (2026-10-30): messages are queued when the app does not answer, and a chat tells all its tasks at once

From #3897 (a multi-hour coordinator run), concerns 3 and 5. Ruled by the operator on
2026-10-29: `tell --all` with acknowledgement. Implemented in #3902 and #3904.

## Durable delivery (#3902)

A `tell`, `note`, `ask` or `answer` the app did not answer within its two seconds was lost: the
sender was told "the purlis app did not answer, so nothing was sent", and a coordinator had to
notice and send it again.

- **Queued first, then delivered.** The command and its tool write the message to the sender's
  queue (`spool/messages/<chat>/`, `purlis_core::dispatchqueue`) and sync it before they answer,
  then ask the app to deliver what the chat queued (`What::Deliver`). The app delivers each
  message as a live one is delivered, held to the same rules (`dispatched::owned`,
  `dispatchtalk::up`, the pair's limit a minute), and removes it.
- **When the app does not answer**, the sender is told the message is *queued, not yet
  delivered*. The app delivers it when it next hears from that chat (any hook line, or any
  ask), when a chat waits on it (the asking chat's `wait` on the task, or the task's wait for
  the answer to its question), and when it opens the project, once the tasks are back. Nobody
  has to act for it. A message sent live after one that was queued is delivered after it.
- **Once and in order.** Each chat's messages are numbered, and one drain of a chat at a time in
  the app removes each as it delivers it. The last number delivered is kept with the chat's key
  (`keys.json`), so a number never goes back and a copy of a delivered message is refused after
  its folder drains. A limit that holds a queued message back holds the messages after it too,
  until the next drain; a `tell --all` waits while any of its tasks' pairs is at its limit.
- **A chat that closes with messages queued** has them delivered as it closes, held back by no
  limit. What cannot be delivered then is kept on the dispatch record of the task it is between,
  with why, where the person reads it in the Dispatches tab; the spool then clears the chat's
  folder.
- **Undeliverable is said.** A queued message the app cannot deliver (its task has finished, it
  is not the sender's task, it does not check) is removed, and `purlis dispatch list` and
  `read` say it was undeliverable, and why. They also list what is still queued and what was delivered.
- **Who may queue, and the boundary that stays.** The queue is in the hook spool's directory and
  under its integrity denial (ADR 0067 §5): a sandboxed chat neither reads nor writes it, so no
  chat can remove or reorder another's messages. Each message is signed with the key the chat's
  spool lines are signed with, derived from its token, and checked against the key the app
  issued that chat; a message that does not check is removed and never delivered. purlis's MCP
  server writes the queue, as a harness runs it outside its sandbox. **A command a sandbox
  confines cannot write it**, and no sandbox grant was added for it: there, a message the app
  does not answer is not queued, and the command says so and points to the tool, which queues
  it. A harness that wraps its MCP server in purlis's sandbox (Codex, opencode) cannot queue
  either, as its hooks cannot spool.
- **Survives a restart of either side.** The queue is on disk and checked under a key the app
  keeps until the chat ends. What was delivered and acknowledged, and each chat's last
  instruction number, are kept on disk beside the queues (`spool/messages/records.json`, under
  the same denial), by each chat's id, which it keeps across a restart (ADR 0066).

## Tell all, and acknowledgement (#3904)

- **`purlis dispatch tell --all "<text>"`**, and the `dispatch_tell` tool's `all` field: one
  follow-up to every task of the asking chat that is still working (not finished, not one the
  person started from its tab), each held to what a `tell` is held to. Queued like any `tell`;
  queued, it reaches the tasks still working when the app delivers it.
- **Every follow-up is an instruction with a number**, minted by the app as it delivers it:
  each asking chat's numbers go up and never back, across a restart too. The task is told the
  number and how to acknowledge it. An acknowledgement matches only the instruction of that
  number its asking chat really sent that task.
- **`purlis dispatch note --ack <n>`**, and the `dispatch_note` tool's `ack` field: the task
  acknowledges instruction `n` of its asking chat's. The text is optional with `--ack`; with
  text, the note is sent as well. An acknowledgement of an instruction its asking chat did not
  send it is refused.
- **`purlis dispatch list` says who acknowledged which instruction and who has not.**
- **Pre-allowed as the other dispatch words are**: `Bash(purlis dispatch tell *)` and
  `Bash(purlis dispatch note *)` already match the new spellings, and the tools are already
  allowed by name (`chattools::DISPATCH_TOOLS`). Neither starts a chat or reaches beyond what a
  `tell` or a `note` reaches.
