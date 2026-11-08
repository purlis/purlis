# purlis-remember: where each trigger test came from

Read by the CI grader (spec #5714) beside `triggers.json`: the trigger bar, the neighbours whose
requests are the near misses, and one note per test in the order of `triggers.json`. A mined
session record is one of this project's `workspaces/smart-ide/sessions/` records, named by its file;
its situation is paraphrased and scrubbed of names, numbers that identify anything private, and
values. Session records are summaries, not transcripts, so no prompt is quoted word for word. A
filed miss Report replaces a plausible phrasing through a reviewed change.

Trigger bar: standard
Neighbours: purlis-tidy-memory, purlis-smart-close, purlis-secrets

## Should start purlis-remember

1. Skill text: description, "remember this"; a verified fact about a system (Where a fact goes, 2).
2. Skill text: description, "make a note for next time"; Mined session record 20261108-023505-5720-smart-close-and-compact-fold-into-workspace.md (a toolchain gotcha recorded for later chats).
3. Skill text: description, "add a todo"; `todo_add`.
4. Skill text: description, "what did we decide"; Recall.
5. Skill text: "True for every persona, anywhere in this project" (shared memory).
6. Skill text: Where a fact goes, 1 (`workspace_section` with section `decisions`).
7. decisions.md of spec #5714 Q29: the one rule for where a fact goes.
8. Skill text: description, "mark a todo done"; `todo_done`.
9. Skill text: description, "file a todo as an issue"; `purlis ws todo promote`; Mined session record 20261103-074516-skill-studio-docs-todos-filled-five-mismatches-f.md.
10. Skill text: `purlis recall` (every memory at once).
11. Skill text: description, "note that"; a verified fact about a system.
12. Skill text: section `glossary`.
13. Skill text: the `workspace_vision` tool.
14. Skill text: Where a fact goes, 3 (this persona's memory).
15. Skill text: Recall; Mined session record 20261106-234233-5498-the-project-index-adr-0079-recall-on-it-the.md.
16. Skill text: `todo_list`.
17. Skill text: `--repo` takes the repo's name in this workspace.
18. plausible phrasing a person types.
19. plausible phrasing a person types.
20. Skill text: "what did we decide"; plausible phrasing a person types.
21. Skill text: shared memory, `--shared`.
22. Skill text: `purlis ws todo "<text>"` with `-w <workspace>`.

## Should not start it

23. Near miss (purlis-tidy-memory): its description, "tidy" (decisions.md of spec #5714 Q30).
24. Near miss (purlis-tidy-memory): its description, "dedupe".
25. Near miss (purlis-tidy-memory): its description, "prune".
26. Near miss (purlis-tidy-memory): moving memory that is already there.
27. Near miss (purlis-tidy-memory): its description, "archive".
28. Near miss (purlis-smart-close): its name, as typed.
29. Near miss (purlis-smart-close): its description, "wrap up", "write a session record".
30. Near miss (purlis-smart-close): a summary of a chat that is ending.
31. Near miss (purlis-secrets): a credential is never memory (Skill text: "Never a secret").
32. Near miss (purlis-secrets): its description, store a credential.
33. Near miss (purlis-secrets): Mined session record 20261105-050425-5240-secret-need-remembered-only-when-asked-pr-5.md (a secret need, not a memory).
34. plausible phrasing a person types: a TODO in code.
35. plausible phrasing a person types: RAM, not purlis memory.
36. plausible phrasing a person types: caching in code.
37. plausible phrasing a person types: an issue with no todo behind it.
38. plausible phrasing a person types: git history, which the repo records.
39. plausible phrasing a person types: "note" in a doc.
40. plausible phrasing a person types: "todo" as a product.
41. plausible phrasing a person types: a code search, not recall.
42. plausible phrasing a person types: memory as a setting.
43. plausible phrasing a person types: "recall" in a session.
44. plausible phrasing a person types: "mark ... done" elsewhere.
