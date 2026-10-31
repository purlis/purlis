# Amended 2026-10-30: a browser's answer to a login on a runner is carried once

**Built by #4044**, under spec #4023, from the person's ruling of 2026-10-30 that a Codex or
opencode ChatGPT sign-in on a runner is approved in the browser and *"purlis carries my browser's
answer to the runner"*. The spec says this goes over the link's loopback brokering, which does not
exist (RR-24, #899, is open), so what is carried is **decided in implementation, to be confirmed**
(D-4044-1 to D-4044-7 in the pull request). §3 stands: no subscription token is read, copied,
stored, vaulted or relayed.

1. **What crosses is the browser's one redirect, and it is not a sign-in.** The redirect holds a
   one-time authorization code bound to a proof only the login on the runner holds, so it is worth
   nothing without that login. purlis passes it from this machine's loopback to that login, as it
   passes a pasted code to Claude Code's, keeps it nowhere and logs it nowhere.
2. **What the login answers stays on the runner.** Only whether it took the redirect crosses the
   link. Codex's next page, whose address carries the account's values, is asked for on the runner
   by its path alone, so those values are not read or sent on.
3. **It is one request, not a port.** Session protocol 1.59's `sign_in_redirect` is the paired
   desktop's alone, and reaches one login's own port and path on the runner's loopback and nothing
   else there, so ADR 0089 §8 stands: a runner's ports stay private, and nothing is opened on it.
