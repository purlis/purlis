# Amended 2026-10-12: purlis's own git credential helper, for git the window runs (#1810, Q14)

The operator ruled, in the onboarding grill of 2026-10-12 (spec #1810, Q14): *"purlis's own git
credential helper, used only for git that the window runs, never for chats. Amends V22c
(#752)."* This replaces ruling 6 above (*"No purlis git credential helper yet"*) and §4's *"Git's
credential path is not part of the seam … none is added now"*. The text above is left as
accepted.

**Why.** Onboarding clones, pushes and publishes projects without `gh` or `glab` (#1810, Q4).
The forge sign-in (ADR 0077) gives an API token, but `git clone` and `git push` over HTTPS ask
git's credential helper, and today that helper is the CLI's.

1. **The helper serves the human's forge account to git that the window runs**: clone, fetch
   and push of a project or of a repo. After FD-2, the host's `local-ui` scope runs that git
   (ADR 0068). The token comes from the account store (ADR 0077 §6) and is read only where
   the human scope is (ADR 0077 §3).
2. **A chat never gets it.** The helper is named on the command line of the git the window
   runs. It is never written into a clone's, a worktree's or the person's global git config,
   where a chat's own `git push` would find it. Asked from a chat's process tree, it refuses and
   answers nothing. ADR 0070 §4 and FI3 hold: a chat's pushes get their own identity only from
   SD-7a/b, and until then a chat has the credential it has today.
   **The window's git clears the helper list before it names purlis's.** It passes an empty
   `credential.helper` first. Git hands a credential that worked to every helper it is
   configured with, and asks each to store it. Apple's git configures `osxkeychain` system-wide,
   so without the reset the token would land in the login keychain, where any chat's `git push`
   reads it back. purlis's helper answers `store` and `erase` with nothing: the token belongs to
   the account store, and git never writes it anywhere.
3. **The CLI's helper stays a fallback.** A clone made before this amendment keeps the helper its
   config names. A person with no purlis account on the host keeps the CLI's login. purlis
   never needs `gh` or `glab` for its own git.
4. **Scopes:** no new scope. On GitHub, the OAuth app's `repo` (ADR 0077, amended 2026-10-12)
   covers pushing over HTTPS. On GitLab, `api` does too: GitLab grants an `api` token the full
   set of abilities, including `push_code` (`Gitlab::Auth`), so neither `write_repository` nor
   another sign-in is needed.
5. **Who builds it:** #752 builds it, with the tests #1810 names. The helper serves window git,
   and it refuses in a chat's process tree. How the helper reaches the human scope is #752's to
   decide, within these bounds.
