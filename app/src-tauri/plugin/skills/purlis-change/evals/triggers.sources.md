# purlis-change: where each trigger test came from

Read by the CI grader (spec #5714) beside `triggers.json`: the trigger bar, the neighbours whose
requests are the near misses, and one note per test in the order of `triggers.json`. purlis-change
is new, so no session record shows a person asking for it; most notes name the text a test
comes from (ADR 0060, `crates/purlis-core/docs/forges.md`, the skill itself). A filed miss Report
replaces a plausible phrasing through a reviewed change.

Trigger bar: standard
Neighbours: purlis-repo-work

## Should start purlis-change

1. ADR 0060's own example: "a core API bump, and the two repos that consume it".
2. Skill text: `purlis change create <slug> --why`; plausible phrasing a person types.
3. Skill text: `purlis change add` with `--needs`.
4. Description ("say which repo merges first"); ADR 0060 §3, ordering.
5. Skill text: `purlis change push`, which opens each request with the cross-link block.
6. Skill text: `purlis change land <slug> --repo <repo>`, one member per run.
7. Description ("what a change is waiting on"); `purlis change show`.
8. Skill text: `purlis change revert`; ADR 0060 §8.
9. Description ("one piece of work in a purlis workspace spans several repos").
10. The tool `change_status` with no change named; `purlis change list`.
11. ADR 0060's problem statement: "nothing says the three are one piece of work, which must land first".
12. Skill text §5; one member per run, in `needs` order.
13. forges.md `purlis change land`: each gate's refusal is named; plausible phrasing a person types.
14. Skill text: `purlis change drop <slug> <repo> --why`.
15. forges.md `purlis change push`: the cross-link block in every request.
16. forges.md: a queued member is recorded by a later `land` of it.
17. Skill text: `revert-<slug>` is pushed and landed like any change.
18. ADR 0060 §4, a partial landing shown with the outstanding members named.
19. Skill text: `purlis change land --squash`.
20. Skill text: `purlis change forget <slug>`.
21. Skill text: create, then add with `--needs`; plausible phrasing a person types.
22. The tool `change_status` with one change named.

## Should not start it

23. Near miss (purlis-repo-work): one repo's branch; `purlis worktree push`.
24. Near miss (purlis-repo-work): its description, "build, test, change and commit".
25. Near miss (purlis-repo-work): its description, "several branches of one repo at once".
26. Near miss (purlis-repo-work): its §2, `purlis clone`.
27. Near miss (purlis-repo-work): `purlis worktree commit` in one repo.
28. Near miss (purlis-repo-work): `purlis worktree update` in one repo.
29. Near miss (purlis-repo-work): its §3, a repo's own conventions.
30. Near miss (purlis-repo-work): one repo's request, no change.
31. Near miss (purlis-repo-work): a branch folder in one repo.
32. plausible phrasing a person types: "change" as a verb about the look.
33. plausible phrasing a person types: a task's Changes in the window, not a cross-repo change.
34. plausible phrasing a person types: a single-repo revert.
35. Mined from this repo's CLAUDE.md: `changes/<slug>.md` fragments; "change" as a file.
36. plausible phrasing a person types: one request's diff.
37. Mined session record 20260929-175343-repo-rename-security-alerts-and-the-48-issue-cle.md: a repo rename, no change.
38. plausible phrasing a person types: a handoff, not a change.
39. plausible phrasing a person types: "change" as a setting.
40. plausible phrasing a person types: an upgrade, not a change.
41. plausible phrasing a person types: an inventory, not a change.
42. plausible phrasing a person types: one branch's git history.
43. plausible phrasing a person types: a code edit in one file.
44. plausible phrasing a person types: `purlis save`, the project's own files.
