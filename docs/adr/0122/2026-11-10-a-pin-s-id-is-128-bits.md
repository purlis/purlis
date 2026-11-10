# A pin's id is 128 bits

**2026-11-10, spec #5948, from the second adversarial review of #5978 (finding 3).** Amends §1
and §4 of ADR 0122, and the amendment of the same day that draws a pin's id again on read.

A pin's id stands for its command in both approvals: a page's script and Regenerate's code are
approved under it. It was the first twelve hex digits of a SHA-256, 48 bits, so a second
preimage (a command of an author's choosing with the approved command's id) is a search a GPU
finishes. Every word of the command is in Regenerate's fingerprint, but the page's approval
holds only the pin's id and the template's fingerprint.

- **A pin's id is the first 32 hex digits, 128 bits, of the same SHA-256**
  (`purlis_core::artifact::pins::id_of`, `ID_BYTES`). It is shown whole: `artifact list` prints
  it, and a pin is still named by its folder as well as its id.
- **What was recorded before reads as changed, never as approved.** A `pins.toml` written with
  twelve-digit ids reads as it is, since every id is drawn again on read, and its next write puts
  the ids right. An approval of a page or of Regenerate recorded under a twelve-digit id is kept
  and read as `pin-changed`, so the bar asks once more and names who changed the pin; neither
  approval stands for an id that is not 32 digits.
