# Draft skills, Try it and the throwaway piece, as #2772 built them

**Amends [ADR 0099](../0099-a-skill-lives-at-a-level-a-project-skill-reaches-a-workspace-by-grant-and-the-narrowest-copy-wins.md), 2026-10-22**, ticket #2772 (SK-9), spec #2762 decisions 13, 38, 50 and 52. The amendment of 2026-10-21 on changing a skill said a chat's change becomes a draft the person accepts in the studio. This records how.

## A chat never writes a skill's files

- **A plane skill's file is any file below a level's skills**: `skills/<folder>/…` or `skills.local/<folder>/…` at the project's root or in a workspace (`drafts::draftable`). Never a dot-name, a `package.json` or a `hooks` folder below the skill's folder, which the proposed-change rule (PC-1) keeps from `.claude/skills` too; never a workspace that is not there; never a path with a link in it.
- **In Claude Code**, sandboxed or not, the chat's `Write`, `Edit` or `MultiEdit` of one is read by its `pretooluse-edit` hook and handed to the app on the proposed change's road (`Ask::Propose`, #1975). The tool is refused with words that say the draft is in the Skill studio. **In another harness** the chat runs `purlis propose <file> --from <draft>`, which now takes a skill's file too.
- **The app keeps it as a draft** instead of holding it as a proposed change, where the `skill-studio` flag is on. Where the flag is off, or the app is older, the edit is what it was before.
- **A sandboxed chat is denied writing the skills folders themselves**, while the flag is on (`sandbox::skills_held`): `skills/` and `skills.local/` at the root, in each workspace and in each persona's folder. A skill runs its `!` commands when it is used and reaches every chat its level reaches, the reason `.claude/skills` is held (ADR 0067 §5). So a chat in any harness cannot plant a skill around the draft. A workspace or persona made after a chat started is not in its list.
- **A persona's skills folder is held and not drafted.** A persona level is no `Level` of the studio's changes yet, so a write there is refused by the sandbox alone. Drafting a persona's own skill waits for persona skills to be changed in the studio.

## A draft is kept on this machine, one a skill

- Decision 52 says a draft is "a local skill until it is accepted". It is kept on this machine, as a local skill is, but **not** in a level's `skills.local/`: every chat at that level reads a local skill, and a draft must reach no chat but its Try it. It lives in the state directory's `skill-drafts/<id>/` (`docs/plane-format.md`), which no save stages and no chat may write.
- **One draft a skill.** `<id>` is derived from the level and the skill's folder, so a second write to the skill, from any chat, adds to the same draft. An `Edit` is made to the draft's copy of the file where the draft has one, else to the project's.
- **Each file keeps the base it had** when the draft first changed it. Accept refuses a draft whose project file changed since, and the studio says so. It is never merged.
- **The window is told on the asks' event** (`asks-changed`), which a draft's arrival raises without being an ask, so the studio and the chat's tab read the drafts again. The Inbox lists no draft.

## Accept is one commit, Reject deletes

- **Accept** writes each of the draft's files into the skill's folder, only while the project's file is what the draft's base says, and commits exactly those files through the plane's one save (`planegit::save_paths`), as each change of #2770's is. A local skill's draft is written and left uncommitted. A draft whose instructions the live lint finds a problem in, a new skill without instructions, or one whose name a built-in skill or another skill of the level has, is refused.
- **Reject** removes the draft's folder through the handle of the folder that holds it.

## Try it runs in a throwaway piece

- The ticket asked for the smallest throwaway piece Try it needs, which grading (#2779) will use too. **It is a folder, not a worktree**: `purlis-try-<id>`, made new and private in the person's temp folder, outside the project. A worktree needs a repo, and a draft at the project's root has none. A folder outside the project also means the scratch chat's writes never reach the project's tree or a save.
- **The skill is laid out in this machine's state, in `skill-tries/<id>/`**, as the one project skill of a project of its own. The layout is the project's files of the skill with the draft's over them. The host arms a chat whose folder is a try's piece from that folder instead of the project (`drafts::tried`), at its root and as no persona. So its set is the built-in skills and the draft alone, on every route: a Claude Code or opencode chat's folder of links, and a Codex chat's briefing and its skill-run reads. Only this machine's state names a piece, and no chat writes the state, so a chat cannot make another folder a try.
- **The harness is the project's default profile**, or the one the person picks in Try it's list (decision 50). The prompt is typed and never sent, as a curation chat's is.
- **Closing the chat ends the try**: the piece and `skill-tries/<id>/` are removed once its program has had time to end, where no other open chat stands in the piece (a restart starts before the chat it replaces ends). A Try it also ends every try no open chat stands in, which a quit or a crash left. Removal is of the names purlis made, through folder handles, never following a link. **What is left is the skill run**, which the plane's event log keeps under the skill's name.
- **Harness skills still load** in a scratch chat: purlis cannot keep a harness from its person's own folders and plugins (§ "Harness and repo skills"). "Only the draft" is about the project's skills.

## What was rejected

- **Holding a draft as a proposed change.** A proposal is an ask in the Inbox with Apply; a draft is tried first, and the spec says nothing else asks for it.
- **A draft in `skills.local/`.** Every chat at the level would get it before the person tried it.
- **A worktree of the plane as the throwaway piece.** A plane without git, or one mid-save, could not cut one, and the chat would stand in the project's tree.
