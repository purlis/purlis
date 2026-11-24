# Amendment, 2026-11-24: `policy why` and the dry run as built (M75 slice 18, #7027)

Spec §10.1 and §10.2 say `purlis policy why` answers for one action without running it, that a
chat may ask only about itself, and that the dry run is the loosening press's diff for a policy
that is not yet committed. Slice 18 built it, and settled what the text left open. Nothing here
reverses a ruling.

- **Asking changes nothing.** The host's decision seals what it sees tighten into the accepted
  floor and spends a Once grant. `policy why` calls the same decision with neither
  (`decide::How::Explain`, `force::judge_at`), so a Once grant is shown as serving and stays
  unspent, and the floor gains no version by being asked about.
- **A person's answer is read here, a chat's by the app.** A person's `purlis policy why` runs in
  the CLI over the plane it is in, with the context its flags describe (a chat that is attended,
  at the floor, no further than the person's own, unless a flag says otherwise). It holds no
  floor of the app's, so it cannot say what the app's merge hold adds; a chat's question goes to
  the app (`Ask::Why`, `Answer::Why`) and is answered from the app's record of that chat and its
  floor, as the host's own decision is. The line names the action and its resource and nothing
  else, so a chat cannot ask as another; the CLI also refuses `--chat` of another number,
  `--workspace`, `--persona`, `--unattended`, `--sandbox`, `--depth` and `--policy` inside a chat,
  naming the flag.
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
  `--policy <branch>` reads the project's files at that commit (`Version::at_rev`) and `--policy
  <file>` lays a `policy.cedar` over the project as committed (a name that is both is the file).
  The answer is that policy judged alone; because the policy in force takes the stricter side, a
  loosening still enforces the stricter outcome until the press, and the answer says so. The diff
  is `press::dry_run` over the same bases as the press (`press::held_against`), so the two cannot
  drift apart.
- **An action the host cannot yet make a context for is refused as unaskable, by name.** The
  common context fields are the host's; the narrowing fields come with each family's slice, so
  `policy why` on an action that reads one answers with the reason the request does not fit the
  schema, as the host's own decision does, until that family's slice lands.
