# Amended (2026-11-06, #5357; the operator's ruling of 2026-11-05): a chat proposes a change to several skills, and only the person makes it

Ruled by the operator: a chat can only propose a bulk change to skills, and the person approves it
in the same review as other proposed changes (#1975). A prompt injection in a skill or a web page
must never be able to bulk delete or bulk update on its own. And, on 2026-11-06, the person's
answer to how a chat that detaches is told apart: a bulk change runs directly only from an
interactive terminal, with no flag to skip its question.

- **The verbs.** `purlis skill rm`, `off`, `on`, `pin` and `archive` take several names, or
  `--source <url>` for every copy installed from a source, as the window's Select mode and a
  source's page do (#5331, #5332). At the person's interactive terminal the change is printed as
  its review, asked once, and made as one commit through `verbs::set`. No flag skips the
  question. A caller with no terminal (a script, a pipe, a detached process) proposes, as a chat
  does.
- **Who is a person.** The command makes a change only when it is positively at the person's
  terminal, and every other case, a doubt included, is a chat's. A change to several skills also
  needs standard input and standard error to be a terminal: a process detached from its chat has
  none. It is a chat's when any of these holds: an environment mark the app sets on everything it starts (the chat's number, its hook
  socket, its session, its sandbox's mark); the sandbox itself, which denies every sandboxed chat
  the project's manifest, whatever its environment says (`sandbox::held::in_a_sandbox`); or where
  the process runs, below a chat's program or in its session by the records of open chats, which
  secret setup already reads (`secrets::setup::in_a_chat`). A doubt in that last check is a chat's.
  None of these is a flag, so nothing a chat passes makes it the person. A chat that strips its
  environment is still known by the other two.
- **From a chat, a proposal and nothing else.** For any of those verbs, with one name or with
  several, the command writes nothing. It hands the app `Ask::ProposeSkillSet`, which names the
  skills and the act and carries nothing else. A chat that can't reach the app is refused. It
  never falls back to making the change. Every other writing `purlis skill` verb is refused in a
  chat, as before, now by the same check.
- **The same ask.** The app holds it with the other proposals, on the chat's tab and in the Inbox,
  with Apply and Dismiss. What is shown is read from the plane, never from the ask: each skill as it
  is and as the change leaves it, the skills its review keeps out, and those left naming one that
  goes. That state is fixed when it is held. A skill edited, deleted or turned since makes it stale
  (PC-8). Apply is audited first (PC-4) and makes it through `SetChange::apply`, the verbs the
  window's own review calls. A chat's newer proposal of the same name replaces its older one (PC-9), no "always" (PC-13), and
  `sandbox.proposed-changes = "off"` holds none (PC-14). The chat hears fixed words: applied,
  dismissed or stale.
- **The sandbox is the boundary, and the check is not.** A sandboxed chat is denied its skill
  folders and manifests whatever any command decides, and the sandbox is inherited by everything
  it starts, a detached process included. The check above makes the command propose rather than
  fail partway. **An unsandboxed chat can still change the files directly**, by deleting a skill's
  folder itself, so for it the CLI check is not a boundary: it stops `purlis skill` from making
  the change, and nothing more.
