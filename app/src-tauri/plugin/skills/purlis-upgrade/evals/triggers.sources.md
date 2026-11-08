# purlis-upgrade: where each trigger test came from

Read by the CI grader (spec #5714) beside `triggers.json`: the trigger bar, the neighbours whose
requests are the near misses, and one note per test in the order of `triggers.json`. A mined
session record is one of this project's `workspaces/smart-ide/sessions/` records, named by its file;
its situation is paraphrased and scrubbed of names, numbers that identify anything private, and
values. Session records are summaries, not transcripts, so no prompt is quoted word for word. A
filed miss Report replaces a plausible phrasing through a reviewed change.

Trigger bar: standard
Neighbours: purlis-repo-work, purlis-tidy-memory, purlis-theme

## Should start purlis-upgrade

1. Skill text: description, "when asked to update or upgrade purlis".
2. Skill text: description, "whether a newer purlis is published".
3. Skill text: "Say what the version brought".
4. Skill text: its name, as typed (it was purlis-update; Mined session record 20261108-085457-5724-four-built-in-skills-renamed-losing-nothing.md).
5. Skill text: `purlis update --channel stable    # or: dev`.
6. Skill text: description, "what a version added".
7. Skill text: description, "how to adopt a new purlis feature".
8. Skill text: "installing ends every running chat".
9. Skill text: "This skill is the version you are running".
10. Skill text: the release channel; plausible phrasing a person types.
11. Skill text: "`purlis update` is only the half about this plane's content"; a person's wrong guess.
12. Skill text: "relays what `purlis news` says each version brought".
13. Skill text: "Do not try to install it for them"; plausible phrasing a person types.
14. Skill text: "the status line at the bottom of the window offers it".
15. Skill text: "What this plane pins".
16. plausible phrasing a person types.
17. Mined session record 20261102-160949-skill-studio-docs-page-and-news-entry-2786-pr-48.md: a version's news entry; plausible phrasing a person types.
18. Skill text: "Say what the version brought"; plausible phrasing a person types.
19. Skill text: "Check now" in the update dialog.
20. plausible phrasing a person types.
21. plausible phrasing a person types.
22. Skill text: description, "how to adopt a new purlis feature".

## Should not start it

23. Near miss (purlis-repo-work): "upgrade" in a repo the workspace owns (spec #5714, the name collision behind the rename).
24. Near miss (purlis-repo-work): "update" in a repo.
25. Near miss (purlis-repo-work): "bump" in a repo.
26. Near miss (purlis-repo-work): "latest" in a branch folder.
27. Near miss (purlis-tidy-memory): "update" on memory.
28. Near miss (purlis-tidy-memory): its description.
29. Near miss (purlis-theme): "update" on a theme.
30. Near miss (purlis-theme): "new" purlis, but a look.
31. plausible phrasing a person types: "update" on a file.
32. plausible phrasing a person types: another program's upgrade.
33. plausible phrasing a person types: another program's update.
34. plausible phrasing a person types: another program's news.
35. plausible phrasing a person types: another program's upgrade.
36. plausible phrasing a person types: "release" in a repo's docs.
37. plausible phrasing a person types: another app's release.
38. plausible phrasing a person types: another program's update.
39. plausible phrasing a person types: "update" in code.
40. plausible phrasing a person types: another program's upgrade.
41. plausible phrasing a person types: unrelated.
42. plausible phrasing a person types: "refresh" in code.
43. plausible phrasing a person types: "what changed" in a repo.
44. plausible phrasing a person types: a harness's own update, not purlis's.
