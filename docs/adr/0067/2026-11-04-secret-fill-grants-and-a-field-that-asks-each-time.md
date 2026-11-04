# Secret fill: the four answers, cards, a missing key, and a field that asks each time

*Amends ADR 0067. Builds on the amendment of 2026-11-04 that records rulings SF-1 to SF-13
(#5096, spec #5100) and what #5115 built.*

*Built (#5116):* the secret fill notice offers the sandbox's four answers (SF-6). Allow for this
chat holds that key on that origin for the chat's life, in memory. Always allow writes one
secret-fill line in the persona's tools, `<name> = { vault = "<vault>", key = "<key>", fill =
"<origin>" }`, shown on the notice before the press and allowed on this machine in the same press,
held back where `sandbox.proposed-changes = "off"` and wherever a browser login's Always is
(`docs/plane-format.md`). A line marked `card = true`, by any persona, makes that vault's key a
card's on every origin: Allow once and Keep blocked alone (SF-4). A key the vault doesn't hold
goes, once the fill is allowed, to the Secret ask (ADR 0109), and the call fills what the person
adds.

*Built (#5120):* a multi-line text box or an editable area of the page receives a secret only
after the person answers a notice for that fill, whatever was allowed for the chat or always
(SF-5), and the notice says that text in such a field is often posted or published. Such a notice
offers Allow once and Keep blocked alone. The engine reads the field's kind again as it fills: a
fill nobody was asked about for a multi-line field is refused, so a field that became one between
the read and the fill is caught.

*Decided in implementation (D-5116-1 to D-5120-3 in the pull request):* the line is a tool of its
own kind that runs no program and holds no value a run is handed; a card's mark is read across
every persona's lines; a press is judged again as the project reads at the moment of pressing and
drawn again where a card's mark or the line changed.
