### Security

- **A dispatch grant is sealed, so it holds for a chat with no sandbox, and one a chat wrote
  counts as none.** Each grant you make, each acceptance of the project's `[dispatch.grants]`
  and each never carries its own seal, made by purlis's app with a key only it reads from the
  login keychain. A grant edited by hand or by a chat, copied from another clone, or taken
  back and put back from a copy counts as no grant, and Settings › Project › Dispatch shows it
  read-only; you grant the pair again through its ordinary ask. On macOS a chat with its
  prompts off and no sandbox now dispatches under a sealed grant instead of being refused; on
  Linux and Windows it is still refused, and says why. Grants and acceptances kept before this
  version are not carried over; your nevers are, sealed once (ticket T1.1-73, #6968).
- **A deleted or emptied nevers file is never "no nevers".** The keyring keeps a marker of the
  nevers the app sealed, so removing them refuses every dispatch across personas until the
  file is restored, under every `[policy]`, `open` included. A plane moved away with a link
  left at its old path keeps that marker. On a machine with no keyring purlis can use, no
  grant or never is kept, and Settings says so.

### Changed

- **Dispatch follows one order: lock, then your never, then the project's deny, then the same
  persona, then the project's allow, then your sealed grant, then an ask.** A project may deny
  a pair in its committed `[dispatch.denies]`, which no grant lifts. A deny is read from the
  last commit, as `[policy]` is: one taken out of the file still holds until you press Accept
  on the project's policy Notice, and a file purlis cannot read denies every dispatch across
  personas. A dispatched chat runs
  under its project's `[policy]` and never looser than the chat that asked. Under the
  `dispatch` family's `log`, a deny or an ask lets the dispatch go ahead and names the rule in
  the asking chat's enforcement log, as a logged finding of a brokered commit and a masking
  let go in a brokered `secret exec` now do too (#6968).
- **The `vaults` family switches.** `open`, or `vaults = "off"` accepted with your press, opens
  a registered vault to a chat of any persona; `watch`, `hold`, `team` and `strict` keep it
  enforced, and an administrator's lock on vault grants keeps it enforced in every mode.
