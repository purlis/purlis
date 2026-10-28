# Router choice, extension routers and the routing record, as #3230 built them

**Amends [ADR 0102](../0102-a-skill-router-decides-what-a-chat-sees-and-nothing-it-hides-is-out-of-reach.md), 2026-10-28**, ticket #3230 (SK-15b). ADR 0102 §5 and §6 say that a workspace or a persona chooses its router, that another router is an extension that declares SkillRouter, that it has 100 ms, and that what came of it is a routing record. This records the keys, the question and its answer, what purlis takes of an answer, and what the record holds. The pointer on a prompt (§3, #3231) and grading routers (§8, #3233) are follow-ups.

## Which router

- A workspace's: `settings.skills.router` in its `workspace.json`, one string.
- A persona's: `skill-router:` in its own definition, not inherited along `extends`. It wins over the workspace's, in every workspace.
- Either is `built-in` (the default, as is no key) or the id of an extension. A value that is neither is read as the built-in router, and the chat's routing record says so (`unchosen`). The Workspace settings layer refuses to save a router that is not a string.

## When it is asked, and what it is handed

**Once, at the chat's start**, where the plane's skills reach the chat and the router chosen is an extension's: by the window's start and by a start the session host composes, each with the executor of its own process. Never for a Try it's scratch chat, a chat on a runner (its host routes it there), or a chat routed by the built-in router, which is asked nothing and writes no record.

The question is protocol 3's studio question `route`:

```json
{"ask": "route",
 "skills": [{"name": "deploy", "description": "…", "pinned": false}],
 "budget": {"skills": 30, "chars": 6000},
 "besides": {"skills": 4, "chars": 610}}
```

- `skills` are **the skills the built-in router may hide** (ADR 0102 as amended 2026-10-25: plane skills handed by a listing route, not manual-only), each with whether the chat pins it. A built-in, harness, repo or manual-only skill is not one of them.
- `besides` is what the rest of the listing spends, which no router hides.
- **Nothing else of the chat**: no prompt, no chat id, no prompt salt (SK-38's `PromptSalt`), no workspace or persona name, no path.

The answer is `{"hidden": [{"skill": "<name>", "why": "<one line>"}]}`: the skills to hide, each with its own line. A router answers what it hides, not what it shows, so an answer that names nothing shows everything.

## What purlis takes, and what it does not

**The router has 100 ms** (`extension::studio::ROUTER_DEADLINE`), counted from before its executor slot is held, so its wait and its confined program's start are inside it. The executor stops its program at that deadline, and the chat's start stops waiting at it too, on a thread of its own: chatting never waits on a router.

An answer is not taken, and the chat is routed by the built-in router instead, where:

- it came past the deadline (`slow`);
- the extension could not be asked, is not approved or on in the project, does not declare `skill-router`, or answered an `error` (`failed`);
- it is not the shape above, names more skills than it was asked about, names a skill that is not one of its candidates, hides a pin, names one twice, gives a line that is empty, longer than 120 characters, on more than one line or holding a character a reader would not see, or leaves the listing over the budget while it could still have hidden more (`wrong`).

So **a router never adds a skill the set leaves out, and never hides a pin**: its answer only hides candidates, and one that tries more is not taken. One that hides all it may and is still over the budget is taken, and the budget says which part is over, as for the built-in router.

**What it hid is a skill's reason of its own**, `HiddenWhy::ByRouter(Line)`, never a free string: "hidden by the router relevance: not relevant to this workspace". The index skill names each hidden skill, as the built-in router's does.

**The answer is kept with the chat's start** (`hoststart::ChatSkills::routed`), and every later routing of its set applies it again: the folder its harness is handed, its arming, Use skill…, its restart stamp. Where its set changed since, a skill pinned or gone since is listed or gone as the set says and the rest of the answer holds; only where what is left leaves the list over the budget does that routing fall back on the built-in router.

## The routing record

`skill.routed` in the chat's event log, under the run its start began (after its `run.started`, and never for a start taken back): its ULID (`routing`), the extension (`router`), how long the question took (`ms`), `fell_back` (`slow`, `failed`, `wrong`, `unchosen`, or null) with `error`, and every skill the routing hid with its `why` (the router's line, or `over-budget` for the built-in router's fallback). It names skills and nothing of a prompt or a description. **Every `skill.started` of that chat's run names it** as `routing`, with `routing_fell_back` where it fell back, and so does every run a `/clear` of the chat begins, which keeps its listing.

The chat's Use skill… says the router and its timing, or why it fell back. The Skill studio, filtered to a workspace or persona that chose an extension's router, says so, and lists how it did at the start of each such chat open in the window.

## The approval prompt

`contributes.skill-router` is `{"title": "…", "reads-prompts": true}`, `reads-prompts` false where absent. The prompt lists the router, that it is asked at each chat's start within 100 ms, and either that it reads every prompt of every chat that chooses it, or that it never reads a prompt.

## What this does not do

- **A reattached chat asks its router again** where its window was restarted: what its start answered is not kept in the reopen record. #3578 keeps it, and amends this in [A reattached chat keeps its start's routing](2026-10-28-a-reattached-chat-keeps-its-start-s-routing.md).
- **The studio's rows show what the built-in router would hide**, per harness, as before: an extension's answer is per chat, and the studio says that.

**Built by #3230.**
