# A reference is met where the skill's chats resolve it

**Amends [ADR 0100](../0100-a-skill-is-installed-from-a-git-source-as-a-pinned-copy-and-never-updates-by-itself.md), 2026-10-31**, tickets #4032, #4154, #4155, #4172 and #4186. §2 said "Skills a skill refers to come along": every reference the source holds is installed with the skill that refers to it. A copy of a skill the skill's chats get already is either refused (the level holds that name, or it is a built-in's name) or a second copy that shadows the one they had. This file records which references are met already, and so are not brought along.

## The rule

A skill installed at a level refers to `skills:<name>`. The reference is **met already**, and the source's `<name>` is not brought along, when every chat that gets the installed skill also gets a skill named `<name>` without the install. That is how `purlis_skills::skill_set` resolves a name for those chats:

- **A built-in skill's name** is always met. A built-in is in every chat, and no plane skill may take its name (spec #3679 S12). Before this, the source's copy came along and the install refused.
- **The level itself** meets it: a skill of that name at the level, from any source or made there by hand (#4032).
- **A workspace level** is met by a **project skill granted to that workspace** and not turned off there. A chat in the workspace gets that project skill. A copy in the workspace would shadow it.
- **A persona level** is met by a **project skill the persona's `skills:` list names**. A persona brings what it names into every workspace, whatever the workspace was granted (ADR 0099 as amended 2026-10-21).
- **Sides.** An install into a level's shared skills (`skills/`) is met only by shared skills. Those are on every machine the committed copy reaches, and a teammate does not have this machine's local skills. An install into a level's local skills (`skills.local/`) is met by local and shared skills alike: it runs only on this machine, where it has both.

- **A project install with a grant** (#4172) is met by the project's skill of that name, as the level itself. A second copy cannot be made at the same level. Its chats in a workspace get that skill only where its own grant reaches the workspace and the workspace has not turned it off. So the install **widens the held skill's grant** to each workspace the install's grant reaches and the held skill's does not: `root` and `ops` become `ops`, `web` and `ops` become `web, ops`, and `all` makes it `all`. The review says so before the install ("refers to notify, which is granted to root: it is left as it is, and the install grants it to ops too"), the install writes the widened grant in the same all-or-nothing change, and takes it back with the rest. A workspace that turned the held skill off keeps it off: no grant turns a skill back on. The review and the install name that workspace, whose chats get the installed skill without it.

- **An update at the project level** (#4186) relies on what it refers to as an install with the updated skill's own grant does. A held project skill its newer commit refers to is widened to where the updated skill reaches, in the same all-or-nothing change, and the update review says so ("refers to lint, which is granted to root: it is left as it is, and the update grants it to ops too"). A skill the update brings along lands with the updated skill's grant, and the review says that too. An update of a skill granted to the root alone, or one in the project's local skills, grants nothing.

Everything else still comes along: a project skill not granted to the workspace, or turned off there; a workspace's skill for a project install; and any skill a persona's chat may or may not get from the workspace it runs in.

## Rejected

- **Met wherever the plane holds the name.** A project install would then rely on a workspace's skill that no chat at the root gets, and a shared install on a local skill a teammate does not have.
- **Met only by the level itself** (the rule #4032 shipped). A workspace install then copied a project skill granted to it, and the copy shadowed the project's from then on, so the project's later changes stopped reaching that workspace.
- **The review returns where each referred skill lives, and the window decides.** The window would copy the core's resolution, as #4031 and #4032 already rejected for unticking and for the level.

- **A project install with a grant brings its own copy where the held skill does not reach.** The install refuses that copy, as the level holds the name, so nothing could be installed.
- **The review asks before widening each grant**, with a box per held skill. Declining would leave the installed skill's chats in those workspaces with a reference that resolves to nothing. The person who wants a narrower reach installs with a narrower grant, or narrows it after with `purlis skill grant`.

## What the review says

A reference met already is listed on the skill that makes it (`SkillReview.already`), as "It refers to notify, which its chats there have already: it is left as it is." The install reviews again by the same rule, so what was accepted is what is installed.
