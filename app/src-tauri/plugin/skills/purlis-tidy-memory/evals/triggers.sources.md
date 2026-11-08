# purlis-tidy-memory: where each trigger test came from

Read by the CI grader (spec #5714) beside `triggers.json`: the trigger bar, the neighbours whose
requests are the near misses, and one note per test in the order of `triggers.json`. A mined
session record is one of this project's `workspaces/smart-ide/sessions/` records, named by its file;
its situation is paraphrased and scrubbed of names, numbers that identify anything private, and
values. Session records are summaries, not transcripts, so no prompt is quoted word for word. A
filed miss Report replaces a plausible phrasing through a reviewed change.

Trigger bar: standard
Neighbours: purlis-smart-close, purlis-safe-remove

## Should start purlis-tidy-memory

1. Skill text: description, "tidy" (it was purlis-compact; Mined session record 20261108-085457-5724-four-built-in-skills-renamed-losing-nothing.md).
2. Skill text: `purlis persona dedupe`.
3. Skill text: "exact duplicates, near duplicates, stale, index drift".
4. Skill text: description, "prune".
5. Skill text: "Improve the charter"; Mined session record 20261108-023505-5720-smart-close-and-compact-fold-into-workspace.md.
6. Skill text: `purlis workspace optimize`; "Read the reports".
7. Skill text: "Near duplicates merge by hand".
8. Skill text: "Stale memories are still true or they are not".
9. Skill text: `purlis workspace move-memory ... --to-shared`.
10. Skill text: "A lesson that has held across several memories ... belongs in the charter".
11. Skill text: "archived, and only when the person agrees".
12. Skill text: description, "improve".
13. Skill text: "Compacting makes it smaller and truer"; Mined session record 20261107-020518-5504-dedupe-optimize-and-stats-score-candidates-.md.
14. Skill text: "Exact duplicates and index drift are safe and reversible".
15. plausible phrasing a person types.
16. Skill text: "`workspace forget` ... for a memory that must not survive, such as one holding a secret".
17. Skill text: "`unarchive` / `unarchive-memory` puts it back".
18. Skill text: "Done when you can say, for each finding, what you propose".
19. plausible phrasing a person types.
20. Skill text: "index drift".
21. Skill text: "then archive the memories it replaces".
22. plausible phrasing a person types.

## Should not start it

23. Near miss (purlis-smart-close): this chat ending (decisions.md of spec #5714 Q30).
24. Near miss (purlis-smart-close): its name, as typed.
25. Near miss (purlis-smart-close): its description, "wrap up".
26. Near miss (purlis-smart-close): a summary of this chat.
27. Near miss (purlis-smart-close): its description, "write a session record".
28. Near miss (purlis-safe-remove): removal only in safe remove (decisions.md of spec #5714 Q30).
29. Near miss (purlis-safe-remove): its description, "retire".
30. Near miss (purlis-safe-remove): promoting memories on the way to a removal.
31. Near miss (purlis-safe-remove): "clean up" meaning removal.
32. plausible phrasing a person types: the harness's own /compact, which the rename from purlis-compact kept apart (spec #5714).
33. plausible phrasing a person types: RAM, not purlis memory.
34. plausible phrasing a person types: RAM.
35. plausible phrasing a person types: "memory" in code.
36. plausible phrasing a person types: "dedupe" on data.
37. plausible phrasing a person types: "clean up" in code.
38. plausible phrasing a person types: "prune" elsewhere.
39. plausible phrasing a person types: "archive" elsewhere.
40. plausible phrasing a person types: "tidy" on a file.
41. plausible phrasing a person types: unrelated.
42. plausible phrasing a person types: a fact to record, not a tidy.
43. plausible phrasing a person types: "merge" in git.
44. plausible phrasing a person types: "delete" in a repo.
