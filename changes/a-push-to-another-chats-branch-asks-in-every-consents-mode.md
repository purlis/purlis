### Fixed

- **A chat's push to a branch another chat made asks you in every `consents` mode.** With
  `[policy] consents` set to `off` or `log` (as under the `watch` template), such a push used to
  be refused with no way out, so you had to push it by hand. Now the window offers Let this chat
  push it whatever the mode. Nothing is loosened: the push stays refused until you press Allow,
  and Keep blocked still holds (#7749).
