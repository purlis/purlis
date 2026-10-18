# Amended 2026-10-18: the desktop keeps a runner's key once, and removes the runner whole

**Ruled by the operator on 2026-10-18** for item 1, on the format #2525 proposed: *"yes,
`<config>/runners.json` exactly as proposed: {"version":1,"runners":[{"name","pool":"docker",
"container","image","user","home","device","key","paired"}]}. Refusals as you described."* Items 2
and 3 are *decided in implementation, to be confirmed* (D-2525-1, D-2525-2). §3 and §8 are left
as accepted, and this note adds to them.

1. **The desktop keeps what it pinned in `<config>/runners.json`** (§8), for a Docker pool's
   runners, as ruled: each runner's name, its pool, its container's full id, the image by
   digest, its user and group by number, its home, the device id and public link key its
   pairing answered, and when it was paired (`purlis_core::runners`). **A runner's device id and
   key are kept once.** A later answer for a runner already kept, with another device id or
   key, or another container under its name, is refused as a changed key
   (`runners::Refused::KeyChanged`, which names the runner and says to remove it), and what was
   kept stays. An answer with a device id or key
   another kept runner holds, or with this machine's own device id, is not kept. Chats are
   denied the file.
2. **Removing a runner removes everything it is, and says what it could not.** The desktop
   unpairs itself over the link when the link is up (session protocol 1.25, ADR 0068 amended
   for #2525). Then the runner's container and its home volume are removed, whatever state they
   are in, each tried whatever the other did. A part that is already gone counts as removed.
   The runner's row goes once both are gone. The peers file is in the home volume, so a peer
   that could not be unpaired goes with the volume.
3. **What is left is said with the command that removes it, and the runner stays kept** until
   all of it is gone, so the removal can be asked again.
