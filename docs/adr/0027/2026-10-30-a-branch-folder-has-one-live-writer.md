# Amended 2026-10-30: a branch folder has one live writer (#3901)

**purlis's piece claim now names a branch folder's one live writer.** The decision that purlis
records nothing of its own about a worktree still holds for what git can answer: which folders
exist, what branch each is on, whether it is dirty. Who works in a folder is not something git
can answer. The piece log has always recorded it (`claimed`, `done`, `abandoned`), and it now
also decides whether a brokered write goes ahead.

Why: after an app restart, a coordinator took a live task for a dead one and dispatched a resume
chat (#3897). The two chats rebased, committed and pushed in one branch folder before either
noticed.

The rule (`purlis_core::piececlaim`):

- A `claimed` line may carry `chat`, the chat's stable id (ADR 0066). A chat's number is dealt
  again across launches, so a claim names its holder by the id.
- `purlis worktree commit`, `update` and `push`, brokered by the app, are checked first. A chat
  that writes in a folder no live chat holds takes it, and its claim is logged. A second live
  chat is refused. The refusal names the holder and how to hand the folder over: the holder's
  `done` or `abandon`, or the person's.
- A claim whose chat has ended is released. That means the chat is not open in the app, or its
  program has ended. The holder's own `done` or `abandoned` also releases it, and so does the
  person's. One from another chat releases nothing.
- **A write fails closed when the holder's liveness can't be read.** That covers a holder still
  restoring after a restart or waiting to start again, and a chat whose app did not answer. A
  claim that can't be logged is also a refusal.
- A folder claimed by the window, by the person or by a line with no `chat` has no chat holding
  it.

**The check and the claim are made under one lock, so two chats writing at once can't both take
a free folder.**

What this costs:

- **This prevents accidents. It isn't a security boundary.** The piece log lives in the
  workspace, which chats may write. The guard keeps two chats from writing one folder by
  accident, which is what happened. It doesn't stop a chat that sets out to get around it.
- **Plain git is outside it.** A chat that runs without the sandbox commits with git, which purlis
  does not broker, so the claim is not checked there.
