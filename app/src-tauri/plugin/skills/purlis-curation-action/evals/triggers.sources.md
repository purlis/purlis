# purlis-curation-action: where each trigger test came from

Read by the CI grader (spec #5714) beside `triggers.json`: the trigger bar, the neighbours whose
requests are the near misses, and one note per test in the order of `triggers.json`. A mined
session record is one of this project's `workspaces/smart-ide/sessions/` records, named by its file;
its situation is paraphrased and scrubbed of names, numbers that identify anything private, and
values. Session records are summaries, not transcripts, so no prompt is quoted word for word. A
filed miss Report replaces a plausible phrasing through a reviewed change.

Trigger bar: standard
Neighbours: purlis-skill-studio, purlis-persona

## Should start purlis-curation-action

1. Skill text: description, "add ... a persona's curation action".
2. Skill text: description, "a new entry in the Curate menu".
3. Skill text: the example's `label: Review open work`; description, "change".
4. Skill text: `on: persona`.
5. Skill text: `on: plane`; "name the skill in words".
6. Skill text: `runs-in: plane`; Mined session record 20261108-085457-5724-four-built-in-skills-renamed-losing-nothing.md (curation action ids kept).
7. Skill text: "with a prompt already typed into it".
8. Skill text: "exactly four variables".
9. Mined session record 20261105-054255-5186-pre-filled-prompts-start-their-skill-per-ha.md: a curation prompt that names a skill.
10. Skill text: "one file, `personas/<persona>/curation/<id>.md`".
11. Skill text: "any other `{word}` is an error".
12. plausible phrasing a person types.
13. Skill text: `on` is comma-separated.
14. Skill text: `label` is what the menu shows.
15. plausible phrasing a person types.
16. Skill text: "one short, plain line".
17. plausible phrasing a person types: changing a persona's actions.
18. Mined session record 20260930-213238-si-1-si-8-plane-root-curation-crud-pane-ux-shell.md: plane-root curation.
19. Skill text: "never as a `/slash` command, which only one harness reads".
20. plausible phrasing a person types.
21. Skill text: `runs-in: subject`.
22. Skill text: `label`, `on`, `runs-in`.

## Should not start it

23. Near miss (purlis-skill-studio): the prompt its New skill button types.
24. Near miss (purlis-skill-studio): the prompt its trigger tests button types.
25. Near miss (purlis-skill-studio): the prompt its outcome evals button types.
26. Near miss (purlis-skill-studio): its description, "an edit drawn from its runs and grades".
27. Near miss (purlis-persona): its description, "create one".
28. Near miss (purlis-persona): its description, "dispatch work to another".
29. Near miss (purlis-persona): its description, "act as a role".
30. Near miss (purlis-persona): its description, "deciding which persona a piece of work belongs to".
31. plausible phrasing a person types: "curate" elsewhere.
32. plausible phrasing a person types: a menu in code.
33. plausible phrasing a person types: "action" elsewhere.
34. plausible phrasing a person types: a menu entry elsewhere.
35. plausible phrasing a person types: a prompt, not purlis.
36. plausible phrasing a person types: unrelated.
37. plausible phrasing a person types: a harness question.
38. plausible phrasing a person types: unrelated.
39. plausible phrasing a person types: unrelated.
40. plausible phrasing a person types: unrelated.
41. plausible phrasing a person types: unrelated.
42. plausible phrasing a person types: a menu in code.
43. plausible phrasing a person types: unrelated.
44. plausible phrasing a person types: unrelated.
