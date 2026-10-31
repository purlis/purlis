# Amended 2026-10-26: a runner chat's browser runs on its runner, and logs in on an owned one by opt-in

**Built by #2050**, under spec #2041 (the person's rulings of 2026-10-14: *"A cluster browser
login needs the desktop connected and the persona opted in for that pool; the opt-in is bound to
the pool's definition and image digest"*, and the defaults *"browser frames on a link capped at
15 a second and 1280 pixels wide, input ahead of frames, one frame in flight"*). Item 3 is the
dispatcher's ruling of 2026-10-26 on D-2050-4. The rest not quoted from the rulings was decided
in #2050: items 4 to 8 are **decided in implementation, to be confirmed** (D-2050-1 to D-2050-12
in the pull request). The amendment of 2026-10-14 stands as it is: a runner not marked owned
keeps one label and never receives a vault value; an owned device receives one only as ruled.

1. **A runner chat's browser runs in its runner's session host**, beside its chat, started,
   watched, driven and logged in through the desktop's host, which relays each for its own window
   alone (session protocol 1.38, `remote-link` granted them "relayed"), and only to a runner of
   1.38 or later. A runner's host lets a link between hosts act only on a browser opened over such
   a link for the window it names. A browser on a runner is never opened from a kept login and
   keeps none (#2021).
2. **Over a link between hosts a browser view is held to the link's frames**: at most 15 a
   second and 1280 pixels wide, the engine scaling its frames down, and one frame in flight. The
   window holds a runner browser's view to them too, whatever the runner sends.
3. **A login is filled only on a runner purlis can show is the person's own** (ruled 2026-10-26;
   amended by the person's ruling of 2026-10-30, built by #4071, which marks "marked owned" of
   the amendment of 2026-10-14 and #2561 as built). That is one of:
   - a Docker pool whose engine is a local socket on this machine: the runner is this device,
     owned under the amendment of 2026-10-14, with no mark;
   - a cluster pool, only while **both** hold: **the person's mark**, per pool, off by default,
     set only by the person in the window and never from a chat, the Pool or the operator
     (`owned` in `runners.json`, *This is my own* on the Pools row, whose text says who else can
     read what the runner holds); and **its claim vouched for now**, by the check every link
     makes, at the gate and again after Allow, failing closed where the cluster does not answer.
   A pool on any other engine (`tcp://`, `ssh://`, any `DOCKER_HOST` that is not a local socket) is
   refused in words, and an unmarked cluster pool in words that say how to mark it. The person's
   opt-in is a further gate, never a stand-in for ownership. A login on a cluster runner stays
   behind its per-persona, per-pool opt-in, bound as item 4 says but for that runner: its pool's
   name, context, cluster uid, operator key, claim uid, pod, device and link key, with the Ask's
   engine naming the cluster and context, and no image digest. Any change asks again. A mark
   taken back forgets the pool's opt-ins. A claim being deleted vouches for nothing. A link
   reports the runner it reached with the key it was pinned with, and one that is not the runner
   kept now is refused, so a value goes only to the runner the gate judged. A runner a chat ran
   on without the sandbox is sent nothing, however it is marked. The pool is read again once the
   vault has been read, before anything is sent.
4. **The opt-in is the person's answer to an Ask** naming the login, the pool and its engine, for
   one persona's login on one pool, bound to the login as declared and allowed, to a digest of the
   pool's definition (as `runners.json` keeps it), its image digest, the receiving runner (device
   id, pinned key, its image) and its engine; **kept in memory while purlis runs**. A restart or any
   change asks again; Keep off holds until something changes. A login the persona does not
   declare and allow here is refused before anyone is asked. A lasting answer is #3299.
   *Decided in implementation, to be confirmed (D-2050-3, D-2050-9).*
5. **Every browser view keeps one frame in flight, this machine's too.** *Decided in
   implementation, to be confirmed (D-2050-1).*
6. **The caps are purlis's defaults, the same for every pool for now**; a pool setting for them
   is #3297. *Decided in implementation, to be confirmed (D-2050-2).*
7. **A login asked while the desktop is away waits for it**, at most 15 minutes: the runner tells
   the chat at once that the person is asked once they are back, keeps the ask (its login's name,
   never a value) in memory, and hands it on once when a window hears the runner again, marked as
   one that waited, so the window asks the person afresh whatever it was answered before. One
   whose chat has gone is let go of. *Decided in implementation, to be confirmed (D-2050-5,
   D-2050-10).*
8. **A runner chat started without the sandbox is filled no login**: it shares its runner with
   the browser engine unconfined. *Decided in implementation, to be confirmed (D-2050-11).*
9. **The values go from the vault, read by the window, to the runner's browser for the one
   login**, read as the persona declares the login and this machine allowed it, at the call. No
   vault value is stored on a runner.
10. **The engine's frames are no wider than the narrowest view held to a width** (#3297): a view
    on the runner's own host, which is not held to the link's width, never widens the frames a
    view over the link is sent while one is attached. Only while no view over a link is attached
    are they as wide as the browser's pages. *Decided in implementation, to be confirmed
    (D-2050-13).*
11. **The bounds on what waits for the desktop stay purlis's own** (#3297): at most 16 lines
    waiting on a runner, 4 of one chat's, and 8 told to a chat's next browser call, the same on
    every runner and never a pool's settings. They bound the memory a runner's host keeps for an
    absent desktop and keep one chat from crowding out another's, for a `secret exec` as for a
    login. *Decided in implementation, to be confirmed (D-2050-14).*
12. **A pool's browser frame limits are its own** (#3297, the pool keys `frames` and `width`
    ruled by the dispatcher 2026-10-28 under the person's delegation of that day): each pool may
    set the most frames a second (1 to 60) and the widest frame (320 to 3840 pixels) its runner's
    browsers are watched at over the link; one that sets neither keeps 15 and 1280. The window
    gives them with each browser's start on the runner (session protocol 1.49), so a change holds
    for the next browser without the runner being made again, and the runner holds that
    browser's views over the link to them. The window holds the frames it draws to the more of
    the pool's and the defaults, since a runner before 1.49 sends the defaults. This replaces
    item 6's "the same for every pool". *Decided in implementation (D-2050-19 to D-2050-21).*
