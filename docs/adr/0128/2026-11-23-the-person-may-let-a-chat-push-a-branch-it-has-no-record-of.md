# Amendment 2026-11-23: the person may let a chat push a branch it has no record of

Amends [ADR 0128](../0128-your-sign-in-brokered-one-approach-for-every-forge.md), D8 (what the
route lets a chat write) and the created-branch record of #7363, after the amendment that lets a
refused push say why (`2026-11-23-a-refused-push-says-why-and-the-asking-chat-follows-up-its-tasks-branch.md`).

## What was wrong

The route lets a chat update a branch only from the tip it last put there (#7403). A branch that
already existed on the forge before the route did (pushed by the person, by a watcher, or by an
ended chat with no dispatch link) has no owner record, so every chat's update was refused, and
the person pushed it by hand from a terminal. The refusal now says why (the earlier amendment),
but the only way out was still outside purlis.

## Decision

An update to an existing branch that the route refuses **for having no owner, or another owner**,
raises one ask in the window: **Let this chat push it**. Its answers are **Allow** and **Keep
blocked**.

- **Allow** records this chat, in the project, as the branch's owner at **the commit the forge
  shows the person when they press**, read as the person (the held acts' forge, the account they
  chose for the repo), never the old id the chat named. It is one ordinary push line in the
  created-branch record (`docs/plane-format.md`): no new kind of line, no new field. From then on
  the tip rule (#7403) applies as to any branch the chat made, and the chat tries the push again.
  A branch that is gone from the forge, or a repo whose id is not the one the ask was raised for,
  records nothing.
- **Keep blocked** records nothing. The ask goes, and the chat is not asked again for that branch
  while it lives (in memory, for the chat's life); git's output says the person kept it blocked.
- **Only the person's press records an owner** (D5): `windowonly::let_chat_push` takes the
  person's `HostScope`, which no chat's process tree can claim, and both commands are the
  window's alone (`allow_chat_push`, `keep_chat_push_blocked`: never served on a link). The press
  names only an ask this app raised, in this project, for this session: a control that was never
  raised, was answered, or belongs to another project grants nothing, and the project in the
  record is the pressing window's own. The CLI and a chat have no way to write the line.
- **Only a branch the repo lists, and a name that can be said.** The route raises the ask only for
  a branch in the repo's advertisement, read when the chat fetched; any other name is invented,
  raises nothing and is refused as before. A name is also checked as a base branch is (1 to 255
  bytes, no space or control character, not starting with `-`) and must be printable ASCII.
- **A cap, so a crafted push cannot flood the window.** A push raises **at most one** ask (the
  first branch that qualifies), and a chat has **at most three** waiting at once. Past either, no
  new ask is raised and git's output says asks are already waiting; answering one makes room.
- **The ask says enough to decide.** It names the branch, the repo and the host; says whether
  nobody owns the branch in purlis's record or another chat does (naming it); and says that after
  Allow the chat can replace the branch's history, not only add to it. The branch's tip is read
  only at the press, never when the ask is raised.
- **One ask per chat and branch**, however often the chat is refused. Under `[policy] consents`
  set to `log` or `off` no ask is raised, as for every window-only refusal (Q308b), and the
  refusal is as it was.
- **Never for a protected branch.** The default branch, a protected, deploy, release, production
  or pages branch, a delete, a tag, a ref that is not a branch, a branch the chat owns at another
  tip (someone moved it since: the chat fetches, or the person pushes) and a push with any other
  refusal in it raise no ask and are refused as before. The route reads the branches from the
  push's own commands, never from anything the chat says.
- **A distinct refusal.** The policy refuses such an update as `NotYourBranch`, ranked below every
  other refusal so that it is only ever a push's refusal when everything else in it is fine. Git's
  output for it says an ask is waiting in the window where one was raised, and otherwise says to
  ask the person to push it.

## Tier

No store is added and the record's format is unchanged; `docs/plane-format.md`'s row for
`forge-created-branches.jsonl` names the second writer. The ask and the kept-blocked answer are
memory only.
