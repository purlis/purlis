# Amendment, 2026-11-16: the session host evaluates the policy, and `$PURLIS_POLICY` retires (Q309)

Decision 3 says only the app resolves the setting, and a hook asks the app over the hook socket.
What a chat's hooks read today is what the chat started with, handed to them in
`$PURLIS_POLICY` (`enforcement::ENV`), which the app sets at the launch.
[ADR 0126](../0126-one-project-policy-governs-every-chat-decision-cedar.md) supersedes that
hand-off (Q309 point 3):

- **Only the session host loads, verifies and evaluates the policy.** Hooks, the proxy and
  dispatch ask it over `hooks.sock` for each decision, and never read the policy file or a
  variable.
- **If the host can't be reached, the action is refused.** Decision 3's "every family enforced
  where the app does not answer" becomes that refusal.
- `$PURLIS_POLICY` is retired. Decision 5's choke points stay: each calls the host's evaluation
  in place of reading the variable.

The rest of decision 3 stands as ADR 0126 decision 2 restates it: the policy is read at the plane
root's `HEAD`, a loosening waits for the person's press, and what the person accepted is kept
sealed.
