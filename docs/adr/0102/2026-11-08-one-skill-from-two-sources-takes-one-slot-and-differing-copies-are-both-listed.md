# One skill from two sources takes one slot, and differing copies are both listed

**Amends [ADR 0102](../0102-a-skill-router-decides-what-a-chat-sees-and-nothing-it-hides-is-out-of-reach.md), 2026-11-08**, ticket #5737, under the operator's spec #5714 (decisions Q1–Q36, settled 2026-11-08). It adds to [Claude Code's limit holds the person's own skills](2026-11-08-claude-code-s-limit-holds-the-person-s-own-skills-and-codex-lists-built-ins-first.md), whose read of the harness's own skills at a chat's start it uses.

## The question

The same skill often reaches a chat twice: a plane skill copied from a plugin the person also installed (`tdd` from the project and from a plugin). Each copy takes a slot and a description in the listing. When the two copies differ, either one may be the right one.

## The decision

- **The same content takes one slot.** Where a plane skill has the name of a skill the chat's harness finds itself (the person's own, or a plugin's other than purlis's own) and every file of the two folders is the same, purlis leaves its own copy out of the chat (`SkillSet::without_copies_of`). The copy left out says which copy is listed in its place: *the same as the copy plugin mattpocock-skills brings, which is listed in its place*. purlis keeps the harness's copy because purlis only ever leaves out what it hands. The check runs at the chat's start, so the copy is never laid out. Use skill… and the restart's stamp see the same set.
- **Different content is never decided for the person.** Both copies are listed. The chat's Skills view has a need, *Two skills share a name*, that names each pair and whose each copy is (`copies::differing_copies`, `skills::twins_said`). It names pairs among purlis's copies and the harness's own copies, including two plugins that bring one name. Its fix opens Harness skills.
- **Two folders are the same** only where they hold the same files at the same paths with the same bytes. A folder with a link in it, one that cannot be read whole, or one past 1,000 files is the same as no other, so its copies are both listed.
- **Unchanged:** a built-in skill never gives way (SK-37). A repo's copy keeps decision 51: the narrower copy wins where purlis builds what the harness is handed (`Outcome::RepoWins`), so a repo copy is in neither rule. Ties between plane levels keep the narrowest copy.

## What remains

- Two identical copies that both come from the harness (two plugins, or a plugin and the person's own folder) still both load. purlis does not turn off a skill it does not hand, so it names no pair for them either.
- A chat the plane's skills do not reach is handed no plane skills, so it has no copy to collapse.
- The studio's previews and a Codex chat's briefing hook pass no harness skills, so they show both copies of an identical pair. A chat's start, its Use skill… and its Skills view leave the copy out.
- Two folders are compared each time a chat's set is worked out. Same-named pairs are rare, and each comparison stops at the first difference in the file list.

**Built by #5737.**
