# Amendment, 2026-11-24: Windows Hello at each start is the seal's first opt-in, and it narrows the gap (Q319 C6, Q320a N1)

The decision keeps the seal key in the system keyring and declares that on Windows any program
the person runs can read it. M75 slice 43 (#7052) ships the first opt-in that narrows that gap,
before Linux's (Q319a O4):

- **Off by default; the person turns it on in Settings.** With it on the keyring holds the key
  *wrapped* by what Windows Hello signs, and the plain item and its spares are deleted after the
  wrapped item has been read back and unwrapped to prove it. The app asks Hello at its start and
  unwraps the key. Hello signs a fixed challenge with a non-exportable key Windows keeps (Q320a
  N1); a signature that differs reads as a Hello that did not confirm, so the key stays locked
  and is never wrongly released.
- **Narrowed, never closed**, and said so by Settings and doctor (review round 2, F2-B1): a
  same-user process can still read purlis's memory after the unwrap, or start a second purlis.
- **Without the confirmation the key stays locked.** On an anchored plane that is the deny state
  (slice 07a reports it, slice 07b refuses on it); the shipped default applies only on a fresh
  install (spec §4.6, §9.4; review round 5, FB5-B1).
- **Turning it off is a loosening**: it needs the person's press (the window's command layer
  alone makes one) and a Windows Hello confirmation. Turning it on takes the press too, so a chat
  can neither lock the person out nor loosen the seal.
- **The seam is `secrets::hello::Confirm`**, with the real Windows implementation behind
  `cfg(windows)` and a platform-neutral `FakeHello`, so what the opt-in decides is tested on
  Linux and macOS and the real calls run in CI's nightly `windows` job as evidence.
