# Amended (2026-11-04, spec #5100, built from #5116 and #5120; the person's rulings SF-4, SF-5 and SF-6 on #5096): secret fill's grants, cards, and a field that asks each time

Builds on the amendment of the same day that records rulings SF-1 to SF-13 and what #5115
built.

*Built (#5116):* the secret fill notice offers the sandbox's four answers (SF-6).
- **Allow for this chat** holds that key on that origin for the chat's life, in memory.
- **Always allow** writes one secret-fill line in the persona's tools:
  `<name> = { vault = "<vault>", key = "<key>", fill = "<origin>" }`. The line is shown on the
  notice before the press and allowed on this machine in the same press. It is held back
  wherever a browser login's Always is: `sandbox.proposed-changes = "off"`, a project with no
  `[sandbox]`, a persona whose list waits for an Allow, or policy that removes the scope
  (`docs/plane-format.md`).
- **Cards:** a line marked `card = true`, by any persona, makes that vault's key a card's on
  every origin. It is offered Allow once and Keep blocked alone (SF-4).
- **A missing key:** where the vault doesn't hold the key, the fill goes to the Secret ask (ADR
  0109) once the person allowed it. The call then fills what the person adds.

*Built (#5120):* a multi-line text box or an editable area of the page receives a secret only
after the person answers a notice for that very fill, whatever was allowed for the chat or always
(SF-5). The notice says that text in such a field is often posted or published, and it offers
Allow once and Keep blocked alone. The fill carries that answer to the engine. The engine reads
the field's kind again as it writes, in purlis's own world and in the same step as the write. A
fill nobody was asked about as multi-line is refused where the field is multi-line by then, so a
field that changed between the read and the fill is caught.

*Decided in implementation:*
- **The line is a tool of its own kind.** It runs no program and holds no value a run is handed.
  It is never armed for a chat (no shim, no `secret exec`, no runner tool), and is refused if it
  is ever matched as one. Rejected: a table outside `tools`, since the spec puts the line beside
  the login line; and handing the key as a value, as a login line does, which would be one more
  road to a run.
- **A card's mark fails closed.** It is read from the project's file as written: a faulty line
  marked `card` still marks, and so does a project whose sandbox is off. It counts across every
  persona, every origin and every spelling of the names, and through a linked entry either way.
- **A grant holds only an `https` site open.** A site on this machine (`http://localhost` and the
  like) is served by whatever listens on its port, which may be a program the chat started. Each
  fill there is offered Allow once and Keep blocked alone.
- **A browser on a pool's runner is asked about each time**, even under a grant: a grant here
  holds this machine's browsers, and the notice warns that the value goes to that runner (#5122).
- **A press is judged again as the project reads at that moment.** Allow once and Allow for this
  chat are drawn again where a card's mark changed. Always is drawn again where anything it would
  write changed, and is refused where policy took vault grants away since.
- **The name Always gives the line** is the key's own, then `-2`, `-3` and on where that name is
  taken. It is never a name other settings read as a forge's program (`gh`, `glab`, `git`).
- **A key missing from the vault is asked for after the person allows the fill**, so no one is
  asked to add a secret for a fill they then keep blocked. The call waits on that ask for what is
  left of the time a fill waits on the person. The fill is judged again once the key is added.
- **A value is kept out as an address or a form would spell it**, percent-encoded and
  form-encoded as well as whole. A page that puts it into its address shows the agent none of it.
- **`multi_line` rides on `fill_secret`** with a default of false and no new protocol minor: a
  host that doesn't read it refuses every multi-line field, so it fails closed.
