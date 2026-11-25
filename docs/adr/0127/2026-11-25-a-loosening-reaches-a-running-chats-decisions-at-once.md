# Amendment, 2026-11-25: a loosening reaches a running chat's decisions at once (M75 slice 23, #7032)

Spec §9.5 said a running chat takes a tightening at its next decision and a loosening only at its
next start after the press, and called that proposed. Slice 23 built the chat's side and found the
host does not hold to the second half. The person ruled on 2026-11-25 (review round 1, H1): amend
the text to match what the host does. Nothing is built for this; no chat's policy is pinned.

**The ruling.** Hook and permission decisions always use the current policy. A tightening applies
at the chat's next check, with nothing to press. A loosening reaches the chat at its next check
too, once the person has pressed Accept (the accepted copy only moves at the press). Only the
shell sandbox, the OS-level confines compiled when the chat started, waits for a restart.

**Spec §9.5 now reads:** "A running chat's hook and permission decisions use the policy in force at
each ask, so a tightening applies at once and a loosening applies at the next ask after the
press. The OS sandbox compiled at start can't change mid-chat: a Never, a removed grant, a forbid
or a narrowed permit on a path, a new workspace, the sealed answers' deny state or a purlis
restart marks the chat **out of date** (in the sense **Plane updated** gives the word, as an older
shell sandbox) and offers a restart. A tightening is sealed at each decision that sees it. A
loosening never marks a chat, since it leaves the sandbox as strict as it was."

**The reason.** A decision reads the sealed store and the policy in force afresh each time it is
asked (`force::in_force`: the stricter of the committed project and the accepted copy), so there
is no copy of the policy held by a chat that could be older than the person's. The sandbox is the
one thing compiled once (`sandbox_rules::compile`), and the OS cannot change it under a running
program. Pinning each chat to the policy of its start would add state to every decision and make
a tightening wait for a restart, which is worse; so it is not built.

**What the window marks.** A chat whose shell sandbox is older than a tightened policy: the mark
says **older shell sandbox**, not "project updated", its tooltip names which change, and its pane
offers Restart chat. A loosening, a forbid-free commit and a press that loosens mark nothing.

The spec's working copy lives outside this repository (the map's `m75-policy` ticket), so its
§9.5 is amended there with the text above; this file is the record that ships.
