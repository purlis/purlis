# Amended (2026-11-03, #4996; the person's ruling of 2026-11-03): a call naming another site asks to replace a recorded login

Ruled by the person. Until now `browser_login` naming a `site` for a login its persona already
declares kept the recorded site and was refused the same way again, and only an edit of the
project's settings by hand changed it.

- **The call raises a quick-approve notice instead.** It shows the site recorded now, the site
  proposed (followed to its form as any suggestion is), and the vault and key names, never a
  value. The chat's call waits for the answer.
- **The choices:** Allow once, Allow for this chat, Always replace, Keep the recorded site. Once
  and this chat write nothing. Always replaces the login's one line in the project's settings,
  showing the line first, and allows it on this machine. Keep leaves the recorded site and tells
  the chat plainly. Nothing changes without the person's press.
- **`sandbox.proposed-changes = "off"` holds Always back**, as it does for an added login.

*Decided in implementation (D-4996-1 to D-4996-4 in the pull request):* only the site moves, so a
call naming another vault or other keys is refused in words; a call naming the very site
recorded is no ask, and the declared login fills; Always swaps the line only while it still
reads as the notice showed it; a login the person allowed for the chat is not asked about again.
