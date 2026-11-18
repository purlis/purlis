# Amendment, 2026-11-16: Project recommends and the timed exception retire (Q312)

[ADR 0126](../0126-one-project-policy-governs-every-chat-decision-cedar.md) makes the project's
committed policy the Project default, which a person only tightens. Q312 point 3 supersedes four
parts of this record. None of them was built, so nothing migrates.

**Decision 3's "Project recommends" row and item 3.**
- There is no **Project recommends** tier. The project's committed template, modes and rules are
  the Project default, and joining asks nothing.
- **Project requires** becomes a Project lock or forbid.
- The **timed exception** becomes a **timed grant**: a sealed grant scoped to this chat until a
  time. It exists only at an `@ask`, never against a deny, a lock, a never, the kill switch or a
  grader's bundle. `exceptions = "self-declared"` is dropped.

**Decision 4.1's and 4.6's wording.**
- **Use it / Keep mine** becomes **Accept / Not now**: the loosening press of ADR 0126 decision 2.
- At setup and on joining, nothing is recommended to take or decline. A team's choice is the
  committed policy, and loosening it is a project change through git plus each person's press.
- 4.6's change protocol is the press's: a tightening applies at once, and a loosening counts only
  once this person presses Accept on the committed text the window showed.

**Decision 9's "timed exception where it exists".** A policy or project block's ways out are the
level, the file and the `owner` contact. At an ask, the way out for this chat alone is a timed or
This chat grant. At a deny, it is **Propose a project change…**, never **Turn off for this chat**.
