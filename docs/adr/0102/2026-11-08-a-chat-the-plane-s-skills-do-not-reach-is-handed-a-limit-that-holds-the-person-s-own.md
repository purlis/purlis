# A chat the plane's skills do not reach is handed a limit that holds the person's own

**Amends [ADR 0102](../0102-a-skill-router-decides-what-a-chat-sees-and-nothing-it-hides-is-out-of-reach.md), 2026-11-08**, ticket #5842, under the operator's spec #5714. It closes the first item that [Claude Code's limit holds the person's own skills](2026-11-08-claude-code-s-limit-holds-the-person-s-own-skills-and-codex-lists-built-ins-first.md) left open.

## The question

That record hands Claude Code a listing limit that holds the person's own skills and plugins, read at the chat's start. It counted them only where the plane's skills reach the chat, because the arming read them through the plane's skills. A chat the plane's skills do not reach (the Skill studio's flag off, or a workspace granted none) still lists the person's skills beside purlis's built-in ones, and was handed the 40,000 floor alone, so a large catalogue lost its descriptions there.

## The decision

- **The person's own catalogue is part of what arms every chat, apart from the plane's skills.** A chat's start reads the skills its harness finds itself whatever the plane hands it (`hoststart::ChatSkills::own_read`, as before), and the arming carries them on its own (`harness::Kit::own`), not inside the plane's skills. Claude Code's limit is worked out from them as the earlier record says, with purlis's list then its built-in skills alone.
- **Nothing else changes.** The ceiling of 100,000, the floor of 40,000, the margin and the need _Some descriptions are dropped_ are the earlier record's. Where the plane's skills do not reach a chat there is still no plane copy to leave out ([One skill from two sources takes one slot](2026-11-08-one-skill-from-two-sources-takes-one-slot-and-differing-copies-are-both-listed.md)).

## How it is held

A chat composed in the session host with the plane's skills off, whose Claude Code profile points at a config folder of the person's own skills, is handed a limit that holds them, the same limit as with the plane's skills on and none in the plane (`a_chat_the_planes_skills_do_not_reach_is_handed_a_limit_that_holds_the_persons_own_skills`).

**Built by #5842.**
