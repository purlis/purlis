# Amended for #2023, browser limits and the idle stop, 2026-10-18

A chat's browser that neither the chat nor the person used for a while stops, and a device runs
at most a budget of browsers beside the three a project's chats hold. Spec #2008's Limits rules
the defaults (three per project; 16 GB: 4, 32 GB: 8, 64 GB: 16; ten idle minutes), and that every
limit is in Settings and a refusal names the setting. The keys and their home are **the person's
ruling of 2026-10-18**:

1. **Three keys in this machine's store**, `machine.json`'s `browsers` table:
   `browsers.per_project`, `browsers.on_this_device` and `browsers.idle_minutes` (`0`: never).
   Each is absent until set, and absent is purlis's default; a value out of its bounds (1 to 16,
   1 to 64, 0 to 1440) reads as not set. Machine tier, device-bound: the budget follows this
   machine's memory, and a team's committed file never sets another person's machine.
2. **Settings › This machine › Browsers** shows and sets them, each with a reset to its
   default. They apply at the next browser call; a browser running past a lowered limit runs on.
3. **A refusal names the setting**: past the project's limit, or the device's budget, the chat
   (or the person) is told so and that Settings › This machine › Browsers sets it.

Decided in implementation, to be confirmed:

4. **The memory class** is the largest power of two of GiB the memory reaches with a tenth to
   spare, one browser per 4 GiB of it, never none; a device whose memory cannot be read gets the
   smallest ruled class's budget, 4. An 8 GB device gets 2. *D-2023-2.*
5. **What counts as a use**: the chat's act or login, an Ask the chat raised about the browser,
   and the person's take-over and hand-back. A browser the session host lists with a driver (the
   person, or purlis performing a login) never stops for going unused, nor one an Ask waits on.
   A host that cannot say what it drives has nothing stopped. *D-2023-3.*
