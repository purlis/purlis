# Amendment 2026-11-23: a refused push says why, and the asking chat follows up its task's branch

Amends [ADR 0128](../0128-your-sign-in-brokered-one-approach-for-every-forge.md), D8 (what the
route lets a chat write) and the created-branch record of #7363. Two fixes from one live report.

## 1. A refused push is answered in git's own words

### What was wrong

The route answered every refusal `403`. For a push, git printed `RPC failed; HTTP 403`, `the remote
end hung up unexpectedly` and `Everything up-to-date`, which reads as a permission fault at the
forge. The reason (a protected ref, a branch that is not this chat's, a stale tip) was in the
audit and nowhere the person looks.

### Decision

The listener answers a refused git request the standard way, so git shows the reason
(`listener/gitsay.rs`):

- A refused `POST …/git-receive-pack` is `200` with a well-formed report-status:
  `unpack ok` and `ng <ref> <reason>` for each ref the push named, in the side-band the client
  asked for (channel 1 when its first command carries `side-band` or `side-band-64k`, bare
  pkt-lines otherwise). Git prints `! [remote rejected] <branch> (<reason>)`. A report-status was
  chosen over a channel-3 message because git renders it per ref, and exits non-zero on it.
- A refused `GET …/info/refs?service=…` is `200` with the service banner and one `ERR <reason>`
  pkt-line, which git prints as `fatal: remote error: <reason>`.

**Fail closed, unchanged.** This runs only after the core refused, and forwards nothing. A reason is
purlis's own sentence for the refusal kind, or the sentence a pin or a held act already tells the
chat (which may name the branch, tag or repo, and a forge's own error): never a token, the sentinel or a path. It is one line, cut at 200
bytes, control characters made spaces. Every ref of a refused push is rejected (the push is
refused whole), each with the push's reason. A push whose commands do not read, a request with no
credential (still `401`, so git asks its helper) and every other request stay as they were.

## 2. A chat may push a branch a task it dispatched made

### What was wrong

The created-branch record is keyed by project and chat. A branch a dispatched task made is the
task's, so once the task ended the chat that asked for the work could not push its follow-up: the
person had to.

### Decision

An update is also let through when the branch's recorded owner is a chat of the same project that
the pusher **dispatched as a task, directly or up the chain** (at most eight links), from the same
tip rule: the old id is the recorded tip (#7403).

- **The link is app-owned.** It is read from the dispatch records the app writes
  (`dispatchrecord::dispatched_by`, matching chats by ULID), which no chat can read or write. Never
  a session record's frontmatter, never a file a chat writes. No such record means refused.
- **A handoff is not a link**: the work moved, and the asker gave it up.
- **Recording.** The confirmed push is recorded under the pusher, as any push is, so its line is
  the last and the task is no longer the owner. The task and the dispatcher cannot both keep moving
  the branch: whoever pushed last moves it, and only from the tip they left. Keeping the original
  owner and adding a tip would have let both move it.
- **One way.** A task never moves its dispatcher's branch: the walk goes from the pusher's
  dispatched chats downward only. Nothing asked for the other direction, and a task's reach is the
  smaller.
- A dispatch record is collected 30 days after it ended, and the link goes with it: after that the
  person pushes, as before.

Tier: no store is added; the record's format is unchanged, and `docs/plane-format.md`'s row for
`forge-created-branches.jsonl` says the new read rule.
