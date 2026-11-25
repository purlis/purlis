# Amendment, 2026-11-24: `policy why` and the dry run as built (M75 slice 18, #7027)

Spec §10.1 and §10.2 say `purlis policy why` answers for one action without running it, that a
chat may ask only about itself, and that the dry run is the loosening press's diff for a policy
that is not yet committed. Slice 18 built it, and settled what the text left open. Nothing here
reverses a ruling.

- **Asking changes nothing.** The host's decision seals what it sees tighten into the accepted
  floor and spends a Once grant. `policy why` calls the same decision with neither
  (`decide::How::Explain`, `force::judge_at`), so a Once grant is shown as serving and stays
  unspent, and the floor gains no version by being asked about.
- **The app answers, for the person and for a chat, because only it reads the sealed side.** The
  CLI process cannot read the sealed store (`Sealed::of` gives `NotTheApp`, so it reads as
  unverifiable with no entries): from it the dry run would diff against the shipped default and
  not the accepted copy and the accepted floor, `enforced_until_pressed` would be wrong, and a
  plain `why` would never show a grant or a never. So both questions go to the app over the hook
  socket. A chat's goes as `Ask::Why`, bound to the chat whose token the line carries, and is
  answered from the app's record of that chat and its floor, as the host's own decision is. A
  person's goes as `Ask::PersonWhy`, which names no chat and carries no token: the listener
  answers it only on two proofs, both required. The sender runs inside none of the app's chats
  (`ChatOf::InNone`), and the line carries a credential, a seal of the ask under a key derived
  from the plane's `fingerprint.key`, which no chat's sandbox reads. Ancestry alone is not proof
  (a process that double-forks and `setsid`s reads as in no chat), so an ask without the
  credential is refused with `PERSON_ONLY` whatever the ancestry says. `--policy` names a branch
  or a file that must lie inside the plane root (links followed), never one outside it. `--chat N` names a chat whose facts the app looks up in its
  own record, and an app that has no such chat open refuses. **Where no app answers**, the CLI
  makes the answer from the committed project alone, with the context the flags describe, and the
  answer says plainly that the sealed side was not read (`Why::sealed_unread`); `--chat N` is then
  refused with a reason, never answered from invented facts.
- **Whether the caller is in a chat is the app's word, not the environment's.** The refusal of the
  flags a chat may not name (`--chat` of another number, `--workspace`, `--persona`,
  `--unattended`, `--sandbox`, `--depth`, `--policy`) keys on `brokered::in_a_chat()`, which asks
  the app by the caller's ancestry as the guards do; a process that unsets `$CHARTER_CHAT` and
  `$CHARTER_SESSION_ID` is still in its chat, and an app that cannot answer leaves it one. A chat
  with no number in its environment may use no `--chat`.
- **"The permits a forbid overrode" are the permits that apply on their own** where what is
  enforced is not an allow (`Decision::overrode`, not on the wire, which an older hook parses). In
  the shipped default that is `policy-everything-else` under every forbid, since the catch-all
  permit applies to everything.
- **"The nearest level that could loosen it" is each level that holds a rule that decides.** A
  permit below a forbid cannot pass it (§4.2, deny wins at every level), so the level that wrote a
  deciding forbid must change or remove it; where no permit applies, the level nearest the chat
  may add one. A lock freezes every level below it, so only its own level and the ones above are
  named, and the lock itself is. Where several rules decide, each level named must change.
  Organisation changes need no press; every other level's wait for it.
- **The dry run answers as the policy would once accepted, and says what is enforced until then.**
  `--policy <branch>` reads the project's files at that commit (`Version::resolve_rev`,
  `at_sha`) and `--policy <file>` lays a `policy.cedar` over the project as committed (a name
  that is both is the branch; a relative file is the plane root's, and is read to at most 1 MiB).
  The answer says which commits it was computed for: `HEAD`, and the one the branch resolved to.
  It is that policy judged alone; because the policy in force takes the stricter side, a
  loosening still enforces the stricter outcome until the press, and the answer says so. The diff
  is `press::dry_run`, which takes its bases (`press::held_against`) and its rows, the shipped
  default's own change included (`press::name_the_shipped_default`), from the code the press
  uses, so the two share their rows. **They agree when the same sealed side is read**: the app
  reads it, and a CLI that could not says so instead of showing a diff against the shipped
  default.
- **An action the host cannot yet make a context for is refused as unaskable, by name.** The
  common context fields are the host's; the narrowing fields come with each family's slice, so
  `policy why` on an action that reads one answers with the reason the request does not fit the
  schema, as the host's own decision does, until that family's slice lands.
