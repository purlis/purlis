# A persona names another workspace's skill by its workspace

**Amends [ADR 0099](../0099-a-skill-lives-at-a-level-a-project-skill-reaches-a-workspace-by-grant-and-the-narrowest-copy-wins.md), 2026-11-10**, ticket #2933 (SK-5a). The amendment of 2026-10-21 said a bare `<name>` on a persona's `skills:` line resolves in the chat's own workspace, and that naming a workspace skill by its workspace waited on a real need. A persona dispatched from `ops` into `smart-ide` could not bring `ops`'s own skill. This records how it names one, which copy wins, and what still holds it back.

## The entry is `workspaces/<ws>:<name>`

- **`workspaces/<ws>:<name>` names the workspace skill `<name>` of the workspace `<ws>`** (`purlis_skills::Reference::Workspace`). The part before the `:` is the workspace's folder in the plane, as `docs/plane-format.md` spells it. The part after it is the skill's `name`, as every other entry has it. `<ws>` is one plain name, as a workspace's folder is. An entry with any other part there (`workspaces/:x`, `workspaces/a/b:x`, `workspaces/..:x`) names nothing, and the lint says so.
- **Why this spelling.** It is the issue's own example. It reads as the path a person sees in the plane. It cannot be a plugin's name, since no plugin's name holds a `/`. A bare `ops/deploy` was the other candidate. It would be the first entry with no `:`, and a skill's own name could one day hold a `/`. So `workspaces/<ws>:<name>` keeps the one rule that what comes before the `:` says where the skill comes from.
- **Only the skill named comes along.** The rest of `<ws>`'s skills stay that workspace's. Each copy is read as `<ws>`'s own chats read it: its local copy first, while git keeps it on this machine, then its shared copy. A folder made while a sandboxed chat ran waits for its Accept (SK-9).

## Which copy wins

Narrowest wins still holds, and the order is: **the persona's own copy, then the copy it names by its workspace, then the chat's own workspace's, then the project's.** The issue left the middle step open. The copy the persona named by its workspace beats the chat's own workspace's for these reasons:

- A bare `<name>` already means "whatever copy this workspace has". A persona that writes `workspaces/ops:deploy` has said which copy it means. If a `deploy` in the chat's workspace won, the persona's explicit choice would silently become a different skill, depending on where it was dispatched. That is the thing #2933 asks to stop.
- The named copy ranks with the persona's choices. Like a project skill the persona names, it reaches every workspace whatever was granted (as amended 2026-10-21). It sits just below the persona's own folder, which is the persona's choice too and the narrowest.

A chat's own workspace copy that the named copy hides is left out with the same line any hidden copy gets. Where the chat runs in `<ws>` itself, the entry names that workspace's own level, and nothing changes.

## What still holds it back

- **No other chat gets it.** The skill reaches only chats as the persona that names it. A chat in another workspace on no persona, or on another persona, gets none of `<ws>`'s skills, as before. A workspace's `settings.skills.off` reaches only project skills, so it does not touch this one.
- **Writing it stays refused.** Every level's `skills/` and `skills.local/` is already denied to a sandboxed chat (`sandbox::skills_held`), in every workspace. The chat reads the skill where it lies, as it reads its own workspace's. No copy is made.
- **One that is not there is left out and said.** A workspace the project does not have is left out: "its persona names it, and the project has no workspace of that name". So is a workspace with no skill of that name: "its persona names it, and that workspace has no skill of this name". `persona lint` warns of each with its fix (`skills::persona::unresolved`). Neither depends on the machine, so each is checked with nowhere to look for a harness's skills too.

## Where it shows

The Skill studio as the persona, and `purlis skill list`, list the copy with its level and the workspace's name (`workspace ops`, `StudioRow::level_label`). It is written as the persona's line writes it (`workspaces/ops:deploy`), and Remove takes it out. Naming one is done on the persona's line: another workspace's skills have no row in the studio as the persona until one is named, so there is none to press Add on. `skills::persona::change` takes the entry as written. Three things are not built yet:

- A missing one is not on the chat's tab's Notice.
- A run of the skill is not told apart from a copy of the same name in the chat's own workspace.
- The studio's Add does not offer another workspace's skills.

Both are follow-ups, named in the pull request.
