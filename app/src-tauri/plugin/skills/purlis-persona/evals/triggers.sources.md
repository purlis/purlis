# purlis-persona: where each trigger test came from

Read by the CI grader (spec #5714) beside `triggers.json`: the trigger bar, the neighbours whose
requests are the near misses, and one note per test in the order of `triggers.json`. A mined
session record is one of this project's `workspaces/smart-ide/sessions/` records, named by its file;
its situation is paraphrased and scrubbed of names, numbers that identify anything private, and
values. Session records are summaries, not transcripts, so no prompt is quoted word for word. A
filed miss Report replaces a plausible phrasing through a reviewed change.

Trigger bar: strict
Neighbours: purlis-handoff, purlis-secrets, purlis-safe-remove

## Should start purlis-persona

1. Mined session record 20261102-120712-4585-prototype-a-chat-s-actions-in-its-pane-acce.md: "I asked `skills`, which chose (a)".
2. This ticket's own brief: a task dispatched by the asking chat, reported with `purlis dispatch report`.
3. Mined session record 20261101-031011-review-of-pr-4361-hot-count-role-3098-fix-first-.md.
4. Skill text: description, "create one".
5. Skill text: "Know which one is active" (`purlis persona current`).
6. The steward persona's own `delegate-when` line; skill description "act as a role".
7. Skill text: description, "deciding which persona a piece of work belongs to".
8. Mined session record 20261102-044257-security-review-of-pr-4589-chrome-crash-frames-n.md: a security review as its own task.
9. decisions.md of spec #5714 Q30: persona when this chat needs the answer.
10. Skill text: `purlis persona list`.
11. Skill text: "a role with a written charter"; `purlis docs show personas`, the charter format.
12. decisions.md of spec #5714 "Implementation run": spec #5714's tickets run as parallel tasks.
13. Skill text: `purlis persona show <name>`.
14. Skill text: "A task ... can be waited on, told more and cancelled" (from purlis-handoff, which points here).
15. decisions.md of spec #5714: #5725 held while its plane step waits; plausible phrasing a person types.
16. Skill text: `purlis docs show personas`, inheritance.
17. decisions.md of spec #5714 Q28(d): dispatch when the work needs another persona's charter, memory or vault.
18. Skill text: `purlis persona where`.
19. decisions.md of spec #5714: the run waited on dispatched tasks' reports; plausible phrasing a person types.
20. decisions.md of spec #5714 Q28(d): do it yourself; dispatch when it needs another persona's charter, memory or vault.
21. This ticket's own brief (`purlis dispatch report --outcome done`); decisions.md of spec #5714 Q28(j).
22. Skill text: `purlis docs show personas`, roster health.
23. decisions.md of spec #5714 Q30: "hand" but this chat needs the answer, so a dispatch.

## Should not start it

24. Near miss (purlis-handoff): the person reads the new chat ({D} Q30).
25. Near miss (purlis-handoff): its description, "open a chat for something".
26. Near miss (purlis-handoff): fire-and-forget, nothing comes back.
27. Near miss (purlis-handoff): the person will read it.
28. Near miss (purlis-secrets): a credential for a command, though vaults are a persona's.
29. Near miss (purlis-secrets): its description, "when asked to store one".
30. Near miss (purlis-secrets): "Find out what exists".
31. Near miss (purlis-safe-remove): its description, "Remove ... a persona without losing what it learned".
32. Near miss (purlis-safe-remove): Mined session record 20261019-073303-browser-flow-session-host-profiles-and-runners-p.md: this project's QA personas, left mentioning a retired install; its description's "retire".
33. Near miss (purlis-safe-remove): removal.
34. plausible phrasing a person types: a UX persona, not purlis's.
35. plausible phrasing a person types: marketing personas.
36. plausible phrasing a person types: "role" in a database.
37. plausible phrasing a person types: "dispatch" in front-end code.
38. plausible phrasing a person types: "dispatcher" in code.
39. plausible phrasing a person types: "charter" as a document.
40. plausible phrasing a person types: roles in access control.
41. plausible phrasing a person types: work this chat does itself.
42. plausible phrasing a person types: unrelated.
43. plausible phrasing a person types: unrelated.
44. plausible phrasing a person types: "job" without a task.
45. plausible phrasing a person types: unrelated.
