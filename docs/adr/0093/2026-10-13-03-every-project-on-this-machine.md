# Amended 2026-10-13 (operator ruling R2): every project on this machine

> **R2.** Where something stays a per-machine grant (sockets, other services), one press can
> cover this machine for every project; per-project remains under "Other scopes".

This adds a level to D-sock-2's two (one chat in memory; this project on this machine):

- **Every project on this machine**, for a local socket or a system service only. What a
  machine has (a container engine, an agent) is the same whichever project asks for it, and a
  person who allowed Docker's socket in one project was asked again in the next. Kept in
  `~/.config/purlis/sandbox-grants/every-project.json`, beside the files kept per clone: this
  machine's own, which no sandboxed chat writes and no repository brings. **Never a host or a
  folder**, which stay a project's, and never a dispatch grant.
- **Where it is offered.** A block's Notice for a socket or a service draws two answers side by
  side: **Allow once**, still the main button and the narrowest, and **Allow on this machine,
  for every project**. Only this chat, and this project alone, are under Other scopes. (Decided
  in implementation and flagged: the widest answer is one press away and named for what it is,
  and is not the default button, because a container engine's socket is about the same as no
  sandbox, for a project opened later too.)
- **It is judged like any grant**: by the socket's resolved path when offered and when allowed,
  and again at every start **for the project the chat starts in**, so a socket inside that
  project's own tree reaches no chat there. Audited at this level; listed in every project's
  Allowed list as "Every project on this machine", with Remove, which takes it from them all.
- **Policy.** `socket-grants: false` forbids it and takes it away, as it does a project's. A
  policy that fixes `allow-scopes` leaves this level out unless it names it: `you` does not
  stand in for it. (Its word, `machine`, was ruled on later the same day: see "the policy's
  word for every project" below.)
