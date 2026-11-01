# Amended (2026-11-02, #4595; the person's rulings of 2026-11-01, which amend PC-1 and PC-5 of #1975 for this one case): a chat suggests a browser login and the person confirms its site

Ruled by the person. Until now a browser login had to be declared by hand and allowed apart. Now
`browser_login` on a login the persona's file doesn't list may suggest one, and that raises one
notice instead of failing. The chat's call waits for the answer. On a runner it can't wait across
the link: it's told once the person answers, and its pool opt-in (ADR 0089) is still asked.

- **The choices are the sandbox's own:** Allow once, Allow for this chat, Always allow, Keep
  blocked. Once and this chat write no file. Always writes the one line to the project's settings
  and allows it on this machine in the same press, the line shown first. It's a narrow typed
  route, a browser login line and nothing else. `sandbox.proposed-changes = "off"` holds Always.
- **The site is suggested by the chat and confirmed by the person.** The notice shows it large and
  editable, with the vault and key names and never a value. https only. What the person confirms
  is the exact origin filled: the page is rebuilt from it, and a press names the digest of what was
  drawn. A site that isn't what they typed is drawn again.
- **A key on another site is warned of plainly.** The choices stay the person's.
- **A key a `mint` line names is never offered.**

*Decided in implementation (D-4595-1 to D-4595-10 in the pull request):* the notice offers no
address, no bare name and no name on this machine, which are still declared by hand. A login
allowed from its notice fills in a new browser of its own and keeps no sealed login. Always is
offered only while the persona's list is empty or already allowed here, so its Allow carries
nothing nobody saw, and only in a file that has a `[sandbox]`. Once and this chat let this one
login read a vault its persona isn't tagged for, with a warning, and Always allows that vault as
the vault notice does. Two Keep blocked end a chat's login notices.
