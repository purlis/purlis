# The Skill studio's flag is gone

**Amends [ADR 0099](../0099-a-skill-lives-at-a-level-a-project-skill-reaches-a-workspace-by-grant-and-the-narrowest-copy-wins.md), 2026-11-04**, ticket #2786 (SK-23), the finish line of spec #2762. The `skill-studio` flag is removed, and with it Settings › You › Previews, whose one switch it was, and the `PURLIS_SKILL_STUDIO` variable. This changes no ruling. It says what each ruling that named the flag means now that the flag is gone.

## The 2026-10-22 ruling: the flag hides only the studio

[The 2026-10-22 amendment](2026-10-22-the-migration-from-claude-skills-as-built.md) ruled that the flag hides only the studio, and that a project which requires `project-skills` reaches every chat whatever the flag says. Both still hold. There is no longer anything to hide: the studio's surfaces (the view, `Use skill…`, `purlis skill`, Settings' summary row) are on for every machine, and the project's skills reach every chat of this build, moved or not.

`skills::reach_chats(plane, said)` is still the one question every chat's start, restart offer, missing-skill Notice and skill tool asks. `said` is now the word the chat's start took, which every start of this build gives as yes.

## #4349's runner rule, as amended 2026-10-31

[The 2026-10-31 amendment](2026-10-31-the-move-does-not-wait-on-the-flag-and-a-required-project-reaches-a-runner.md) is unchanged: a runner's chat is reached where its desktop said so, or where the project requires `project-skills` (`said || requires`). An unmigrated project keeps its desktop's `false`. That `false` can now come only from a desktop older than this build. A start that says nothing takes the host's own reading, as that amendment says, and the host now reads yes. The amendment's "this follows" is now done.

The variable that carries the start's word into the chat a runner composes, which was the flag's `PURLIS_SKILL_STUDIO`, is `PURLIS_PLANE_SKILLS`. Only its `0` means the plane's skills don't reach the chat; no other value turns anything on or off. A record's `plane_skills`, kept for a chat's reattach (#3466), stays as it was. Where a record has none, the reattach reads yes.

What that amendment listed as staying by the flag until #2786 is gone with it:

- On every machine, a moved project's chats have their skills and the studio's own surfaces.
- Accept for a workspace or persona folder that a sandboxed chat holds back is offered everywhere.
- A sandboxed chat is denied writing every level's skills folders (`sandbox::skills_held`) on every machine, a runner's among them.

## The other records that named the flag

- **[Draft skills and Try it](2026-10-22-draft-skills-and-try-it-as-built.md).** A chat's change to a skill's file is always kept as a draft, and a sandboxed chat is always held from the skills folders.
- **[Harness and repo skills](2026-10-21-harness-and-repo-skills-as-built.md).** Settings' Plugins group is always the summary row. Its flag-off controls are gone.
- **[opencode's built-in skills](2026-10-26-opencode-gets-the-built-in-skills-under-their-own-names.md).** These are laid out as they were. That amendment's "flag on or off" now means always.
- **The old `[harness_plugins]` keys.** Their Ask was the Inbox's only while the studio was off (#3689). It is now always an item of the studio's Needs you, and the Inbox no longer draws it.
- **`machine.json`'s `flags` table** (#2764). Nothing reads it now. An older build's `{"skill-studio": true}` is ignored and left out at the store's next write (`docs/plane-format.md`).
- **`purlis persona lint`.** It always checks a persona's `skills:` line. The warning that the line is read only with the preview on is gone.

## What was rejected

- **Keeping the variable as a way to turn the studio off.** A switch that hides the studio, or keeps the project's skills from a person's chats, is the flag under another name.
- **Dropping the desktop's word from a runner's start.** A runner may still be started by an older desktop. Its `false` on an unmigrated project is the ruling of 2026-10-31, and this change keeps it.
