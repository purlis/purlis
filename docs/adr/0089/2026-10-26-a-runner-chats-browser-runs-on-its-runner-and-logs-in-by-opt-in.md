# Amended 2026-10-26: a runner chat's browser runs on its runner, and logs in by opt-in

**Built by #2050**, under spec #2041 (the person's rulings of 2026-10-14: *"A cluster browser
login needs the desktop connected and the persona opted in for that pool; the opt-in is bound to
the pool's definition and image digest"*, and the defaults *"browser frames on a link capped at
15 a second and 1280 pixels wide, input ahead of frames, one frame in flight"*). What is not
quoted from the rulings was decided in #2050: items 3 to 6 are **decided in implementation, to be
confirmed** (D-2050-1 to D-2050-8 in the pull request).

1. **A runner chat's browser runs in its runner's session host**, beside its chat, started,
   watched, driven and logged in through the desktop's host, which relays each for its own window
   alone (session protocol 1.38, `remote-link` granted them "relayed"). A browser on a runner is
   never opened from a kept login, and keeps none: a kept login stays on the device it was made
   on (#2021).
2. **Over a link between hosts a browser view is held to the link's frames**: at most 15 a
   second and 1280 pixels wide, the engine scaling its frames down, and one frame in flight.
3. **Every browser view keeps one frame in flight, this machine's too**: the next frame is the
   newest, once the client has drawn the last, so no view queues frames. *Decided in
   implementation, to be confirmed (D-2050-1).*
4. **The caps are purlis's defaults, the same for every pool for now**; a pool setting for them
   is a later ticket. *Decided in implementation, to be confirmed (D-2050-2).*
5. **The opt-in is the person's answer to an Ask**, for one persona on one pool, bound to a digest
   of the pool's definition (as `runners.json` keeps it) and to its image's digest, and **kept in
   memory while purlis runs**: a restart, or a changed definition or image, asks again; Keep off
   holds until the pool changes. It is the same for a Docker pool on this machine as for a
   cluster's: every pool asks. Until a pool can be marked owned (§4, as amended 2026-10-14), the
   person's opt-in for a pool is where they say its runner may be given that persona's logins.
   *Decided in implementation, to be confirmed (D-2050-3, D-2050-4).*
6. **A login asked while the desktop is away waits for it**: the runner answers the chat at
   once that it waits as an Ask, keeps the ask (its login's name, never a value) in memory, and
   hands it on to the window as soon as one hears the runner again; what it came to is told to
   the chat's next browser call. *Decided in implementation, to be confirmed (D-2050-5).*
7. **The values go from the vault, read by the window, to the runner's browser for the one
   login**, read as the persona declares the login and this machine allowed it, at the call: the
   runner armed its chat's tools, of which the window holds no record. No vault value is stored
   on a runner.
