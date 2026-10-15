# Amended 2026-10-15: a sandboxed Codex chat runs on a sandbox-only profile (#2037)

**Ruled by the operator on 2026-10-15**, on ADR 0095 §5:

- A sandboxed Codex chat, at the floor or the full sandbox, runs only on a **profile for
  sandboxed chats only** (`sandbox_only` in this machine's profiles, [`docs/plane-format.md`](../../plane-format.md)): a
  profile purlis makes with a folder of its own, signed in through Codex's own sign-in in that
  folder, which no chat outside the sandbox starts on. Codex's home is that profile's folder, in
  place of a home of the project's own, and no copy of the person's sign-in is made.
- A sandboxed Codex chat with no such profile raises **one Ask**, *Codex needs a sandbox-only
  profile here*, whose button makes one and opens Codex's own sign-in for it. Until then the
  chat waits: its start is refused, with the Ask as the way out. No Codex chat runs without the
  floor because of this.
- **D-88q's guarantee holds as it was:** no sandboxed chat reads or writes the operator's own
  Codex home (`~/.codex`, adopted as a profile), and nothing a sandboxed chat writes in Codex's
  state reaches an unsandboxed run, since no unsandboxed run uses that folder.

*Decided in implementation (D-2037-6 to D-2037-9), to be confirmed by the operator:*

- A sandbox-only profile imports no settings: purlis writes the home's config itself at every
  start, as above (untrusted folders, trust for exactly the hooks purlis arms).
- One Codex home per profile: the stores D-88s grants are shared by the sandboxed chats on one
  profile, across projects, inside the wrap only.
- The wrap takes as Codex's home only the profile's own folder, `<profiles>/<name>/config` as the
  kernel names it, with no `..`, no link and owned by this user, and denies every other
  profile's folder and the homes projects had before.
- The conversations of a project's former home are brought to the profile's folder once, before
  the wrap, read by descriptor without following a link, each where the profile has none of that
  name, and never its sign-in or config.
