# A pin's id is drawn again on read, and Regenerate approves every word of its command

**2026-11-10, spec #5948, from the adversarial review of #5978.** Amends §1, §4 and §6 of ADR
0122, and the amendment of 2026-11-09 on running Regenerate.

§1 says any change to a pin's folder or command is a new pin, and both approvals (a page's
script, and Regenerate's code) are kept against the pin's id. That id was read from
`pins.toml` as written. A teammate could change the command and keep the old id, and both
approvals carried over. Regenerate's approval covered only the code folder of the first word
naming a script, and words starting with `-` were passed over to find it, so
`node -e '<payload>' scripts/gen.mjs` resolved to the approved code and ran.

- **A pin's id is drawn again from its folder, command and working folder whenever the pins
  file is read** (`purlis_core::artifact::pins`). The id written on the line only names that
  line, so two edits of one line unioned by git are still one pin; it is never the pin's
  identity. A line whose id is not its own reads as a new pin, unapproved
  (`pin-changed` where an approval of the folder exists), and the next write puts its id right.
- **What Regenerate's approval covers is every word of the command, the program first, with
  the fingerprint of the code it resolves to** (`purlis_core::artifact::regenerate::of_run`).
  "What is approved is the code, not the command's text" now reads "the code and the command's
  text". A word passed over to find the script is still approved, so a command changed under
  an approved id asks again (`changed`) even if the id check were bypassed. Approvals recorded
  before this change ask once more, since what they hold no longer matches.
