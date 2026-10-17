# Amended (2026-10-17, #2342): a sandboxed chat on Linux reaches a kept vault through the app

Follows [one Keychain question per run, and none from a sandboxed chat](2026-10-10-01-one-keychain-question-per-run-and-none-from.md).

**Decided in implementation (#2342).** The app now runs a sandboxed chat's `secret exec` on
Linux as it does on macOS, inside purlis's own wrap ([ADR 0067's amendment of 2026-10-16](../0067/2026-10-16-purlis-s-own-wrap-on-linux.md)).
So the refusal of a keyring-held vault in a sandboxed chat names `secret exec` through the app
on Linux too, where the 2026-10-10 amendment named a terminal outside the chat. A terminal
outside the chat is named only on a system where purlis has no sandbox to run the command in.
Nothing else of that amendment changes: a sandboxed chat still never reads the keyring through
the command.
