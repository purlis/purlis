# The move does not wait on the flag, and a project that requires its skills reaches a runner's chat

**Amends [ADR 0099](../0099-a-skill-lives-at-a-level-a-project-skill-reaches-a-workspace-by-grant-and-the-narrowest-copy-wins.md), 2026-10-31**, ticket #4344, parent #2786. This changes two things recorded earlier: the refusal in [the 2026-10-22 amendment](2026-10-22-the-migration-from-claude-skills-as-built.md), and SK-20b's rule (#3084) that a link's start decides alone. The operator's decision of 2026-10-31, relayed by the chat that dispatched the ticket: yes to both.

## The move runs with the flag off

`purlis doctor --fix project-skills` no longer refuses while the Skill studio's flag is off. The 2026-10-22 amendment's "the move is part of the preview" no longer holds. Removing the flag (#2786) would have left a refusal naming a setting that no longer exists, and the move's own checks are what keep each chat's skills, not the flag.

It still runs only by its name, because it is a commit every teammate pulls. Bare `--fix` never makes it, and every other refusal stands.

## A project's requirement wins over its desktop's word

A runner's chat is reached by the project's skills where its desktop said so, or where the project requires `project-skills`: `said || requires`. Before, a desktop's `false` won. A runner that had pulled the move, started by a desktop that had not, armed the chat with no project skills, and its clone holds no copy.

- A project that has not moved keeps its desktop's `false`: its programs read `.claude/skills` themselves.
- A start that says nothing takes the host's own reading, `reach_chats_here`, so #2786 changes that one question and this follows.
- A chat reattached keeps its start's word (#3445, #3466): its program is still the one armed at that start. A restart reads anew.

A sandboxed chat on such a runner is now held to the checks of a chat that gets skills: a runner with no app data home refuses to start it rather than starting it without them. The variable that decides the flag inside that chat says the same word its arming took.

## What stays by the flag until #2786

On a machine with the flag off, a moved project's chats have their skills, but the studio's own surfaces stay hidden, as the 2026-10-22 ruling has it: `Use skill…`, `purlis skill`, the chat's Skills view, and Accept for a workspace or persona folder a sandboxed chat holds back. A desktop whose record says `false` for a chat its runner armed keeps that word until the chat restarts. These last only until the flag goes, and running the move with the flag off makes them reachable by the person who ran it, not only by a teammate.

## What an unmigrated project's chats get with the reading on

Nothing is lost, and nothing is written or committed. Claude Code and opencode keep reading `.claude/skills` at the root and through the copy in a clone, which goes on being made until the project moves. Codex never read that folder. A skill in both places stays with every chat that had it, and the move refuses rather than choosing a copy. A persona that names an unmoved skill is told it is missing only on a program that reads no copy, as Codex does not. `project_skills_move_without_a_chat_losing_one.rs` holds this through the skill sets purlis works out, not through a real harness's own listing.

## What was rejected

- **Moving a project automatically at the upgrade.** It is a commit every teammate pulls, and nobody would have asked for it. An unmigrated project loses nothing by waiting.
