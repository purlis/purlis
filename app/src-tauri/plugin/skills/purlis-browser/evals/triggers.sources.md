# purlis-browser: where each trigger test came from

Read by the CI grader (spec #5714) beside `triggers.json`: the trigger bar, the neighbours whose
requests are the near misses, and one note per test in the order of `triggers.json`. A mined
session record is one of this project's `workspaces/smart-ide/sessions/` records, named by its file;
its situation is paraphrased and scrubbed of names, numbers that identify anything private, and
values. Session records are summaries, not transcripts, so no prompt is quoted word for word. A
filed miss Report replaces a plausible phrasing through a reviewed change.

Trigger bar: standard
Neighbours: purlis-secrets, purlis-theme

## Should start purlis-browser

1. Skill text: "log in with `browser_login`, and call it first"; description, "a web login".
2. Skill text: a login a project declares, `browser_login` with `login = "shop-login"`.
3. Skill text: "One browser per worker"; description, "several workers each need their own logged-in session".
4. Mined session record 20261105-045730-5121-browser-login-as-a-preset-of-secret-fill-pr.md: a login filled from a vault; plausible phrasing a person types.
5. Skill text: description, "an end-to-end check".
6. Skill text: "Any other secret goes in with `browser_fill_secret`", "an API key into a dashboard's settings".
7. Skill text: "A 2FA code comes from its seed".
8. Skill text: "a real browser the person can watch and take over"; description, "verifying a UI".
9. Skill text: description, "reaching something behind auth".
10. Skill text: a project declaring a login in `[sandbox.personas.qa]`.
11. Mined session record 20261103-192600-browser-login-form-host-and-replace-notice-4995-.md: a login on the wrong site is replaced.
12. Skill text: `browser_new`, one browser per worker; plausible phrasing a person types.
13. Skill text: "Files: downloads and uploads", `browser_save_download`.
14. Skill text: `browser_upload` into a page's file input.
15. Skill text: "In the person's own terminal", `PLAYWRIGHT_MCP_SECRETS_FILE`.
16. plausible phrasing a person types: two sessions on one site.
17. Mined session record 20261104-043321-5074-browser-login-and-profile-credential-misses.md: a console login from a vault; plausible phrasing a person types.
18. Skill text: "A different fixture account is a different pair of keys".
19. Skill text: "A page with a captcha or a second factor", hand them the browser with `ask_operator`.
20. Skill text: `browser_fill_secret`, "a token into a deploy form".
21. Mined session record 20261104-133221-5152-a-logged-in-browser-reaches-its-sign-in-hos.md: a logged-in browser reaching its pages.
22. Skill text: description, "several workers each need their own logged-in session"; Mined session record 20261105-130417-5296-the-browser-limit-names-what-s-open-a-kept-.md.

## Should not start it

23. Near miss (purlis-secrets): a credential a command uses (decisions.md of spec #5714 Q30).
24. Near miss (purlis-secrets): a credential an API call uses.
25. Near miss (purlis-secrets): its description, "when asked to store one".
26. Near miss (purlis-secrets): its description, "kubeconfig".
27. Near miss (purlis-secrets): a token for a command, not a page.
28. Near miss (purlis-secrets): its description, "SSH key".
29. Near miss (purlis-theme): its description, "from an image or screenshot".
30. Near miss (purlis-theme): "Checking a theme"; "check" and "verify" without a site.
31. Near miss (purlis-theme): its description, "a Figma frame".
32. Boundary (other browser skills, decisions.md of spec #5714 Q30): no login, one session; a plain negative.
33. Boundary (other browser skills): no login, one session.
34. Boundary (other browser skills): test code, no vault login.
35. Boundary (other browser skills): no login.
36. Boundary (other browser skills): no login.
37. plausible phrasing a person types: unrelated.
38. plausible phrasing a person types: browser words, no browser.
39. plausible phrasing a person types: "login" in code.
40. plausible phrasing a person types: auth in code.
41. plausible phrasing a person types: "session" in code.
42. plausible phrasing a person types: unrelated.
43. plausible phrasing a person types: "password" in code.
44. plausible phrasing a person types: unrelated.
