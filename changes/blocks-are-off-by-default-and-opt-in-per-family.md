### Changed

- **purlis's refusals are off by default and opt-in per family.** A project's committed settings
  file says how each family of refusals runs under `[policy]`: `template` is `open`, `watch`,
  `hold`, `team` or `strict`, and a family such as `guards.merge = "enforce"` overrides it. Each
  family is `off` (not checked), `log` (checked and allowed, with one quiet line in the chat's
  enforcement log and nothing shown) or `enforce` (refused, as before). **A project that says
  nothing runs `watch`**: its guards log instead of refusing, while vault values stay masked,
  vaults stay closed to other personas and credentials stay out of commits. `strict` is purlis as
  it was. In this release the git, merge, release, save and credential guards, secret masking, the
  floor's process rules, dispatch (a pair no grant covers starts anyway) and host asks (answered
  Allow) switch, while your own dispatch nevers and Keep blocked hosts hold in every mode; the other families are keys already and stay enforced until
  they are wired (rulings Q308, Q308a and Q308b; ADR 0125; `docs/policy.md`).
- **A refusal no longer says that nothing can lift it.** It says a person can set its family to
  log or off under `[policy]` and accept that in the window (reverses charter#370).

### Security

- **A chat cannot loosen its own enforcement.** `[policy]` is read only from the plane root's
  committed file, a chat's write or commit of it is refused, and a loosening reaches your chats
  only once you press Accept in the window. What you accepted is sealed on this machine with a key
  only purlis's app reads; an acceptance whose seal does not hold counts as none. A tightening
  applies at once. An administrator's `/etc/purlis/policy.json` can lock families enforced with
  its new `enforcement` list, and its other locks keep their own families enforced.
- **The app's keychain writer refuses the seal's key.** Any program could start the app's binary
  to write one item under `purlis/`; the seal's key lives outside that prefix and is made only by
  the app itself.
