# Amended (2026-11-04, spec #5100, built from #5115; the person's rulings of 2026-11-03 on #5096): a chat asks purlis to fill a vault secret into a field

Ruled by the person (SF-1 to SF-7), with the steward's decisions (SF-8 to SF-13). Until now a
chat could put a vault value into a page only through `browser_login`, a user name and a
password, and anything else it would have had to type itself, which puts the value into the
conversation. Now any vault value can go into any browser the chat is using, under the same
control as a login.

- **What is approved is one vault key for one exact origin** (SF-1): scheme, host and port. The
  notice also shows the field's label and the page, but any field on that site may receive it.
- **Any browser the chat is using** (SF-2), as long as the engine sees, at the moment it fills,
  that the document of the field's own frame is on the approved origin; where it isn't, nothing
  is filled and the refusal says why (SF-13). The value is kept out of everything the chat reads
  in that browser, for the browser's life.
- **After the fill** (SF-3, SF-9): while the page holding the value is open, the browser reaches
  only the approved site, and copy, cut and drag out of the field are blocked; a new document in
  the main frame, or the tab's close, ends the hold, and a same-document route change keeps it.
  The back-forward cache stays off.
- **Kinds** (SF-4, SF-11): any vault value, and 2FA codes worked out from a stored seed at the
  moment of filling, the seed approved per site and never in the page. A card or payment value
  is offered Once only.
- **Fields** (SF-5): a one-line input field takes the value as approved; a multi-line text box
  or an editable area asks the person again, each time.
- **Choices** (SF-6): Allow once, Allow for this chat, Always allow (one key → origin line,
  which `sandbox.proposed-changes = "off"` holds back), Keep blocked.
- **`browser_login` becomes a preset of secret fill** (SF-7), in the browser the chat names or
  uses; one that already holds the same login answers "already logged in", and a new browser is
  opened only to keep a login for reuse, saying why.
- **The person's view** marks a filled field "secret: <key>" and the activity line names the key
  and the site, never a value (SF-8); no fill goes into a browser the person has taken over
  (SF-10); the same rules hold on a runner's browsers (SF-12).

*Built so far (#5115):* the tool `browser_fill_secret`, which names a field from the last
snapshot, a vault and a key; the browser's read of the field for the notice, and the fill
itself, at session protocol 1.72, which a host before refuses; the notice with Allow once and
Keep blocked; one-line fields alone, any other refused; and the value kept out for the
browser's life. The rest of the rulings are #5116 to #5122.

*Decided in implementation (D-5115-1 to D-5115-10 in the pull request):* the fill runs under the
agent's own lease, as one of its acts, so a take-over cuts it; the field's kind and label are read
in purlis's own world; the value is written into that one element by a function in purlis's own
world, which checks in the same step that the element's document is still on the approved
origin, so no focus and no moment between a check and a write decides where it goes; a field
that doesn't keep the value whole is emptied and nothing is filled; from a fill on, the agent
copies and cuts nothing in that browser, for its life; a chat can't send the field read itself; a
runner's browser fills nothing yet; a field in a frame of another site than its page's is warned
of; Keep blocked on a key and a site isn't asked again in that chat, and two end the chat's fill
notices; a press is audited as `secret-fill`, naming the vault, the key and the origin; the
person's view of the browser says what was filled where, and why a fill was refused.
