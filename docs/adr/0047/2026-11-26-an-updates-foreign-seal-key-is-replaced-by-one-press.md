# Amendment, 2026-11-26: a foreign seal key is replaced by one press (#7714)

**The problem it closes.** The seal's key (`purlis-seal`, account `answers`) and every plane's
anchor are Keychain items held to the build that made them, by its code signature (V90a). After
an update macOS treats as a different app (an ad-hoc build, or the first Developer ID build a
machine gets after ad-hoc ones), the app owns none of them. It finds the key foreign, every
plane's acceptance of `[policy]` reads as broken (every family enforced, every dispatch across
personas refused), and Accept refuses with the manual step. Pressing Accept never recovered it.

**Decision.**

- **One press, the window's alone: Make a new key and accept**
  (`enforcement::press_with_new_key`, the command `accept_project_policy_with_new_key`, on
  `WINDOW_ONLY` beside `accept_project_policy`). It is offered for **any** foreign key: an ad-hoc
  signature's origin cannot be proven, and the press trusts nothing old. No word of the window
  claims who made the key; it says this happens after an update that macOS treats as a
  different app. It acts on the plane it names, never the one in front when it lands.
- **It replaces every item under `purlis-seal`.** The app lists the accounts there (names only,
  which the Keychain gives without asking), then deletes the spares, every anchor and the old
  list of replaced planes, keeps the new list, deletes the key last, and keeps a new key of its
  own (`seal::renew`). Deleting another program's item is macOS's to allow, and it asks the
  person: **that prompt is their consent**. Where it refuses (Deny, a wrong password, a locked
  keychain), nothing is accepted and the person is told *macOS did not let purlis replace its
  key, so nothing was accepted*, with the manual step. The press can be made again: the key goes
  last, so a replacement cut short leaves it foreign, and the next press finishes it.
- **The digest is checked before, and again after the prompt returns.** A view already stale
  asks nothing of macOS; a file that changed while the prompt was open accepts nothing (the new
  key is kept). Then this plane's acceptance is sealed from exactly what the person was shown.
- **Every other plane that had an acceptance stays broken, never never-accepted** (the person's
  ruling). The planes the old key's anchors spoke for are kept in the keyring, at account
  `seal-key-replaced` under `purlis-seal` (Keyring, device-bound; `docs/plane-format.md`): a list
  sealed with the **new** key (a kind of its own), made before the key, and added to by a later
  replacement. A plane on it reads as broken whether or not the seal holds, so no accepted deny
  drops silently to the shipped default. Only a list whose seal holds is the kept fact that the
  plane broke because purlis replaced its key (`sandbox::local::broken_by_a_new_key`,
  `PolicyState.new_key`), for the window's words and the prerequisite list: a list edited, or
  made by another program, gives no plane those words. A plane leaves the list when it is
  accepted again, and the item goes with its last plane.
- **Tamper detection is unchanged.** A plane not on the list, or one edited after its
  acceptance under the new key, reads broken as before, with the existing words.
- **Where the key is the app's own, nothing is replaced**: the press is Accept.

**The window's part lands with #7719.** The prerequisite modal draws the press (and says
before it that macOS will ask for the login password and every project will need accepting
again); Settings › Project › Policy uses the same command later. Until then Accept's refusal
keeps the manual step.

**Declared gap.** A record of a listed plane edited before it is accepted again cannot be told
from one sealed with the old key, which is gone: it reads as broken because of the new key. A
list deleted by another program (which macOS would ask the person to allow) lets a plane whose
anchor went read as never accepted, as an anchor deleted does today. The
dispatch answers and nevers sealed with the old key count as none, as an anchor taken away does.
