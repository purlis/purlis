# purlis-permissions: where each trigger test came from

Read by the CI grader (spec #5714) beside `triggers.json`: the trigger bar, the neighbours whose
requests are the near misses, and one note per test in the order of `triggers.json`. A test
drawn from purlis's own code or docs names the file. "The harness's own settings" marks a real
request for a harness's own configuration (an env var, a theme, the model, a hook), which the
harness's config skill keeps (spec #5714, Q19). A filed miss Report replaces a plausible
phrasing through a reviewed change.

Trigger bar: strict
Neighbours: purlis-secrets, purlis-browser, purlis-repo-work

## Should start purlis-permissions

1. Spec #5714 user story 56, "stop asking me about this command".
2. Spec #5714 user story 56, "why was this blocked".
3. Spec #5714 user story 56, "allow this host".
4. Briefing's SANDBOXED_NOTE: a host it may not reach, and the Notice's choices.
5. Skill text: `purlis guard ask 'terraform apply *'`.
6. Skill text: `purlis guard allow`; plausible phrasing a person types.
7. Skill text: `purlis guard list`.
8. Skill text: `purlis guard allow ... --local`.
9. crates/purlis-core/src/sandboxblock.rs: a program's own "Operation not permitted" read as a block.
10. Briefing's SANDBOXED_NOTE: a blocked local socket (Docker's) may get a Notice the person can answer.
11. Briefing's SANDBOXED_NOTE: the level is the person's alone, in Settings › Project › Sandbox › Your level.
12. Briefing's SANDBOXED_NOTE: "If they say they lowered it and this chat is still sandboxed, do not guess why".
13. Notice's Always allow, "for a host only, for everyone in the project" (SANDBOXED_NOTE).
14. Briefing's SANDBOXED_NOTE: the Notice's three choices.
15. crates/purlis-core/src/guardcmd.rs: an allow rule reaches the plane root only.
16. Skill text: `purlis guard ask`; plausible phrasing a person types.
17. Skill text: section 3, a purlis guard refused it; crates/purlis-core/docs/hooks.md, the guards.
18. crates/purlis-core/docs/hooks.md: the branch-move guard; plausible phrasing a person types.
19. crates/purlis-core/src/sandboxblock.rs: Go's certificate check failing on the system service the sandbox keeps from it.
20. crates/purlis-core/src/sandboxblock.rs: an egress proxy's refusal; plausible phrasing a person types.
21. Skill text: `purlis guard allow` without `--local`, committed for everyone.
22. Skill text: `purlis guard report`.
23. app/src-tauri/src/hooks.rs BlockOffer::Listen: listening on a port of this Mac's loopback.

## Should not start it

24. Near miss (purlis-secrets): a credential a command needs, not a permission.
25. Near miss (purlis-secrets): its description, "when asked to store one".
26. Near miss (purlis-secrets): a declared tool run with a vault.
27. Near miss (purlis-secrets): the vault guard on reads; that skill says how to use one.
28. Near miss (purlis-browser): a website login is the `browser_login` tool.
29. Near miss (purlis-browser): a website login, not a sandbox host.
30. Near miss (purlis-browser): browser work.
31. Near miss (purlis-repo-work): committing from a branch folder.
32. Near miss (purlis-repo-work): pushing from a branch folder.
33. Near miss (purlis-repo-work): `purlis worktree add`.
34. The harness's own settings: an env var (its config skill's "set X=Y").
35. The harness's own settings: a theme (its config skill sends it to /config).
36. The harness's own settings: the model.
37. The harness's own settings: a hook (its config skill's "when claude stops").
38. The harness's own settings: keybindings.
39. The harness's own settings: the status line.
40. The harness's own settings: an editor preference.
41. plausible phrasing a person types: "permission" in code.
42. plausible phrasing a person types: "blocked" in code.
43. plausible phrasing a person types: "allow" a port in infrastructure.
44. plausible phrasing a person types: file permissions in a repo.
45. plausible phrasing a person types: "blocked" on a forge.
46. plausible phrasing a person types: "allowlist" in code.
47. plausible phrasing a person types: permissions in code.
