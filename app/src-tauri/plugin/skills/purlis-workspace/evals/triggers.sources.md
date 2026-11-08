# purlis-workspace: where each trigger test came from

Read by the CI grader (spec #5714) beside `triggers.json`: the trigger bar, the neighbours whose
requests are the near misses, and one note per test in the order of `triggers.json`. A mined
session record is one of this project's `workspaces/smart-ide/sessions/` records, named by its file;
its situation is paraphrased and scrubbed of names, numbers that identify anything private, and
values. Session records are summaries, not transcripts, so no prompt is quoted word for word. A
filed miss Report replaces a plausible phrasing through a reviewed change. Source files named
(`main.rs`, `brokered.rs`, `briefing.rs`, `live.rs`, `restore.rs`, `select.rs`) are purlis's own,
under `crates/`.

Trigger bar: strict
Neighbours: purlis-safe-remove, purlis-tidy-memory, purlis-handoff, purlis-repo-work

## Should start purlis-workspace

1. Skill text: description, "make, create or start a new workspace"; the briefing's workspace gate (briefing.rs, "create a new workspace").
2. Skill text: "Create", repos named after the name (`purlis workspace create <name> <repo>`).
3. Skill text: "always with a vision"; the briefing's "also ask what it's for".
4. Ticket #5732: "fork this".
5. main.rs: `workspace fork` has the alias `duplicate`; a real workspace of this project.
6. Skill text: "Fork", `--restore`.
7. Ticket #5732: "share this workspace".
8. Ticket #5732: "publish my memory".
9. Skill text: "Live and `purlis save` publish"; a real workspace of this project.
10. live.rs: `workspace live --off` makes it LOCAL.
11. Mined session record 20261107-104458-pr-5591-5551-review-fixes-volume-trash-on-linux-.md: workspace rename; plausible phrasing a person types.
12. Skill text: "Rename"; plausible phrasing a person types.
13. Skill text: description, "rebuild a workspace from its manifest or on another machine".
14. restore.rs: "committed precisely so a teammate can restore somebody else's workspace".
15. Skill text: "snapshot"; main.rs: "Capture this workspace's repos and branches into its committed manifest".
16. Skill text: "Run `purlis workspace snapshot <src>` first".
17. main.rs: `workspace default`, "the workspace a session lands on".
18. select.rs: `workspace default --clear`.
19. main.rs: `workspace use`, "Select a workspace for this terminal and session".
20. Skill text: description, "save and push the plane"; briefing.rs: "Commit + push it with `purlis save`".
21. briefing.rs: the uncommitted memory line, "Commit + push it with `purlis save`".
22. brokered.rs: create_refused, `--live` and `--use` refused in a sandboxed chat; the rule the skill carries.
23. main.rs: fork is "pre-loaded with its charter, memory, todos and manifest".
24. Skill text: "Create", `--use`; plausible phrasing a person types.

## Should not start it

25. Near miss (purlis-safe-remove): removal belongs to safe remove (ticket #5732, "Not for removing a workspace").
26. Near miss (purlis-safe-remove): purlis-safe-remove's description, "without losing what it learned".
27. Near miss (purlis-safe-remove): purlis-safe-remove's description, "retire".
28. Near miss (purlis-safe-remove): purlis-safe-remove's description, "archive".
29. Near miss (purlis-tidy-memory): ticket #5732, "tidying memory (tidy-memory)".
30. Near miss (purlis-tidy-memory): purlis-tidy-memory's description, "prune".
31. Near miss (purlis-tidy-memory): purlis-tidy-memory's description, "fold durable lessons into workspace.md".
32. Near miss (purlis-handoff): ticket #5732, "moving work to another chat (handoff)".
33. Near miss (purlis-handoff): purlis-handoff's description, "open or start a chat for something".
34. Near miss (purlis-handoff): purlis-handoff's description, "move work to another workspace".
35. Near miss (purlis-repo-work): "fork" of a repo, not a workspace.
36. Near miss (purlis-repo-work): purlis-repo-work's branch folders (`purlis worktree`).
37. Near miss (purlis-repo-work): "commit and push", in a repo rather than the plane.
38. Near miss (purlis-repo-work): "rename", of a branch.
39. plausible phrasing a person types: "rename" and "workspace" in code.
40. plausible phrasing a person types: another tool's workspace.
41. plausible phrasing a person types: someone else's workspace.
42. plausible phrasing a person types: "workspaces" in a build tool.
43. plausible phrasing a person types: "fork" in code.
44. plausible phrasing a person types: "restore" in data.
45. plausible phrasing a person types: "save" in an editor.
46. plausible phrasing a person types: "publish" a package.
47. plausible phrasing a person types: "share" on a forge.
48. plausible phrasing a person types: "snapshot" in infrastructure.
