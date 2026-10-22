# Changing a skill, one commit at a time, and the restart offer, as #2770 built them

**Amends [ADR 0099](../0099-a-skill-lives-at-a-level-a-project-skill-reaches-a-workspace-by-grant-and-the-narrowest-copy-wins.md), 2026-10-21**, ticket #2770 (SK-7). ADR 0099 §5 left the restart offer to this ticket, including whether purlis types Claude Code's `/reload-plugins` for the person. Spec #2762's decisions 14, 15 and 37 say what a person can do to a skill. This file records how both were built.

## Each change to a skill is one commit of its own paths

- New, edit, rename, move, delete, restore, grant, turn-off and Share are each one function in `purlis_core::skills::verbs`. The Skill studio and `purlis skill new | edit | mv | rm | restore | grant | share` both call them.
- Each one commits through the plane's one save, so the plane's mode, the secret guard, the push and the journal all apply. The commit is staged by **exactly the paths the change touched** (`planegit::save_paths`), the way `doctor --fix rename-plane` stages its commit. A save that finds something else already staged refuses, because the commit would carry it. A project that has not chosen how it is saved keeps the change on disk for the next save, as a workspace rename does.
- **Rename** changes the `name:` line in `SKILL.md`, the folder and the `<name>.toml` beside it. It also rewrites every place that names the skill: each persona's `skills:` list (bare, or as `skills:<name>`), each `skills:<name>` reference in a persona's or a skill's text, and, for a project skill, each workspace's `settings.skills.off`. If another skill still has the old name, those places are left alone, because they might mean the other skill, and purlis says so.
- **Move** goes between the project's and the workspaces' levels, shared or local, and never lands on anything already there. A move into the project's skills must say where the skill reaches, since a project skill reaches only what its grant names. A move out of the project's skills drops the grant.
- **Delete** first shows who uses the skill: the chats it reaches, and the personas and skills that name it.
  - A committed skill is removed with `git rm` and committed only where that commit can land now (`planegit::paths_unsaved`), and Restore brings it back from the parent of that commit. If the commit is then not made, the files are put back. A skill that holds files git does not have yet is refused, because those files could not be restored.
  - Where the commit cannot land (no save mode yet, saving off, or something else staged), the skill is kept on this machine instead, as an untracked one is, so a delete never loses a skill.
  - A skill git does not track is moved into this machine's `skills-deleted/` (`docs/plane-format.md`).
- **The level defaults to where the person came from:** a workspace's studio makes a new skill there, and the project's studio makes it at the project's root.
- **A chat does not change skills itself.** `purlis skill`'s writes are refused inside a chat. A chat's change becomes a draft the person accepts in the studio (SK-9, #2772).

## A running chat whose skills changed is offered a restart, and nothing is typed into it

- The window keeps a stamp of each chat's plane skills from when the chat started (`purlis_skills::Stamp`). When the project's skills change, it takes the stamp again. If the two differ in a way the chat's harness does not pick up by itself (`Harness::skills_reload`), the chat's pane says its skills changed and offers **Restart**. That is the restart every other restart uses: on the same conversation, once its turn has ended.
- **Claude Code and opencode pick up nothing**, as ADR 0099 §5 measured. **Codex picks up the bodies.** Its briefing lists each skill's `SKILL.md` path once, and the model reads that file where it lies each time the skill applies. So an edit to a listed skill's text needs no restart, and every other change does. This follows from the briefing route itself, not from Codex's own watcher, which purlis never feeds. It was not measured live, for the reason §5 gives.
- **purlis does not type `/reload-plugins` for the person.** A chat is a terminal the person may be typing into at that moment, and nothing else in purlis types into a chat on its own. One restart offer covers all three harnesses.

## What was rejected

- **Committing a skill change with the normal whole-tree save.** That save commits everything that is dirty, so a skill change would carry someone's unrelated work under its message.
- **Writing the kept copy of a deleted committed skill to this machine.** Git already has it, and on every machine. A second copy could go stale, and a teammate could not restore it.
- **Rewriting every bare mention of a skill's name in prose on rename.** "deploy" is also an ordinary word. Only the frontmatter lists and the qualified `skills:<name>` form are references.
