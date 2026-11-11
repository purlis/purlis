# Amendment, 2026-11-11: Always allow is offered as a rule, a harness's saved rule is not

Built for #6177 from [ADR 0121](../0121-attention-what-waits-on-you-reaches-you-and-nothing-else-interrupts.md)
decision 7, after [ask less, as built](2026-11-10-ask-less-as-built.md), which left two grants
unoffered until each had an undo.

- **A reviewed program's Always allow is a rule for this project.** It declares the program a
  tool of the persona's in the project's committed file (ADR 0112), so it is classed this
  project, and offered under the same terms as every rule: the same Allow twice within a week,
  the narrowest grant that would have prevented the asks suggested first (Allow for this chat
  before Always allow where one chat asked), and never on an ask marked risky. The offer shows
  the line Always allow writes before it is pressed, as the run's notice does (#5116).
- **Undo takes the line out through the writer that wrote it** (`settings::always_allow`): the
  same file, the same save, refused where the file moved since it was read. Only a tool that still
  holds the line exactly as written is taken out: one gone already is forgotten without an error,
  and one a teammate changed since is theirs, left as it is, and the rule forgotten. Where the
  person had allowed all of the persona's list, the Allow moves with the line, audited, as it moved
  when the line was written, so the rest of the list stays in force. The removal is a change to the
  committed file, read in its diff as the line was. The vault an Always allow may have given the
  persona is a grant of its own on Settings' Granted list, and stays; the chat that asked keeps the
  run grant it ran on until it ends. The rule is kept with the persona and the line
  (`rules.json`'s `declared`), and only where the persona's tools hold that line when it is made.
- **A risky ask is never offered a rule**, from any source: the core reads the row's risk mark
  (#6011), as `asksSame.ts`'s `highRisk` does, where 2026-11-10 left the seam saying no.
- **A harness's saved permission rule is not offered, on any harness.** purlis writes a harness's
  allow rule in one place, `purlis guard allow`, into the plane root's settings, and by design an
  allow rule never travels into a workspace's or a clone's generated settings (`layer.rs`: a grant
  copied sideways is a permission nobody clicked for), so it would stop the asks of a chat at the
  plane root alone. The window never saves a harness's own "from now on" rule either (V28c, and
  `hooked::ask` drops every option a harness would persist). A rule offered there would not
  prevent the asks it is offered for, so none is. Codex has no command rules to write, and
  opencode none on this machine alone. #6177's second criterion stays open on this ruling.
