# purlis-secrets: where each trigger test came from

Read by the CI grader (spec #5714) beside `triggers.json`: the trigger bar, the neighbours whose
requests are the near misses, and one note per test in the order of `triggers.json`. Sources: the
skill's own text, `decisions.md` of spec #5714, and phrasings a person plausibly types. No mined
transcript yet; a filed miss Report replaces a plausible phrasing through a reviewed change.

Trigger bar: strict
Neighbours: purlis-browser, purlis-persona, purlis-working-in-a-clone, purlis-handoff, purlis-compact

## Should start purlis-secrets

1. Skill text: description ("a database password") and "Use one: pick an injection path"; phrased as a person would.
2. Skill text: the STRIPE_KEY example under "A credential you lack"; plausible phrasing.
3. Skill text: description ("when asked to store one") and "Store one".
4. Skill text: description ("kubeconfig") and the `--file KUBECONFIG=` example.
5. Skill text: "Find out what exists" (`purlis secret list <vault>`); plausible phrasing.
6. Skill text: description ("SSH key"); plausible phrasing.
7. Skill text: the `--replace "the API answers 401"` example.
8. Skill text: "A database client" (`--env DSN=<key> -- usql "$DSN"`).
9. Skill text: "A credential you lack: run the command, or raise a Secret ask" (the SHOP_TOKEN example).
10. Skill text: "To check that one is present" (compare two masked lines).
11. Skill text: "As a file" (`purlis secret cp`, certificate).
12. Skill text: "Naming a program purlis has reviewed (curl)"; plausible phrasing.
13. Skill text: "Store one: the value never goes on the command line"; decisions.md Q28(h): never ask for the value in the chat.
14. Skill text: "A vault's own token is the person's to give" (New vault with 1Password).
15. Skill text: "To check that one is present"; plausible phrasing with a crowd neighbour's words (aws).
16. Skill text: "Working as a persona" (`purlis persona secret exec`).
17. decisions.md Q3: strict tier because a miss leaks a secret; plausible phrasing.
18. Skill text: description ("API token"); plausible phrasing.
19. Skill text: "A value that doesn't work" (replace in the window); plausible phrasing.
20. Skill text: "A database client" (`redis-cli`).
21. Skill text: "Working as a persona" (`purlis persona secret list`).
22. Skill text: description ("server login"); plausible phrasing.
23. Skill text: "use a secret, never reveal it"; plausible phrasing.
24. Skill text: description ("before running any command that requires a credential"); plausible phrasing.

## Should not start it

25. Near miss (purlis-browser): a website login, which purlis-browser's description claims.
26. Near miss (purlis-browser): verifying a UI behind auth, from purlis-browser's description.
27. Near miss (purlis-browser): "several workers each need their own logged-in session".
28. Near miss (purlis-persona): dispatching work to another persona.
29. Near miss (purlis-persona): "create a persona", from its description.
30. Near miss (purlis-persona): this chat needs the answer (decisions.md Q30, handoff and persona boundary).
31. Near miss (purlis-working-in-a-clone): build, test, change, commit in a repo; decisions.md Q28(b).
32. Near miss (purlis-working-in-a-clone): real work inside a repo a workspace owns.
33. Near miss (purlis-handoff): moving the work to a new chat the person will read; mentions a database.
34. Near miss (purlis-compact): memory tidying that mentions a vault (decisions.md Q30, tidy-memory boundary).
35. Plausible phrasing: plain coding work (the scoring module's recorded not-started case).
36. Plausible phrasing: a question about tokens that needs no credential.
37. Plausible phrasing: password code that needs no stored secret.
38. Plausible phrasing: environment variables with no value in them (a crowd skill's kind).
39. Plausible phrasing: unrelated coding question.
40. Plausible phrasing: a throwaway test value, never stored.
41. Plausible phrasing: security review of code, no credential used.
42. Plausible phrasing: documentation about a key, no key handled.
43. Plausible phrasing: unrelated coding request.
44. Plausible phrasing: unrelated data task.
45. Plausible phrasing: auth in the abstract, nothing to run.
46. Plausible phrasing: unrelated coding request.
47. Plausible phrasing: wrapping up, nothing secret (purlis-smart-close's kind).
48. Plausible phrasing: unrelated maintenance request.
