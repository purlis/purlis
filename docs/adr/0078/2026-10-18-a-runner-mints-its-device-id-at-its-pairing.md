# Amended 2026-10-18: a runner mints its device id at its pairing

**Built by #2539** on items 3 and 5 of the amendment of 2026-10-17 on a Docker pool's runner,
which the operator ruled that day. It adds to them and changes neither. *Decided in
implementation, to be confirmed* (D-2539-1, D-2539-2).

1. **A runner's device id is minted at its one pairing, by the runner, and never comes with its
   home.** `purlis bridge --pair` mints it in the machine store
   (`purlis_core::machine::mint_device_id`) before it makes the link key, and pairing is refused
   when the store already holds one, whichever desktop asks, before anything is made: *"this
   runner's home came with a device id this runner did not mint …"*. So two runners started from
   one image never pair with one id, as each has a key of its own (item 3). A runner's host
   never mints a device id itself: a link that arrives before the pairing is answered by nobody
   and leaves the store as it was. A paired runner whose store keeps no device id links nobody.
2. **The image is trusted for its home's first contents, as it is for its binaries, and for
   nothing that makes a runner one runner.** A new container's home volume starts as a copy of
   the image's `HOME`, so what the image put there is the runner's: its shell and tool
   configuration, the harness's own files, and purlis's settings in the machine store. The image
   already decides every program the runner runs, `purlis` among them (`docs/runner-image.md`),
   so trusting those files adds no trust it did not have. What one image gives every container
   alike cannot say which container a link reached, so a runner's identity and its pairing are
   never taken from it: a home that came with a device id, a link key or a peers file is not
   paired (item 5), and the session host's credentials are minted fresh each time it starts
   (ADR 0068 §1).
