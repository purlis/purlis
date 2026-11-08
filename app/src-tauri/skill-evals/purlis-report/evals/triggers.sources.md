# purlis-report: where each trigger test came from

Read by the CI grader (spec #5714) beside `triggers.json`: the trigger bar, the neighbours whose
requests are the near misses, and one note per test in the order of `triggers.json`. Spec #5683
(Q20, #5699) holds purlis-report to the strict bar: a miss is a purlis bug nobody reports.

Sources: the plane's session records and persona memory where a purlis bug came up, each
paraphrased and scrubbed (no workspace, project or person names, no paths but purlis's own), the
skill's six cases, and phrasings a person plausibly types where no record exists. A record is
named by its date and subject; a filed miss Report replaces a plausible phrasing through a
reviewed change.

Trigger bar: strict
Neighbours: purlis-skill-studio, purlis-permissions

## Should start purlis-report

1. Session record 2026-11-05, the paste crash on macOS 27 (#5004): the app quit on Cmd+V of a merge request link, three times in a day. Case 4.
2. Session record 2026-11-03, the restore at launch (#4980, #4986): only one of three projects came back. Case 3.
3. Session record 2026-11-03, the restore at launch, its Open part: Retry seemed stuck. Case 4.
4. Session record 2026-11-03, the 3–5 minute launch wait with about thirty chats. Case 4 ("this is slow").
5. Session record, #5478: a chat's terminal frozen half-drawn while the chat runs. Case 3.
6. Session record, moving a chat to a Docker pool fails at `--attr-source`: the error as the person saw it. Case 1.
7. Session record 2026-11-01, an installed skill gets "Unknown skill" for its siblings (#4510, a purlis fix). Case 1.
8. Session record 2026-11-02, Ask a chat never opened a chat (#4714, a prompt over the paste limit). Case 3.
9. Steward memory, gotchas of 2026-11-03: `purlis worktree done` fails from a task's sandbox on the piece log. Case 1.
10. Steward memory 2026-11-08: `purlis worktree commit --all` leaves new files out with no warning. Case 1.
11. Steward memory, lane stall lesson 2026-11-08: the sandbox refused the piece `purlis worktree add` cut; the agent works around it. Cases 2 and 5.
12. Shared memory 2026-11-02: `git reset --hard` in a sandboxed purlis branch folder fails on vault fixtures; the agent works around it. Case 5.
13. Steward memory 2026-11-08: dispatch answers "the purlis app did not answer"; the agent does the work by hand. Cases 1 and 5.
14. Steward memory 2026-11-05: a lane shown waiting on a background run the restart killed. Case 3.
15. Session record, #5207: a chat stays sandboxed after the person lowered their level, and nothing says why. Cases 3 and 4.
16. Session record, #4562: a background turn cleared a wait the person had not answered. Case 3.
17. Session record, #5283: wrong names after a pane split. Case 3.
18. Session record, #5284: a dashed line across the window's top after a pane move. Case 3.
19. Session record, #4822: no theme drawn when extensions fail at launch. Case 3.
20. Session record 2026-11-04, #5108: a press goes to another pane than the focused one; "why did it do that". Case 4.
21. Spec #5683 story 27 and decisions.md Q4: "you should have used the X skill". Case 6.
22. Spec #5683 story 27: the person names the skill that should have started, after the agent did its work another way. Case 6; plausible phrasing.
23. Spec #5683 story 27: the agent realises it did by hand what a purlis skill does. Case 6; plausible phrasing.
24. Skill text case 1 ("does something its help or docs say it does not"); plausible phrasing.

## Should not start it

25. Near miss (purlis-skill-studio): writing a built-in's trigger tests, which purlis-skill-studio's description claims.
26. Near miss (purlis-skill-studio): rewriting a description after a missed trigger test; a skill that did not start, but the ask is the edit, not a report.
27. Near miss (purlis-skill-studio): "improve it from its runs and grades", from its description.
28. Near miss (purlis-skill-studio): drafting a new skill in a skills folder, from its description.
29. Near miss (purlis-skill-studio): editing this skill's own triggers.json (purlis-report's description: "Not for changing a skill's description or its trigger tests").
30. Near miss (purlis-permissions): the harness keeps asking before a command, from its description.
31. Near miss (purlis-permissions): allowing a host the sandbox blocked, from its description.
32. Near miss (purlis-permissions): a purlis guard refusing on purpose; this session's own refusal of `purlis worktree update` beside another command (2026-11-08). Spec #5683 story 21: a refusal on purpose is not a bug.
33. Near miss (purlis-permissions): allowing a folder the sandbox refuses, from its description.
34. Near miss (purlis-permissions): listing purlis's permission rules, from its description.
35. Skill text "in your project": a bug in the person's own code. Plausible phrasing.
36. Skill text "in your project": the person's failing test. Plausible phrasing.
37. Skill text "in your project": the person's build. Plausible phrasing.
38. Skill text "in your project": a paste crash in the person's own app, worded like test 1.
39. Skill text "in your project": "slow" about the person's own site, worded like test 4.
40. Skill text "in your project": "why did it do that" about the person's own code, worded like test 20.
41. Skill text "the harness's own": a Claude Code bug outside purlis. Plausible phrasing.
42. Skill text "the harness's own": a Codex bug outside purlis. Plausible phrasing.
43. Skill text "the harness's own": an opencode bug that happens with or without purlis. Plausible phrasing.
44. Skill text "the person's own mistake": a typo in a purlis command. Plausible phrasing.
45. Skill text "the person's own mistake": a wrong path the person set. Plausible phrasing.
46. Skill text "the person's own mistake": a setting they chose. Plausible phrasing.
47. Skill text "the person's own mistake": a git step the person forgot. Plausible phrasing.
48. Plausible phrasing: a purlis request with no fault in it (purlis-upgrade's kind).
