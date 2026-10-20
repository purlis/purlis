# A skill router decides what a chat sees, and nothing it hides is out of reach

**Accepted 2026-10-20**, from the operator's spec #2762 (M65, Skill studio), whose decisions were
grilled with the operator on 2026-10-20 (decisions 19, 53, 54, 56, 62 and 64); ticket #2778
(SK-15). It builds on ADR 0099 (#2829: a chat's skill set, and a reason for every skill left out
of it), ADR 0101 (the SkillRouter capability) and the fallback route's amendment to
[ADR 0063](0063-every-harness-a-chat-runs-on-is-handed-charters-skills-by-its-own-route.md)
(`skill_find` and `skill_load`).

**Built by #2778**, on #2768 (persona skills), #2776 (the MCP skill tools) and #2777 (the
capabilities).

## Where purlis was

Every skill a chat could get had its name and description put in the chat's context, and each
harness copes with a large library in its own way without telling the person. Measured on the
versions installed on 2026-10-20: Claude Code keeps its listing to 1% of the context window by
dropping descriptions, Codex keeps it to 2% and then leaves trailing skills out of the listing
silently, and opencode has no limit and puts every skill in the system prompt.

So a growing library either floods the context or loses skills without saying so, and a harness
chooses less well among hundreds of descriptions than among a few.

## The decision

### 1. A router decides what a chat sees

A **skill router** takes a chat's candidate skills (its skill set, ADR 0099), the chat's skill
budget, its pinned skills and, where it has one, the prompt. It returns which skills are visible
and, for every skill it hides, a one-line reason. It never adds a skill to the set: a skill the
set leaves out (not granted, turned off, archived) stays out, whatever the router says. A router
only orders and hides what the chat may already have.

**A router that hides any skill shows the index skill** (§2) with whatever else it shows, and the
index counts against the budget. So every hidden skill is named in the chat's listing, and the
fallback (§6) always has an index to fall back to. A router that hides nothing needs no index.

**The budget is purlis's**: the chat's skill budget for its harness, in skills and description
characters (#2775), shown on the chat tab. It is set below each harness's own limit, so the
harness never has to cut anything itself. On opencode, which has no limit, it is the only one.

### 2. The built-in router: everything below the budget, an index above it

The **built-in router** needs no prompt. **Below the budget it shows every skill**, as purlis does
today. **Above the budget it shows the pinned skills and one index skill**: an always-loaded
skill that lists every other skill by name and one line, and says to load one with `skill_load`.
A small library stays exactly as simple as it is now, and a large one costs one short listing.

Manual-only skills are never in the listing or the index's descriptions, as now. A deprecated
skill is listed after the rest and names its replacement. An archived skill never loads.

### 3. When a router runs

A harness takes its skill listing when a chat starts and does not reload it during the chat
through purlis's routes (ADR 0099 §5). So a router acts at two moments:

- **At chat start**, it chooses the listing the harness is handed by its route. This is all the
  built-in router does.
- **On each prompt**, a router that reads prompts (a retrieval router) may name skills that suit
  this prompt and are not in the listing. purlis adds them to the prompt's hook context as a
  short pointer: the skill's name, one line, and that `skill_load` loads it. The listing itself
  does not change mid-chat.

### 4. Pins come first, and are never hidden

A persona or a workspace can **pin** skills as always loaded. A chat's pins are its persona's and
its workspace's together. **Pinned skills count first against the budget, and no router hides
them.** When the pins alone are over the budget, they are all shown and the chat's budget
warning says the pins are why.

### 5. A persona or a workspace chooses its router

A workspace chooses the router its chats use, and a persona may choose one for its own chats,
which wins over the workspace's, the narrower level winning as it does for skills (ADR 0099 §3).
The default is the built-in router. Other routers are extensions that declare SkillRouter
(ADR 0101), which are confined; a router that reads prompts says so in its approval prompt,
because it reads every prompt of every chat that uses it.

### 6. A router has 100 ms

A router gets **100 ms per prompt**. Past that, purlis does not wait: that prompt goes on with the
listing the chat already has (the index, where the router hides anything) and no pointer.
**The fallback is marked in two records**: the prompt's **routing record** in the chat's event log
(the router, its timing, what it hid and that it fell back), and every skill run opened during
that turn, which names the routing record it started under. The chat tab and the studio
show the router's timings and its fallbacks. A router that fails or answers wrongly is treated
the same way, with its error in the record. Chatting never waits on a router.

The same budget holds at chat start. Past it, the chat starts with what the built-in router
would show.

### 7. Every hidden skill says why

A skill a router hides has a reason in the chat's skill set, as every skill left out does: "hidden
by the router" with the router's name and its own one line, such as "over the budget; in the
index" or "not relevant to this prompt". The studio shows, for any chat, what the router hid and
why, and nothing it hid is out of reach: every hidden skill can be found with `skill_find`, loaded
with `skill_load`, or started from Use skill….

### 8. Trigger tests grade routers

A router is graded with the library's **trigger tests** (ADR 0101 §3): each trigger test is run
with the router in place, and passes when the skill that should trigger is started (by the
harness's own pick or through `skill_load`) and fails when one that should not is. The baseline is
the built-in router on the same tests. A router's grade is a property of the library it was
graded on, recorded with that library's commit.

## What was rejected

- **Leaving it to each harness.** Codex drops skills silently, opencode floods its system
  prompt, and the three disagree.
- **Changing the listing on every prompt.** No harness reloads its listing mid-chat through
  purlis's routes, and a listing that moved under the model would break its prompt cache.
- **Hiding pinned skills when the pins are over the budget.** A pin is the person's statement
  that the skill matters; the warning is the answer.
- **Waiting for a slow router.** The person feels every millisecond on the prompt; the index is
  a good enough answer.
- **A router that can add a skill the set leaves out.** Grants, turning off and archiving would
  stop meaning anything.
- **Codex's own experimental skill search as purlis's router.** It is Codex only, and purlis
  hands Codex a listing within its budget, so Codex's own trimming never fires.

## Consequences

- A small library behaves as it does today. A large one stays reachable on every harness, and
  the person can see what was hidden.
- A retrieval router's 100 ms includes its confined process start (ADR 0101), so its cost is
  measured as part of #2778. A router that cannot meet the budget falls back on every prompt,
  and the studio's timings show it.
- The pointer on a prompt depends on the harness's prompt hook. A harness without one gets only
  the listing and `skill_find`, and its compatibility badge says so.
- A skill reached through the index or a pointer is loaded through `skill_load`, so its run has an
  exact start on every harness.
- Router choice and pins are persona and workspace settings, and are documented in
  `docs/plane-format.md` before code.
