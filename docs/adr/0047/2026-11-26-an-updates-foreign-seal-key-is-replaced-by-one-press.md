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
- **It replaces every item under `purlis-seal`, in an order safe to stop at any step**
  (`seal::renew`). The app lists the accounts there (names only, which the Keychain gives
  without asking). First it keeps the **new key at rest** in its own spare item
  (`purlis-seal/spare`), which the next read makes the key if the old one goes and the new one
  is not kept, and which a press cut short uses again. Then it writes the **list of replaced
  planes before any anchor goes**, sealed with the new key, beside the list before (two
  accounts, so neither write leaves a moment with no list). Then it deletes the spares, every
  anchor and the list before, deletes the old key **last**, and keeps the new key. Deleting
  another program's item is macOS's to allow, and it may ask the person: **that prompt is their
  consent**. Where it refuses (Deny, a wrong password, a locked keychain, or no prompt at all,
  which purlis cannot tell apart), nothing is accepted and the person is told *macOS did not let
  purlis replace its old key, so nothing was accepted*, with the manual step in **Keychain
  Access**, which can delete any item of theirs (never `security` in a terminal, which macOS may
  refuse as it refuses the app). The press can be made again, and no project reads as never
  accepted at any step (#7734's review, F2 and F6). The key's cache is not held across macOS's
  prompt; the keychain is read again once it returns (F7).
- **The digest is checked before, and again after the prompt returns.** A view already stale
  asks nothing of macOS; a file that changed while the prompt was open accepts nothing (the new
  key is kept). Then this plane's acceptance is sealed from exactly what the person was shown.
- **Every other plane that had an acceptance stays broken, never never-accepted** (the person's
  ruling). The planes the old key's anchors spoke for are kept in the keyring, at the accounts
  `seal-key-replaced` and `seal-key-replaced-next` under `purlis-seal` (Keyring, device-bound;
  `docs/plane-format.md`): each entry the anchor's account, `<kind>:<anchor>`, sealed with the
  **new** key, and added to by a later replacement. A plane on it reads as broken whether or not
  the seal holds, so no accepted deny drops silently to the shipped default. Only an entry purlis
  vouches for is the kept fact that the plane broke because purlis replaced its key
  (`sandbox::local::broken_by_a_new_key`, `PolicyState.new_key`), for the window's words: an
  entry of a list that holds under the new key, and that came from an anchor the keyring held,
  or from a list before whose seal holds under the old key or the new one. An entry carried from
  a list that holds under neither (another program's, or edited) is kept, marked `?`, and never
  vouched for (F5). A plane leaves the list when it is accepted again, a list whose seal does not
  hold included, and the item goes with its last plane (F6).
- **The nevers too** (F3). The anchors of a plane's nevers (`dispatch-nevers-live:<anchor>`, and
  `dispatch-nevers:<anchor>` from before the set) are kept on the same list. Such a plane's
  nevers read as unread, a file wiped since included, so no never drops silently, and the words
  say *purlis made a new seal key on this machine, so it cannot vouch for the list of pairs you
  said never to*, with what to do: review the file, then accept the project's policy again
  (`PolicyState.nevers_new_key`; Settings › Project › Policy offers Accept for it). The person's
  Accept on that project seals the nevers the file holds, as they stand, with the new key
  (`dispatchnever::keep_after_a_new_key`): nevers only tighten.
- **Tamper detection is unchanged.** A plane not on the list, or one edited after its
  acceptance under the new key, reads broken as before, with the existing words.
- **Where the key is the app's own, nothing is replaced**: the press is Accept.

**The window's part lands with #7719.** The prerequisite modal draws the press (and says
before it that macOS will ask for the login password and every project will need accepting
again); Settings › Project › Policy uses the same command later. Until then Accept's refusal
keeps the manual step.

**Declared gaps.**

- A record of a listed plane edited before it is accepted again cannot be told from one sealed
  with the old key, which is gone: it reads as broken because of the new key. Likewise a nevers
  file of a listed plane edited before the person's Accept is sealed as it stands then. Nevers
  only tighten, so a pair added is kept, and a pair taken out is the one the window asked the
  person to check for.
- A list deleted by another program (which macOS would ask the person to allow) lets a plane
  whose anchor went read as never accepted, and its nevers as none, as an anchor deleted does
  today.
- The dispatch answers sealed with the old key count as none, as an anchor taken away does:
  every grant is asked again, which only tightens.
- **Not measured yet: whether macOS lets the app delete another program's item with a prompt
  at all** (F1). The press is written for both answers (a refusal names Keychain Access), and the
  pull request carries a manual check on a signed build after an update.
