# The window sets up commit signing, and adds the key through the forge account

**Accepted 2026-11-07**, the person's rulings in the commit-signing grill (Q1–Q27). It builds on
ADR 0115, which made the app sign the commits it makes for a chat, and changes only how a person
who never opens a terminal gets there. The prototype is on the branch `prototype/commit-signing`
(`app/src/CommitSigning.prototype.html`), with its verdict in the file's header.

## Where purlis was

- `purlis signing setup` is the only way to make the key, and it refuses to run in a chat, so a
  person has to open a terminal. It tells GitHub about the key through the person's own `gh`,
  which someone who is not advanced does not have.
- The app's GitHub sign-in (ADR 0077) asks for `repo read:org`, which cannot add a signing key.
  Its GitLab sign-in asks for `api`, which can.
- A forge that refuses unsigned commits shows up in the window as a plain "cannot merge", with
  nothing that says why or what to do.

## The decision

1. **Signed commits are opt-in.** Nothing makes a key or signs until the person sets it up.
   "On" means this device has a **signing key**: setup turns it on, and Remove is the only way
   off, so the window never shows a switch that can disagree with the key. While it is on,
   chat commits are signed in every repo, not only the ones that require it.
2. **Setup runs in the window, behind the person's own click.** Chats still never reach the key,
   and `purlis signing setup` still refuses inside one. A chat that meets the refusal asks the
   person to do it. The CLI stays as it is for anyone who wants it, including `--key` for a key
   the person already has.
3. **The ways in, in order:** the forge's refusal, shown where it happened (on the lane's card
   or in the chat) as "<Forge> only accepts signed commits here" with **Set up signing**; one
   grouped notice in the notices centre and the needs-you indicator in the title bar, however
   many lanes it holds; and the **Signed commits** card in Settings › You › Accounts. There is
   no step for it in onboarding. Before any refusal, an unsigned chat commit gets only a quiet
   line in the chat.
4. **Setup is one three-step dialog**, the same from every way in: *Make your key* (the person
   is told just before macOS asks to let purlis use the Keychain, and to choose Always Allow),
   *Add it to <forge>*, and *Check*. A step that needs nothing from the person runs on by
   itself. It ends on "Verified on <forge> ✓" and, where it was opened from a refusal, turns
   into **Sign and retry**, which re-signs the branch's commits (ADR 0115 §3) and sends the
   work again; from the notice, it does so for every waiting lane.
5. **The app adds the key through the forge account**, not through `gh`:
   - **GitHub**: setup asks the account for `admin:ssh_signing_key` and `user:email` at that
     moment, never at sign-in. If the person says no, the key is copied, GitHub's page for a new
     signing key opens, and the window carries on once the key is there.
   - **GitLab**, gitlab.com and self-managed alike: the account's `api` already allows it, and the
     key is added with the usage type *Signing* only. A server too old to verify SSH signatures
     says so, and setup is not offered there.
   - The key goes to the account bound to the repo that needed it. Each other account is one
     **Add to this account** on the card.
6. **Verified means the forge said so.** The check makes the forge confirm a signed commit
   as Verified: listing the key is not enough. If the committer email is not a verified address
   of that account, the last step offers the account's verified addresses to sign chat commits
   as.
7. **One key per device, never exported**, and added to as many accounts on as many forges as
   needed. **Replace key** makes a new one; **Remove signing** deletes it from the Keychain and
   from every account it was added to, after one confirmation. The card lists the account's
   purlis signing keys from the person's other devices, each with **Remove**, for a lost or
   wiped machine.
8. **Plain words.** The window says *Signed commits*, *Set up signing*, *signing key*,
   *Verified on GitHub ✓* or *Verified on GitLab ✓* and *Sign and retry*. SSH, Ed25519 and gpg
   appear only under the card's Details. Saving's own "Sign commits" switch, which is about a
   save's commits and the person's own git, is renamed so the two are not confused.

## Alternatives weighed

- **On by default.** A key the forge does not know makes "Unverified" commits, which look worse
  than unsigned ones, and setting it up means a permission screen before the person knows what
  it is for.
- **Sign only in the repos that require it, or a switch per repo.** More to understand for no
  gain to someone who is not advanced.
- **Setup in place**, inside the refusal or the card, with no dialog. It looked different in
  each of the three places it opens from, and a lane's card is too narrow for it.
- **A checklist that runs by itself.** Its runs-on-its-own steps were kept, but a step rail reads
  better when the person has to stop and choose.
- **One key synced between devices.** Something to back up and protect; a new key per device
  needs neither.
- **Asking for the signing-key permission at sign-in.** Every person would be asked, for a
  feature most never need.

## What it costs

- A second GitHub consent screen for the people who set signing up.
- How GitLab words its refusal of unsigned commits has to be read from a real server with that
  push rule before the window can recognise it. Until then GitLab's refusal keeps its plain
  wording, with a **Set up signing** link while signing is not set up.
- On builds without a stable code signature, macOS asks again for the Keychain after each
  update; the sentence before that prompt stays until it does not.
