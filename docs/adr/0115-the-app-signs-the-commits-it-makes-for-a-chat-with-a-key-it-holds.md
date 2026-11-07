# The app signs the commits it makes for a chat, with a key it holds

**Accepted 2026-11-06**, the person's ruling on #589 (issue #5480): keep `main`'s
`required_signatures` rule, and have the app sign the commits it makes for a chat, with a key it
holds, which never reaches the chat.

## Where purlis was

- **Since 2026-11-06 `main` refuses unsigned commits**, and GitHub's merge queue checks a pull
  request's own commits, not only the squash commit it makes. A pull request made of
  `purlis worktree commit` commits was refused at enqueue.
- **The app makes a sandboxed chat's commits** (ADR 0067 §2, #1055): the brokered commit,
  `purlis worktree update`'s rebase and its `--merge`. Each told git outright to sign nothing
  (`--no-gpg-sign`, `commit.gpgSign=false`), and a repository whose own config asked for a
  signature was refused, because a signer is a program that may ask for a passphrase with nobody
  there to type it (`crates/purlis-core/docs/git-policy.md`).

## The decision

1. **One Ed25519 key per device, in the Keyring tier** (ADR 0069): the item
   `purlis/@commit-signing/<device>`, holding the private key as OpenSSH writes one, with no
   passphrase. It is written by purlis's own binary and held to the app, as the link key is
   (ADR 0078). purlis makes it, or takes one of the person's own Ed25519 keys and asks its
   passphrase once, in their terminal (`purlis signing setup`).
2. **git's own SSH signature format, made in the app's process.** The app signs the commit
   object with an `SSHSIG` in the namespace `git` over SHA-512, exactly what git's SSH signing
   (`gpg.format = ssh`) puts in a commit's `gpgsig` header, using RustCrypto's `ssh-key`. No
   signer program runs, so the runner's rule that the git purlis starts runs no program a
   repository names stays whole, and the key is never written to a file.
3. **git commits unsigned, and the app signs what git made.** After git writes the commit (or
   the rebase replays, or the merge commits), the app writes a signed copy of each commit it made
   (same tree, parents mapped to their signed copies, author, committer and message, byte for
   byte) and moves the branch to it only from where git left it. A rebase signs the commits after
   the base it replays onto; a merge signs its merge commit alone, so a commit already pushed is
   never rewritten.
4. **With no key, the commit is made all the same**, and the answer says so: a repository that
   requires signed commits will refuse a pull request with it, and `purlis signing setup` fixes
   it. Signing never makes a commit fail.
5. **The person's own git is in charge outside the app's brokered git**: from a terminal or an
   unsandboxed chat, nothing changes.
6. **Setup is three plain steps** (`purlis signing setup`): the key; GitHub told it is the
   person's, with `gh api` where `gh` may add signing keys and by hand otherwise; and a check that
   GitHub lists it. `purlis signing check <owner/repo> <commit>` asks GitHub whether a pushed
   commit shows as Verified, and says what to do if not.

## Alternatives weighed

- **git's SSH signing with a key file and `ssh-keygen` as the signer.** The most familiar form,
  but the private key would sit in a file the app writes, which every sandboxed chat's read rules
  would have to deny, and git would start `ssh-keygen` from a folder a chat wrote. Rejected: this
  decision produces the same signature without the file or the program.
- **purlis itself as git's SSH signing program.** git would call back into a purlis binary for
  every commit, which would read the keychain from another process (a prompt on macOS) and add a
  program the runner has to trust.
- **An `ssh-agent` the app runs.** A socket a chat could find and ask to sign anything.
- **OpenPGP (GnuPG) or X.509 (gitsign).** GnuPG is a daemon with prompts of its own; gitsign's
  keyless signing needs a browser sign-in per session. Both are more to set up for a person who
  is not advanced, for the same Verified badge.
- **Commits made through GitHub's API**, which GitHub signs itself. It would replace git for
  every commit a chat makes, and work only on GitHub.

## What it costs

- A commit's id changes once it is signed: the answer names the signed id, and the unsigned
  object git wrote first is left for git's own clean-up.
- GitHub shows a commit as Verified only where the committer's email is a verified address of
  the account the key is on. Setup says so, and cannot check it without more of `gh`'s scopes.
- GitLab reads the same signature; setup's check against GitLab is not made yet.

## Later

- **2026-11-07, ADR 0116**: setup moves into the window for people who never open a terminal,
  adds the key through the app's forge account rather than `gh`, and covers GitLab as fully as
  GitHub, its check included. This ADR's signing itself is unchanged.
