# Amended for #2786, the Skill studio's flag is gone, 2026-11-10

[The amendment for #2977](2026-11-10-amended-for-2977-a-runner-says-the-skills-a-chat-lacks-there.md)
says a runner's check of a chat's skills honours the Skill studio's flag the desktop sent with
the start. That flag is gone (SK-23, #2786;
[ADR 0099 as amended 2026-11-04](../0099/2026-11-04-the-skill-studio-flag-is-gone.md)). What the
desktop sends is its word on whether the plane's skills reach the chat (`plane_skills`), which
every desktop of this build gives as yes. Only an older desktop can still send no. The check is
otherwise unchanged: the runner asks `hoststart::skills_not_here` as the window does, and says
the result.
