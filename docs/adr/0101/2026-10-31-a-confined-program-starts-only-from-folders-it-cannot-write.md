# A confined program starts only from folders it cannot write

**Amends [ADR 0101](../0101-the-skill-studio-is-built-from-six-extension-capabilities-and-an-extension-that-declares-one-is-confined.md), 2026-10-31.** §2 said a confined program starts programs from what it reads. This narrows that.

A confined program starts programs from the system's program folders and its own folder, less its state folder, its temporary folder and any vault folder or key. Those folders hold its data. A program it runs ships in its own folder, where purlis fingerprints it.

- **On macOS** Seatbelt holds this, and it also maps no library as executable from what it writes.
- **On Linux** the kernel's Landlock holds this. Its rule covers everything beneath a folder, so a folder that holds something held out is named by its other entries, and a link or a name a profile cannot spell is left out. Such a program also makes no file in memory: `memfd_create` is answered as a kernel without the call. The wrap reads this from the profile, so a chat, whose profile starts programs from anywhere, keeps the call.
