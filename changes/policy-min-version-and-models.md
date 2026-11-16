### Added

- **An administrator's policy file can set `min-version` and `models`.** In
  `/etc/purlis/policy.json`, a `min-version` above this build stops every chat that reaches a
  model from starting, a subscription chat included, and the refusal names the file and the
  version to update to. It is read only from a file that passes the trust check. Unknown keys
  inside `models` are allowed, but this version enforces none of them, so any key there stops
  those chats too, naming the key. A key said twice anywhere in the file refuses the whole file,
  as does a `min-version` not written `MAJOR.MINOR.PATCH`. A refused file also stops a chat on a
  key from starting, naming the file and why. Neither key is read on Windows yet.
