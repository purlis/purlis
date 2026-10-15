# Decided (dispatcher, 2026-10-01)

1. **D-0077a: a GitHub App reaches only the organisations that installed it**, and PATs and
   imports cover the rest. Settled by V13 ("What this costs").
2. **D-0077b: the sign-in token is a Keyring item, never a vault entry.** The stricter reading of
   FI2, supported by FI3 and V16b (0077 §7). #729 and #730 carry the correction.
3. **D-0077c: GitLab asks for `api` at sign-in.** The token is the human's only (FI3), and the
   launch needs writes (V14) (0077 §2).
