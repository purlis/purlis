# Browsers on pools and runners: an audit

Research, 2026-11-18. Facts only. This document decides no policy; the person rules on it
afterwards. Read from branch `remote-audit-browsers-on-pools-704bznr5`, cut from `main` at
`92d19c2320`. It feeds the /wayfinder map of the remote rework.

**The person's complaints.** For remote pools and browsers together: the setup UI is unclear and
buggy; starting is slow; the session lifecycle (start, run, restore after a restart, reconnect)
is not trusted; performance and observability are weak.

**Marks.** Every claim carries one:

- **V** (verified from code): read at the cited `file:line` in this tree.
- **I** (inferred): follows from verified facts but was not read or run directly.
- **U** (unverified): needs a run, or a source this audit could not reach.

Nothing was run: no browser was started and no timing was measured. Every number below is a
constant in the source, not a measurement. Paths are relative to the repo root.

**What this audit could not do.** `gh` could not read its configuration in this sandbox
(`open /Users/aharon/.config/gh/hosts.yml: operation not permitted`) and no token was in the
environment, so the tracker was not read. Section F is therefore a list of candidates from the
git log, all **U** against the tracker. Nothing was edited or closed.

## A. What the browser feature is today

### A1. The layers

A chat's browser is a Chrome-for-Testing process, driven over a NUL-framed pipe, behind four
hops. Each hop has its own crate:

| Layer | Where | What it does |
|---|---|---|
| Harness tools | `crates/purlis-core/src/browsertools.rs` (1063 lines) | `browser_open`, `browser_login`, `browser_fill_secret` and the rest are the chat's tool names. |
| Window | `app/src-tauri/src/chatbrowsers.rs` (6281), `app/src-tauri/src/browsers.rs` (2502) | Posture Ask, budget, judge answers, vault reads, pool-login opt-in. Each call arrives as `Ask::Browse` (`chatbrowsers.rs:4-8`, V). |
| Session host | `crates/host/src/browsers.rs` (1081) | Frames only while watched; the driver lease; link-held frame limits (`host/src/browsers.rs:12-40`, V). |
| Engine glue | `crates/purlis-core/src/browsers.rs` (2481) | Download, pin, reach proxy, judge; `EngineBrowsers::start` at `:373` (V). |
| Driver | `crates/browser-engine/src/drive/browser.rs` (7609) plus `browser/*.rs` | CDP calls, settle, takeover, login, fill, downloads. |

On a pool the chain is: harness, window, **the window's own host, which relays**
(`crates/host/src/relay.rs:1-60`, V), the runner's host, the engine. The runner runs the engine
beside the chat, so the chat's `localhost` is the runner's (`chatbrowsers.rs:55-66`, V).

### A2. Watched browsers

- A browser makes frames only while a view is attached. The first view asks the engine for
  frames; the last to go stops them (`host/src/browsers.rs:12-17`, V). A thumbnail is about one
  frame a second (`THUMBNAIL_EVERY`, `host/src/browsers.rs:138`, V).
- Over a link between hosts a view is held to 15 frames a second and 1280 pixels wide, one frame
  in flight (`host/src/browsers.rs:130`, `:19-30`; `tests/it/a_runner_runs_its_windows_browsers_as_its_desktop_relays_them.rs:7-12`, V).
  The pool can carry its own limit (`purlis_core::runners::link_frames_of`, used at
  `app/src-tauri/src/browsers.rs:1329-1330`, V).
- The window side is `app/src/BrowserView.tsx`, `BrowsersGridTab.tsx`, `BrowserPages.tsx`,
  `BrowserDownloads.tsx`, `browserFrames.ts`, `browserWatch.ts` (V, listing).
- **Header pages and downloads (#7120, `ba5eb70659`):** a view lists its pages and downloads and
  the person can watch another page (protocol 1.91, #6989, #6990). The engine side is
  `PersonInput::Watch` → `Browser::person_watches` (`browser-engine/src/drive/browser/takeover.rs:498`, `:762`, V);
  downloads are accounted in `drive/browser/downloads.rs` and held in the browser's own
  `downloads` folder before any page loads (`purlis-core/src/browsers.rs:489-496`, V). The
  tool side is `Called::Pages`, `SwitchPage`, `Downloads`, `SaveDownload` (`eventlog/browsing.rs:100-127`, V).
  The view's title, address and load (#6938, `11ec780b65`) and where a page is (#6959,
  `fbcafb34b4`) landed the day before.

### A3. The takeover flow

- **The lease is the host's.** The agent's until the person takes over, then theirs until they
  hand back; neither the window's link ending nor its crash returns it (`host/src/browsers.rs:33-45`, V).
- Engine side: `Browser::take_over` (`takeover.rs:228`), `hand_back` (`:253`), `driver()` (`:208`),
  `person_input` (`:414`), `size_to` (`:383`) (V). Every act is cut by epoch: an act checked
  against an earlier lease is cut by the engine (`host/src/browsers.rs:37-39`, V).
- The person's input names the frame it was aimed at (`AIMED_JPEG`); input at a page that moved on
  is dropped (`tests/it/the_person_takes_a_browser_over_and_hands_it_back.rs:1-9`, V).
- A take-over while purlis performs a login cuts the login and leaves the browser the person's
  (`host/src/browsers.rs:41-44`, V).
- `Driver::Person` is the protocol's word (`session-protocol/src/browsing.rs`, `Driver`; the event
  log's own `Driver` is at `eventlog/browsing.rs:62`, V).

### A4. Vault logins: `browser_login` and `fill_secret`

- `browser_login` names one of the persona's declared logins. The window reads the vault values
  for exactly that login and hands them to the host, whose driver fills them on the login's own
  origin (`chatbrowsers.rs:25-34`, V; `purlis-core/src/browsers.rs:36-44`, V). The chat is told
  the page the login left, never a value (V, same comment).
- Just before the first value goes in, the reach proxy is **bound to the login's site alone for
  the browser's life** (`purlis-core/src/browsers.rs:39-44`, V).
- `fill_secret` fills one field; the proxy is held to the field's site until the page loads
  another document or closes (`purlis-core/src/browsers.rs:45-53`; `FILL_RELEASED_AFTER = 2s` at `:1484`, V).
  Engine: `drive/browser/fill.rs` (742 lines), `login.rs` (1273), `leaving.rs` (907).
- **On a runner a login needs the person's opt-in**, bound to the pool's definition and image
  (`purlis_core::poollogins`; `chatbrowsers.rs:55-66`, V). For a Docker pool on another machine
  the opt-in is bound to the person's mark and the pin (ADR 0123, 2026-11-14 amendment).
  A runner keeps no value and no kept login (`chatbrowsers.rs:65-66`, V).
- Sign-in timeout: `SIGN_IN_WITHIN = 120 s` (`purlis-core/src/browsers.rs:1169`, V).

### A5. Session-protocol powers

- `browsing::Act` carries `Open`, `FindLoginForm`, `DescribeField`, `AsksToSignIn`, `Navigate`,
  `Snapshot` and more (`session-protocol/src/browsing.rs:35-66`, V). The event log's `Called`
  lists 26 powers (`eventlog/browsing.rs:100-127`, V).
- `StartProgram` for a browser carries `reach`, `kept` (kept login) and `link_frames`
  (`app/src-tauri/src/browsers.rs:1325-1331`, V).
- Reach is judged per act: the chat's sandbox hosts plus the person's Allows since; a site new to
  the project asks (`purlis-core/src/browsers.rs:30-35`, V). A browser started with no reach
  fails closed (V, same).
- `WINDOW_ONLY`: only the person's press answers a browser Ask (`chatbrowsers.rs:74-76`, V).

### A6. The engine

- Pinned Chrome for Testing, downloaded to `<data>/engines/chrome-for-testing` on first use and
  checked against the shipped pin or a newer Sigstore-signed manifest (`purlis-core/src/browsers.rs:7-12`, V).
- Kept current by a background check: `FIRST_CHECK` 5 min after the host starts, then every 6 h
  (`:196-199`, V).
- A runner image carries the libraries the engine links (`runner-image/Dockerfile:44-47`, V); the
  engine itself downloads on the runner's first browser, "as on any machine" (V, same lines).

## B. Lifecycle

### B1. Start

The window's call, in order (`app/src-tauri/src/browsers.rs:1296-1345`, V):

1. The chat's first call raises the posture Ask and waits for the person (`chatbrowsers.rs:6-14`, V).
2. A process-wide `starting` mutex is taken (`browsers.rs:1303`).
3. `Command::List` over the link, to count running browsers against the device budget (`:1304-1311`).
4. `StartProgram` over the link, waited for up to `A_START_TAKES_AT_MOST` (`:1336`).
5. On the host: `EngineBrowsers::start` (`purlis-core/src/browsers.rs:373-503`): data folder made,
   engine chosen and prepared (download if absent, manifest read if behind), judge, reach proxy,
   engine process via the keeper, `Browser::judged` attaches, downloads folder made.
6. `follow_its_end` registers the end callback (`browsers.rs:931-` , V).

### B2. App restart

- **A normal quit ends every browser.** The next start offers each address once as a button,
  never loaded by itself (`BrowsersAtQuitNotice.tsx:7-12`, V). Cookies and logged-in state of the
  ended browser are lost: its data folder is fresh and removed at its end
  (`host/src/browsers.rs:7-10`, V). Only a login the person kept for reuse survives (#2021), and a
  runner keeps none (`chatbrowsers.rs:65`, V).
- **A window crash while the host lives keeps the browser**, with its lease, for the grace period
  (ADR 0068 §2). On return `programs_of` lists it and the window takes it back by its note
  (`chatbrowsers/takenback.rs:1-33`, V). A browser with no note, an unreadable note, a chat that
  did not come back, or a login the note does not name is **ended** (`takenback.rs:22-27`, V).
- A runner's chats come back after the window restarts because the window names its link by a
  stable digest (`relay.rs:44-52`, #2623, V). Whether a runner's **browser** comes back the same
  way is **I**: the relay carries the projects and views over (`relay.rs:20-35`), and the window
  note is kept "in the host beside it" (`takenback.rs:4-8`), but no test I read runs the two
  together (see D).
- **The opt-in to log in on a pool is in memory.** "The opt-in lasts while the app runs"
  (`chatbrowsers.rs:62-63`, V) and the posture Ask is "in memory only: a relaunch starts with
  none, and the chat's next call asks again" (`:68-70`, V). After every restart the person is
  asked again for the same pool and persona.

### B3. Link drop

- The window's host keeps its link to a runner and relinks on its own (`relay.rs:20-41`, V).
  Commands in flight are refused as unanswered, never resent; while down every command is refused
  in words (`relay.rs:29-33`, V).
- The window shows **Reconnecting…** on the runner's chats (`reconnecting.rs:1-14`, #2496, V);
  past 30 s it says why with Try now and Check pool (`cf302a779e`, ADR 0078 amended, #7135).
  States: `Reconnecting`, `Reconnected`, `KeyChanged`, `RunnerGone` (`reconnecting.rs:26-38`, V).
- Browser views have their own `link_said` callback (`browsers.rs:1359-1365`, V). **Whether a
  browser's view dims as a chat's does is U**: the dim above is described for chats.
- A view's stream the other host opened waits `PARKED_FOR = 30 s` for its attach (`relay.rs:168`, V).
- Frames dropped while down are not replayed: a reattached view starts on the newest whole frame
  (`host/src/browsers.rs:23-26`, V). That is correct for a picture; a call in flight is not
  recovered (above).

### B4. Pool forgotten or removed

- **Forget it here** (ADR 0123, 2026-11-17, #7122): forgets the pool and runners locally,
  ends the window's links, drops login opt-ins, and says the `docker container rm` and
  `docker volume rm` to run by hand (amendment file, V).
- In code (`app/src-tauri/src/dockerpools.rs:675-688`, V): `gone_from_here` calls
  `forget_pool_logins` and `links_end_once_gone`. It does **not** call `end_on`, which the
  browsers module has for exactly this (`app/src-tauri/src/browsers.rs:1004`). The only caller of
  `end_on` is the untrusted-pool hook (`lib.rs:4270-4274`, V).
- I: the browsers on a forgotten pool end when their link ends, through `follow_its_end`
  (`browsers.rs:931-960`); that callback logs a crash for any end the window did not ask for
  (`:950-953`, V). A forgotten pool's browsers would then be recorded as **crashed**. **I**, not run.
- On the machine that stays, the runner and any browser in it keep running until someone removes
  the container (ADR text above, V). That is disclosed, not hidden.

### B5. Where state lives, and where it is lost

The tier column is **I**: named from the code's own words, not checked against
`docs/plane-format.md`'s table.

| State | Tier (`docs/plane-format.md`) | Kept where | Lost when |
|---|---|---|---|
| Engine and its download | Machine (rebuildable) | `<data>/engines/chrome-for-testing` | Never by purlis; a runner downloads again if its home is new. |
| A browser's data folder (cookies, storage) | Machine, scratch | `<data>/browsers/<program>` | At the browser's end, by design (`host/src/browsers.rs:7-10`). |
| Engine log | Machine | `<data>/browsers/<program>.engine.log`, newest 20 (`purlis-core/src/browsers.rs:129`) | Swept at a later start. |
| Window note per browser | In the host's memory, beside the browser | Session host | Host exit; or quit. |
| Posture Ask answer, pool-login opt-in | **None: memory** | `chatbrowsers.rs:62-70` | Every app restart. |
| Site Allows | Plane or Machine | `[sandbox].browser-sites`, `app/sandbox.json` (`plane-format.md:868`) | On Remove. |
| Kept logins | Keyring-adjacent, device-bound | Device that made it (`relay` never sends a kept login, `a_runner_runs...:6`) | Never sent to a runner. |
| Addresses at quit | Machine | Read by `browsers_at_quit` (`chatbrowsers.rs:6048`) | Dismiss. |

## C. Speed

**Time to the first page of a browser a chat opens.** No timing is recorded anywhere (see E), so
this is the serial order and the limits that bound each step, not a measurement.

| # | Step | Where | Serial? | Bound (source) |
|---|---|---|---|---|
| 1 | Posture Ask answered by the person (first call only) | `chatbrowsers.rs:6-14` | Yes; human time | none |
| 2 | Pool-login opt-in Ask (login on a pool, once per app run) | `chatbrowsers.rs:55-62` | Yes; human time | none |
| 3 | `starting` mutex: one browser start at a time **in the whole window** | `app/src-tauri/src/browsers.rs:1303` | Yes | held across steps 4-9 |
| 4 | `Command::List` round trip over the (relayed) link | `:1304` | Yes | 30 s default for any answer |
| 5 | `StartProgram` over the link | `:1336` | Yes | **`A_START_TAKES_AT_MOST` = 60 + 3600 + 60 = 3720 s** (`purlis-core/src/browsers.rs:243-244`) |
| 6 | Engine chosen; manifest read **only if behind**, up to 60 s (`MANIFEST_READ`) | `:403-421`, `:526-544` | Yes | 60 s |
| 7 | Engine download (first browser on a machine or runner; Chrome archive, up to 1 GiB) | `install.rs:55`, `:289` | Yes | `Https::ARCHIVE` 3600 s |
| 8 | Reach proxy started; engine launched through the keeper; `Browser.getVersion` over the pipe | `launch.rs`, `process.rs:207` | Yes | `FIRST_ANSWER` 60 s |
| 9 | `Browser::judged` starts threads and `Screencast`; downloads folder made | `drive/browser.rs:2292-2400` | Yes | none |
| 10 | `Target.createTarget about:blank`, wait for the page event | `drive/browser.rs:3562-3600` | Yes | `settle_within` 10 s |
| 11 | `Page.navigate`, then **wait until the page is quiet for 500 ms** | `drive/mod.rs:1131-1132`; `settle.rs:217` | Yes | `settle_within` 10 s |
| 12 | A view attaches; first frame (more latency over a link: 15 fps, 1280 px) | `host/src/browsers.rs:130` | Yes | none |

What the audit can say from the source:

- **V.** A browser start has no concurrency: steps 3 to 11 are strictly serial and step 3 locks
  out every other start in the window while it runs, including a download (`browsers.rs:1303-1345`).
  Two chats on two runners wait on each other.
- **V.** A step that fails slowly fails slowly: a start the person cannot reach waits up to 62
  minutes before the window gives up (`A_START_TAKES_AT_MOST`). The rationale is the first
  download's size (`purlis-core/src/browsers.rs:239-242`); it applies to every start.
- **V.** The runner image carries the engine's libraries but not the engine
  (`runner-image/Dockerfile:44-47`). A new runner downloads Chrome on its first browser. For a
  runner made "for minutes" (ADR 0089) that download is on the critical path of every first
  browser. **I** on how often a runner is fresh; the 2026-11-02 amendment keeps a stopped runner's
  home, which helps.
- **V.** The 500 ms quiet window is added to every navigation answer that settles
  (`drive/mod.rs:1131`).
- **U.** The `Command::List` round trip (step 4) exists to count browsers; the host could refuse
  at the budget itself. Whether it matters on a ~100 ms link was not measured.
- **U.** Which step dominates on a real pool. This is the first thing a harness should record.

## D. Tests and real-infra coverage today

### D1. What exists

| Layer | Tests | What is real, what is a stand-in |
|---|---|---|
| Engine | `crates/browser-engine/tests/it/` (about 60 files, e.g. `a_real_engine_runs_with_no_port_and_ends_with_its_host.rs`, `a_real_engine_paints_frames_only_while_shown.rs`, `a_browser_opened_from_kept_logins_is_signed_in.rs`) | **Real Chrome**, local, on fixture sites. CI job `browser-engine-macos` (`ci.yml:1578-1616`) runs it on a pull request that changes the crate; the Linux job runs it too (`ci.yml:1501`, `:1559`). |
| Host | `crates/host/tests/it/the_person_takes_a_browser_over_and_hands_it_back.rs`, `a_window_that_comes_back_takes_its_browsers_back.rs`, `a_runner_runs_its_windows_browsers_as_its_desktop_relays_them.rs` | **Stand-in browsers** (`StandInBrowsers`), in-memory links (`duplex`). The relay test says "The browsers are stand-ins, so none of this needs an engine" (`:15`). |
| Window | `app/src-tauri/src/chatbrowsers/tests*.rs` (3834 lines), `browsers/hosted/tests.rs`, `hostedsessions/tests/browsers_back.rs`, `behaviour/remote_logins.rs` | App logic against a hosted stand-in. `remote_logins` "checks the gate, not a fill" (`:12-13`). |
| Core | `purlis-core/tests/it/a_browsers_answer_reaches_the_login_on_its_runner.rs` | A real paired link between hosts, no browser. |
| End to end | `app/e2e/specs/browser-view.browser.e2e.ts` | **One test**: built app, local host, real engine, a view tab draws a frame and a click takes over. Local only. |
| Pools, real infra | `app/e2e/specs/pool-setup-{docker,cluster,remote-docker,remote-docker-tls,remote-podman}.pool.e2e.ts`; `tests/{kind-cluster,remote-docker,remote-podman,ssh-runner}`; the CI parity job "(host, protocol, sandbox, browsers)" (`ci.yml:583`) | Real Docker, SSH, kind. The parity job is gated on paths that include the browser crates (`ci.yml:614`). I found **no scenario in it that opens a browser on the runner**: `behaviour/` has no browser file beyond `remote_logins.rs`. **U** for the parity suite's inner list. |

### D2. The gap

**V.** No test I could find starts a real Chrome on a real runner, reached over the real relay
and link, watches it from the window, takes it over, drops the link, restarts the window and
comes back. Every hop has its own test against a stand-in for the next, which was the stated
design (`browser-view.browser.e2e.ts:7-9`), and which leaves the join between hops, where the
person's complaints live, untested.

### D3. Cases a real-infra harness should run

Run each against: a local Docker pool, a remote Docker pool over SSH, a remote one over TLS, a
cluster pool on kind. Each case records the time per step in C.

1. **Cold first browser on a fresh runner.** Time from `browser_open` to first frame, with and
   without the engine in the runner's home. Fail above a budget the person sets.
2. **Second browser on the same runner**, and two chats on two runners starting together: they
   must not wait on each other (`starting` mutex).
3. **Start with the engine's download blocked** (no route to the host that serves it): the person
   is told within a stated time, not 62 minutes.
4. **Watch, then take over, type, hand back**, on the runner, over the link. Input aimed at a
   moved page is dropped.
5. **Link drop for 5 s, 40 s, and past the keep-alive**, with a browser mid-navigation, mid-download
   and mid-login: the view dims, comes back on a whole frame, the agent's call in flight is
   answered as unanswered, and nothing is sent twice.
6. **Window crash and relaunch** (kill -9 the app, host alive): the browser comes back on its
   chat, the lease is unchanged, the login is still the same login.
7. **Normal quit and relaunch**: runner browsers end; the addresses are offered; the runner's data
   folder is gone.
8. **Runner restart** (container restart, machine reboot): the browser's row ends with a reason
   that names the runner, not "crashed".
9. **Pool forgotten and pool removed while a browser runs**: the row ends once, with a reason
   that says "pool forgotten", and no phantom `crashed` event is logged.
10. **Pool pin changes or the person clears the mark while a login is held**: the browser ends
    and the opt-in is gone.
11. **Login on a marked pool and an unmarked pool**: the unmarked gets no value and the chat is
    told why; the marked page is signed in; the value is never on the wire past the first fill
    and never in a log. (Never print the value; assert by a canary.)
12. **A download of 50 MB on the runner** is held in its folder, listed in the header, and saved
    to the project through `browser_save_download`; a download the link cuts is given up on.
13. **Header pages**: open two pages, `Watch` the second, close the first.
14. **A site new to the project** asks; Allow once, Allow always; the ask is answered while the
    link is down.
15. **Frame budget**: 15 fps and 1280 px are not exceeded on the link; a view on the runner's own
    host is not held.

## E. Observability gaps

**V**, from the code read above:

- **No step of a start is timed.** The only `Instant` in the start path is the engine's age
  check (`purlis-core/src/browsers.rs:652`). `tracing` has four lines in the host and core
  browser modules in total.
- **The event log records what an act was, not how long it took.** `Said::Opened` carries only
  the browser's name; `Said::Call` carries browser, call, origin and outcome (`eventlog/browsing.rs:140-170`).
  No duration, no step, no refusal reason.
- **An end says who ended it and why, but only in the host's log** (`browsers.rs:947`, #2745).
  The window logs `Crashed` for any end it did not ask for (`:950-953`), which includes pool
  forget and link end (see B4).
- **The engine log is per browser and kept 20 deep**, but nothing in the window links a browser's
  row to it (**U**; I found no reader).
- **The engine download is told as progress** (`enginedownload::here().started(&named)`,
  `browsers.rs:1317`), but a start that is stuck on something else (the mutex, the `List` call,
  the proxy, the first answer) looks identical to a slow download from the chat's side.
- **A runner's engine, version and download state are not shown** on a pool's page (**U**: not
  found in `poolsetuptab.rs` or `PoolSetupTab.tsx`).
- **Reconnecting says why for a chat** (#7135) but I found no equivalent for a browser's view
  (B3).
- **Nothing reports frame rate, frame size or link delay actually delivered**, only the limits.

What a person debugging a slow or stuck browser cannot see today: which step they are in, how
long each took, whether the runner downloaded Chrome, whether the link or the engine was the
slow part, and why a row ended on a runner.

## F. Tracker hygiene for milestone M63 Browsers

**Not done.** `gh` is blocked in this sandbox (see the top). I did not look for credentials
elsewhere. Nothing was edited or closed. The 56 open issues were not read, so no claim below is
about the milestone's contents; each is **U** against the tracker.

What can be offered is a list of issues that merged work names, which the person or a later
session with `gh` should check first. If any is still open in M63, it is done-but-open:

| Issue | Merged work that names it | What to check |
|---|---|---|
| #6989, #6990 | `ba5eb70659` (#7120): a view lists its pages and downloads, and the person can watch another page | Both are closed by the PR's wording. |
| #6938, #6994 | `11ec780b65` (#7074): title, whole address, load, back/forward/reload/stop. "continues #6994" | #6994 may be partly open on purpose. |
| #6959 | `fbcafb34b4`: where a page is and who drives it | |
| #6290 | `926fe44cca` (#6311): a browser handed over `ask_operator` has an address field | |
| #6291 | `76ca92b578` (#6323): `browser_login` opens an OAuth authorize URL with its query | |
| #6387 | `1657e78816` (#6943): a runner chat's file opens read-only | Not browser; check its milestone. |
| #6266, #6267, #6268, #6269, #6270, #6271, #6272 | the Docker-on-another-machine series, `3a9b1bfe98` to `c0be2cab70` | Pool, not browser; belongs with G. |
| #7122, #7116 | `7105c61f9b`, `7ef6d5dcb8`: a pool whose machine doesn't answer can be forgotten; a remote pool comes back after its machine restarts | Pool lifecycle, relevant to B4. |
| #7135 | `cf302a779e`: Reconnecting… says why | |

**Duplicates and stale:** not assessable without the list. A method that fits this repo's rules
(`docs/agents/issue-tracker.md`): list M63 with `gh issue list --milestone "M63 Browsers" --state open --json number,title,body,labels,updatedAt`,
match each `#N` against `git log --grep "#N"`, and run the `skills:backlog-sync` skill, which
builds an evidence ledger the person approves before anything is closed.

## G. Overlap with pools

Most of the complaints are not browser problems. The browser rides on the pool's runner, link
and start path, and each of these is the pool's or the session host's:

| Problem | Why it is shared | Browser's part |
|---|---|---|
| **Slow cold start** | A runner made for minutes has a fresh home (ADR 0089); the engine downloads into it on the first browser (`runner-image/Dockerfile:44-47`). The same cold home slows a harness's first start. | Which engine, and whether to bake it in. |
| **Setup UI unclear and buggy** | `poolsetuptab.rs` (2456 lines), `PoolSetupTab.tsx`, `AddDockerPool.tsx`: **33 commits in 24 days** touch them (`git log --since=2026-10-25`), mostly fixes: Connect over SSH with a space in the path (#6823), no host key recorded, a non-SSH machine told from a key not saved (#6955), refused forwarding (#6349), TLS files, pin change. | None, except the login opt-in Ask. |
| **Link drop and "Reconnecting"** | One relay for chats, browsers, shells, code (`relay.rs`). A single fix serves all. | Whether a browser view shows the same state (B3). |
| **Restore after restart** | The window names its link by digest so the runner names it back its chats (#2623). Browsers use the host's note (`takenback.rs`) over the same link. | Notes are in the host's memory; opt-ins are in the window's. |
| **Pool forget, remove, pin change, key change** | One set of events (`reconnecting.rs:26-38`, ADR 0123). | `end_on` is not called from `gone_from_here` (B4). |
| **Observability** | Pool doctor, `Check pool` and the link states are the pool's. | Step timings of a start, rows' end reasons. |
| **Mark and vault gate** | `poollogins::Bound` and the owned mark (#4071, #6271) are the pool's. | Fill and login. |
| **Test infra** | `tests/{remote-docker,ssh-runner,kind-cluster}` and the parity job are pool infra. | One more scenario family would reuse them. |

Browser-only: the engine's CDP driver, settle and the 500 ms quiet, the reach proxy and judge,
fill and login, takeover and header pages, frame limits.

## Top 10 problems

Ranked by how much of the person's complaint each one explains.

1. **No test joins the hops.** No real Chrome on a real runner over the real relay, watched,
   taken over, dropped and restored. Everything between the window and the engine is tested
   against a stand-in (D2). Without it, "not trusted" cannot be argued down.
2. **A browser start is serial, and one window-wide mutex covers all of it** (C, steps 3-11,
   `app/src-tauri/src/browsers.rs:1303`). Two runners wait on each other, and a download blocks
   every other start.
3. **A start may wait 62 minutes** before the window gives up (`A_START_TAKES_AT_MOST`, 3720 s).
   A start that cannot finish looks the same as one that is slow.
4. **Nothing is timed or traced in the start path.** Four `tracing` lines; no step, no duration,
   not in the event log either (E). "Starting is slow" cannot be located today.
5. **A fresh runner downloads Chrome on its first browser.** The image carries the libraries but
   not the engine (`runner-image/Dockerfile:44-47`), so a runner made for minutes pays up to a
   1 GiB download on its first browser (C7).
6. **The posture Ask and the pool-login opt-in live in memory.** Every relaunch asks again for the
   same pool and persona (`chatbrowsers.rs:62-70`); a chat started yesterday cannot open a
   logged-in browser unattended today.
7. **Quit ends every browser, and a runner's browser loses its login with it.** Addresses are
   offered back; cookies are not (B2). A login is repeated each time.
8. **Forgetting a pool does not end its browsers through `end_on`** (`dockerpools.rs:675-688`,
   B4). Their rows end by the link and are logged as crashes (**I**). Cause and effect are
   misreported exactly where the person is already unsure.
9. **A browser's view may not say why the link is down.** The chat's Reconnecting… now says why
   past 30 s (#7135); I found no browser-view equivalent (B3, **U**).
10. **Pool setup is churning**: 33 commits in 24 days on three files, each a corner of Connect
    (G). The unclear UI is a symptom of a flow with many branches (SSH, TLS, context, Podman,
    cluster, join) that a person walks once.

## Open decisions only the person can make

1. **What is "slow"?** Pick a number for time-to-first-page on a warm runner and on a cold one.
   Without one, a harness has nothing to fail on.
2. **Is a cold first browser acceptable?** The choice is: bake Chrome into the runner image
   (bigger image, an engine pinned by the image), or keep downloading (smaller image, a wait on
   the first browser, and a pin and signature check at run time).
3. **Should one stuck start block every other?** Remove the window-wide `starting` mutex and let
   the host enforce the budget, or keep serial starts as a safety.
4. **What is the longest a start may take before the person is told?** 62 minutes is today.
5. **Should the posture answer and the pool-login opt-in persist?** If so, for how long, in which
   tier (Machine, device-bound), and bound to what (pool image, pin, persona)? Persisting a login
   consent is a security rule, not a convenience.
6. **Should quit end a runner's browsers at all?** The runner keeps running whatever the window
   does. The rule from #2022 (ruled 2026-10-18) was about the person's own machine.
7. **When a pool is forgotten, do its browsers end as "pool forgotten", or stay, orphaned, on a
   machine purlis no longer talks to?** Today the second is true on the remote machine.
8. **What counts as real infra?** A nightly job against a remote Docker pool over SSH and a kind
   cluster that opens a real browser, or per pull request on browser paths. It costs minutes and
   needs the existing sshd and kind rigs.
9. **Is the pool setup flow to be reshaped or patched?** Thirty-three commits suggests the shape.
   A reshape is one decision; patches are many.
10. **Is M63 ("Browsers") one milestone or two?** G shows most of the cost is pool and link. A
    shared "remote" milestone would hold the fix once.
