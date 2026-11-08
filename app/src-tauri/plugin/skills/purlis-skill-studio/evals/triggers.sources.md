# purlis-skill-studio: where each trigger test came from

Read by the CI grader (spec #5714) beside `triggers.json`: the trigger bar, the neighbours whose
requests are the near misses, and one note per test in the order of `triggers.json`. A mined
session record is one of this project's `workspaces/smart-ide/sessions/` records, named by its file;
its situation is paraphrased and scrubbed of names, numbers that identify anything private, and
values. Session records are summaries, not transcripts, so no prompt is quoted word for word. A
filed miss Report replaces a plausible phrasing through a reviewed change.

Trigger bar: standard
Neighbours: purlis-curation-action, purlis-persona

## Should start purlis-skill-studio

1. Skill text: "Draft a new skill"; the prompt the studio types (`asking` in crates/purlis-core/src/skills/verbs/drafts.rs).
2. The prompt the studio types when what the skill does was left empty, finished by the person.
3. Skill text: "Trigger tests"; the prompt `asking_for_trigger_tests` types.
4. Skill text: "Outcome evals"; the prompt `asking_for_outcome_evals` types.
5. Skill text: "Checks: trigger tests and outcome evals together"; the prompt `asking_for_checks` types.
6. Skill text: "Improve a skill from its runs and grades".
7. The prompt the studio types, whole, as a harness without its own skill start shows it.
8. Skill text: "Redo a draft".
9. Mined session record 20261104-231538-5183-skill-page-files-tab-open-in-finder-pr-5199.md: a skill's page and its checks.
10. Skill text: the `triggers.json` format.
11. Skill text: the `evals.json` format.
12. Skill text: description, "a new skill"; plausible phrasing a person types.
13. Skill text: description, "an edit drawn from its runs and grades".
14. Skill text: description, "both kinds of checks at once".
15. The prompt the studio types at a persona's level.
16. Skill text: "Redo a draft"; "the person accepts or discards it there".
17. Skill text: "Trigger tests", what makes a good test.
18. The prompt the studio types, unfinished, when New skill was left empty (`asking`).
19. Mined session record 20261108-035419-5738-a-correction-records-a-miss-skill-studio-sh.md: runs and misses in the studio.
20. Skill text: "Outcome evals"; plausible phrasing a person types.
21. The prompt `asking_for_checks` types at a workspace's level.
22. The prompt the studio types at a workspace's level.

## Should not start it

23. Near miss (purlis-curation-action): a skill named in a curation action.
24. Near miss (purlis-curation-action): its description, "the Curate menu".
25. Near miss (purlis-curation-action): a Curate entry whose chat writes a skill.
26. Near miss (purlis-curation-action): its variables.
27. Near miss (purlis-persona): its description, "create one".
28. Near miss (purlis-persona): its description, "dispatch work to another".
29. Near miss (purlis-persona): a persona's skills, not a skill's text.
30. Near miss (purlis-persona): "which persona a piece of work belongs to".
31. plausible phrasing a person types: tests in code.
32. plausible phrasing a person types: evals in code.
33. plausible phrasing a person types: "skills" elsewhere.
34. plausible phrasing a person types: "skills" elsewhere.
35. plausible phrasing a person types: tests in code.
36. plausible phrasing a person types: "draft" elsewhere.
37. plausible phrasing a person types: "grade" elsewhere.
38. plausible phrasing a person types: tests in code.
39. plausible phrasing a person types: "improve from runs" in code.
40. plausible phrasing a person types: unrelated.
41. plausible phrasing a person types: "draft" elsewhere.
42. plausible phrasing a person types: unrelated.
43. plausible phrasing a person types: evals elsewhere.
44. plausible phrasing a person types: a question, not a studio job.
