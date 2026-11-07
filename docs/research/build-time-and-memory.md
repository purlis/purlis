# Build time and peak build memory: the levers, what they cost, and their order for purlis

Research note, 2026-11-06. It answers one question: what are the standard levers for cutting
build time and peak build memory in this Tauri 2 + Rust workspace, what does each cost at runtime
and in CI, and in what order should purlis try them.

## How to read this note

Each claim is tagged with where it comes from:

- **[cited]**: read this session in the primary source linked. The Rust books were read from the
  offline copy that rustup installs with the 1.98.1 toolchain
  (`~/.rustup/toolchains/stable-aarch64-apple-darwin/share/doc/rust/html`). It is the same text
  as `doc.rust-lang.org`, one minor version behind the pinned 1.99.1. Tauri and tauri-specta
  were read from their crate sources in the local Cargo registry, at the versions `Cargo.lock`
  pins (tauri 2.12.5, tauri-macros 2.7.4, tauri-specta 2.0.0-rc.25).
- **[cited, not re-read]**: a primary source I know of but could not open, because the sandbox
  refused network fetches (see [Open questions](#open-questions)). Check it before you rely on
  it. A second pass the same day, with the network open, re-read most of them. Each one it
  confirmed or corrected is retagged **[cited]** in place, with the correction written in.
  The section "Tauri's own way to split commands" comes from that pass.
- **[repo]**: measured or recorded in this repository: ADR 0092 and its amendments, comments in
  `.github/workflows/*.yml`, issue #5277 and PR #5379 as summarised in the brief.
- **[inference]**: my reasoning from the above. Not established until it is measured.

**State of the profiles.** Since #5479 (ADR 0092, amended 2026-11-06), `release` and
`dev-release` both use `lto = "thin"`, `codegen-units = 16`, `opt-level = 3`,
`panic = "unwind"`, `strip = true` [repo].

## The answer

The peak is in one compiler process: the `rustc` that compiles `purlis_app_lib` at `opt-level = 3`
grows past the runner's memory even with `--jobs 1` [repo]. So the levers that act across
processes do not touch it: fewer parallel jobs, caching, a faster linker, or a different LTO mode
at the final link. Only two kinds of lever do. One gives that one `rustc` less to optimise:
a per-package profile override for `purlis-app`, or spreading the IPC dispatch over several
functions or crates. The other gives it more memory: a larger runner. Spreading the dispatch has
a catch. A `#[tauri::command]`'s wrapper is a `macro_rules!` expanded where
`generate_handler!` is called [cited], so moving command functions into other crates does not
move their wrapper code out of the crate that registers them. The split has to move the
registration (`generate_handler!` / `collect_commands!`) too.

The second pass (see [Tauri's own way to split commands](#tauris-own-way-to-split-commands))
found a likely culprit inside the wrappers. In a release build Tauri gives every `async` command
its own copy of the async runtime's task code. In a debug build it boxes them all into one copy,
"for a faster compile time" [cited]. So 573 copies at `opt-level = 3` exist only in the build
that runs out of memory. Tauri's sanctioned unit for splitting commands is the **plugin**. It
moves that code only if the plugin lives in its own crate and is not generic over the runtime.

In order, purlis should:
(1) measure where the one `rustc`'s memory goes: `-Z time-passes` on nightly, a peak-RSS wrapper
on stable, and the one-line experiment that switches Tauri's debug-only boxing on for a
measurement build;
(2) put the release Linux build on a 32 GB or larger runner, and raise or move off the macOS job's
self-imposed 90-minute limit;
(3) try `[profile.release.package.purlis-app]` with a lower `opt-level` or more codegen units,
gated by the cold-start and latency guards;
(4) if the command glue dominates, move the registration into a handful of Tauri plugins, one
small crate per area, sitting above the app library, with one real plugin crate built first to
prove the memory moves;
(5) ask upstream for the boxed path in release builds, which would cut the same code without a
migration.

A linker change, nightly-only flags and more caching come last for this problem.

## The levers, ranked for purlis

Ranked by expected payoff against the measured failure (a single `rustc` past 16 GB on Linux; a
65-68 min cold macOS build against a 90-minute limit), divided by cost.

| # | Lever | Build time / peak memory | Runtime cost | CI / dev cost | Stable? |
|---|---|---|---|---|---|
| 1 | **Measure first**: `--timings`, peak RSS per `rustc`, `-Z time-passes`, `-Z macro-stats`, `cargo llvm-lines`, and one build with `tauri`'s `debug-assertions` on, to test whether the per-command task code is the peak | None by itself. It decides which of 3-6 applies | None (measurement builds only; never ship the `debug-assertions` one) | A nightly toolchain for the `-Z` flags, used locally or in one non-shipping job | `--timings` yes. `-Z` flags nightly only |
| 2 | **Larger runner** for the release build (Linux 8 vCPU / 32 GB or 16 vCPU / 64 GB [cited]). **Raise the macOS job's `timeout-minutes`** (90 is the repo's own value; GitHub's limit is 6 h [cited]) | Memory: removes the OOM outright. Time: more cores help the crates that build in parallel, little for the one serial `purlis_app_lib` | None | Per-minute cost, and a payment method on file. One workflow edit. The release job stays cold, uncached and on one runner (ADR 0092) | Yes |
| 3 | **Per-package override** `[profile.release.package.purlis-app]`: `opt-level = 2`, `1` or `"s"`, and/or more `codegen-units` | Probably the largest code-free cut to that `rustc`'s LLVM time and memory [inference]. Not measured | Slower IPC glue, and any generic code instantiated in `purlis-app`. Under LTO it may also lower the optimisation of the whole program (see the lever's section) [inference] | Small. ADR 0092 needs an amendment, since `dev-release` may set nothing new | Yes |
| 4 | **Split the commands into Tauri plugins**, about eight areas, each a small crate above the app library, concrete over `tauri::Wry` (see [Tauri's own way](#tauris-own-way-to-split-commands)). Several handlers in one crate is the weaker fallback | Each area's wrappers, and their copies of the task code, compile in their own `rustc`, in parallel, after the library. The library's own front-end (300k lines) does not shrink | A string split, a mutex and a short scan per IPC call. A sync plugin command holds Tauri's plugin lock while it runs, so window-opening and slow sync commands must be `async` first | Large but mechanical: the library moves down one package; the commands become `pub`; invoke names become `plugin:<area>\|<cmd>` in 417 test files' mocks; one capability entry per area. The one list stays in `ipc_commands.rs` and `build.rs` (ADR 0052 amendment) | Yes |
| 4b | **Ask Tauri for boxed async commands in release** (the debug-only path of tauri-apps/tauri#13464) | The same cut as 4's task-code part, with no migration | One box and one indirect call per async IPC call (unmeasured) | An upstream issue or PR. A local fork or `[patch]` would break priority 2 (standard tools) | Upstream |
| 5 | **LTO mode and codegen units** for the shipped profile: `lto = false` (thin-local) or `"off"`, against the current `"thin"` | Saves the whole-program ThinLTO at the final link. Probably not the `purlis_app_lib` peak, which happens before that link [inference] | Thin → none: some speed and size lost, unmeasured for purlis. ADR 0092 measured fat → thin: zipped artifact +7 % on macOS, +2.5 % on Linux | One line. Needs an ADR 0092 amendment | Yes |
| 6 | **Linker**: lld/mold on Linux, ld-prime on macOS | Link step only. The repo measured the app's debug link at 6 s [repo]. rust-lld is already the default on x86_64 Linux since 1.90 [cited] | None | Low, but little to win | lld default: yes. mold: an external tool |
| 7 | **Caching** (PR CI and local): rust-cache, sccache, incremental | Cuts warm PR builds. Does nothing for the cold release build, which keeps no cache by design | None | Already in place (rust-cache saved on `main` only; sccache on the macOS legs). The repo's 10 GB of cache is already full [repo] | Yes |
| 8 | **Debug info, `split-debuginfo`, `strip`** | `release` already has `debug = false` (the default) and `strip = true`. `dev` already uses `line-tables-only` [repo] | None | None | Yes |
| 9 | **Proc-macro / build-script settings** (`build-override`) | Unknown until `-Z macro-stats` and `time-passes` show how much time goes to expansion | None (build-time crates only) | Low | Yes |
| 10 | **Parallel front-end** `-Z threads=N` | Faster front-end, but uses **more** memory (up to 35 % more): the wrong way for the OOM [cited] | None | Nightly only. Not for the shipped path | No |
| 11 | **Cranelift backend** for dev builds | Faster debug codegen, locally | Slower debug binaries. May not support unwinding, which `panic = "unwind"` needs for AppKit's exceptions [cited] | Nightly only | No |

## Tauri's own way to split commands

The operator's question: what is the idiomatic, Tauri-sanctioned way to structure an app with
hundreds of commands, so that the build runs in parallel and no single `rustc` or function holds
them all, without harming the app's runtime or developers' experience?

Sources for this section: the v2 docs, read live on `v2.tauri.app` and in their source
(`tauri-apps/tauri-docs`, branch `v2`, `src/content/docs/`); the crate sources at the locked
versions (tauri 2.12.5, tauri-macros 2.7.4, tauri-build 2.7.4, tauri-plugin 2.7.1, tauri-specta
and specta-macros 2.0.0-rc.25); and issues and code on GitHub, all read this session.

### What Tauri gives an app

- **One handler per app.** "To register multiple commands, you cannot call invoke_handler
  multiple times. Only the last call will be used. You must pass each command to a single call of
  `tauri::generate_handler!`." [cited,
  <https://v2.tauri.app/develop/calling-rust/#creating-multiple-commands>]. The source agrees:
  `Builder::invoke_handler` replaces the box (tauri `src/app.rs:1723`).
- **The docs' only advice for many commands is modules.** "If your application defines a lot of
  components or if they can be grouped, you can define commands in a separate module instead of
  bloating the `lib.rs` file." Command names are not scoped to a module, so they must be unique
  across modules. And "Commands defined in the `lib.rs` file cannot be marked as `pub` due to a
  limitation in the glue code generation." [cited, same page,
  "Defining Commands in a Separate Module"].
- **A plugin is the only unit with a handler of its own.** "A Tauri plugin is composed of a Cargo
  crate and an optional NPM package that provides API bindings for its commands and events." Its
  commands "are regular Tauri applications commands", registered with the plugin builder's
  `invoke_handler(tauri::generate_handler![…])`. The frontend calls them as
  `invoke('plugin:<plugin-name>|upload', …)` [cited, <https://v2.tauri.app/develop/plugins/>].
  In the source, `plugin::Builder::invoke_handler` sets the plugin's own handler
  (`src/plugin.rs:321`). `Webview::on_message` splits `plugin:<name>|<cmd>` and hands the call
  to that plugin (`src/webview/mod.rs:2068-2122`).
- **Plugin permissions.** By default a plugin's commands are not reachable from the frontend.
  In the plugin's `build.rs`, `tauri_plugin::Builder::new(COMMANDS).build()` generates an
  `allow-<command>` and a `deny-<command>` permission for each listed command. A capability then
  grants them as `<plugin>:allow-<command>`. Identifiers are lowercase ASCII and hyphens only
  [cited, plugins page, "Command Permissions", "Autogenerated Permissions", "Identifier Rules"].
  The default permission is not generated (tauri-plugin `src/build/mod.rs:97`), and the crate
  must set `links` (`src/build/mod.rs:161`). `tauri-apps/plugins-workspace` follows that layout
  for every plugin: `build.rs`, `permissions/`, `guest-js/`, `links = "tauri-plugin-<name>"`, and
  a generic `pub fn init<R: Runtime>() -> TauriPlugin<R>` [cited,
  <https://github.com/tauri-apps/plugins-workspace/tree/v2/plugins/clipboard-manager>].
- **A plugin without a crate of its own.** `tauri_build::InlinedPlugin` is the "Definition of a
  plugin that is part of the Tauri application instead of having its own crate". The app's
  `build.rs` declares it with `Attributes::plugin(name, InlinedPlugin::new().commands(&[…]))`,
  which generates the same `allow-<command>` permissions, optionally with
  `DefaultPermissionRule::AllowAllCommands` [cited, tauri-build `src/acl.rs:27-80`,
  `src/lib.rs:45-55`].
- **Nothing in the docs recommends any structure for build time or memory.** None of the pages I
  read mentions build time or memory as a reason for a structure: calling Rust, plugin
  development, App Size, Debug, Develop, capabilities and permissions. The App Size page's Cargo
  profile is for *size*: `codegen-units = 1`, `lto = true`, `opt-level = "s"`,
  `panic = "abort"`, plus nightly `-Zthreads=8` for "better compile performance" [cited,
  <https://v2.tauri.app/concept/size/>]. That is the opposite direction for build memory, and
  purlis needs `panic = "unwind"` (ADR 0092). The same page's `build.removeUnusedCommands` drops
  commands no capability allows [cited]. `build.rs` grants every listed command, so it would drop
  nothing [inference].

### Where the per-command code comes from

- **In release, every async command gets its own copy of the async runtime's task code.**
  `InvokeResolver::respond_async_serialized` (tauri `src/ipc/mod.rs:343-389`) boxes the future
  under `#[cfg(debug_assertions)]`, as "Dynamic dispatch the call in dev for a faster compile
  time". The comment adds "TODO: Revisit this and see if we can do this for the release build as
  well if the performance hit is not a problem". Without debug assertions it calls the generic
  `respond_async_serialized_inner::<F>`, which calls `async_runtime::spawn` on the command's
  future [cited]. The async wrapper calls it once per command (tauri-macros
  `src/command/wrapper.rs:355-390`) [cited]. So each of the 573 async commands instantiates the
  task machinery for its own future type, in the crate where its wrapper expands [inference from
  the code].
- **tauri-apps/tauri#13464** (merged 2025-05-29) made that change. Its `cargo llvm-lines` on a
  minimal app went from 1,231,833 lines in 66,222 copies to 399,528 lines in 17,715. The top
  lines before were `std::panic::catch_unwind`, `tokio::runtime::task::core::Cell<T,S>::new`,
  `tokio::runtime::task::harness::poll_future` and `Harness::complete`. A full debug build went
  from about 80 s to 70 s, an incremental one from about 10 s to 4 s, and about 7 s to 3.2 s on
  macOS, per a maintainer [cited, <https://github.com/tauri-apps/tauri/pull/13464>].
- **tauri-apps/tauri#7073** (open since 2023, Modrinth's app): most of the final crate's
  compile time is spent on `#[tauri::command]`s, "related to
  `tauri::InvokeResolver::respond_async_serialized` and `tokio::runtime::task::harness`" [cited,
  <https://github.com/tauri-apps/tauri/issues/7073>].
- **tauri-apps/tauri#14167** (open): a `generate_handler!` of 100+ commands overflows the stack
  at run time in release on Windows. A maintainer says the debug build avoids it "by not inlining
  the command functions". The wrapper's closure exists "for preventing stack overflow on Windows,
  see #12488" (`wrapper.rs:325`). One user reports that splitting the list into several
  `generate_handler!`s with manual routing did not cure it, and one `generate_handler!` per
  command behind a lookup did [cited, <https://github.com/tauri-apps/tauri/issues/14167>]. That
  is a run-time symptom of the same one big function, not build memory.
- [inference] This fits the measurements: 2-4 GB per `rustc` in the debug build, where those 573
  copies collapse into one boxed path, and about 26 GB in release, where 573 copies meet
  `opt-level = 3`. It is the first thing to test (below). If it holds, the code to move is
  wherever `generate_handler!` expands, which is exactly what a plugin crate moves.

### Plugins: what moves, and what does not

- **A plugin in its own crate moves its wrappers there.** That crate calls `generate_handler!`,
  so the argument parsing, reply serialisation and task copies of its commands compile in its
  `rustc`, in parallel with the other plugin crates [inference from the mechanism above].
- **Catch 1: a generic plugin moves nothing.** The usual shape is
  `init<R: Runtime>() -> TauriPlugin<R>` (plugins-workspace, tauri-specta's example, Modrinth).
  Generic code "may be generated in the crate where it is instantiated", and at `opt-level` 2 or
  3 a crate does not reuse other crates' monomorphisations [cited, Cargo book, profile
  overrides]. `init::<tauri::Wry>()` called from `purlis_app_lib` would generate the handler
  there again [inference]. purlis's plugin crates must be concrete:
  `pub fn plugin() -> TauriPlugin<tauri::Wry>`. The app's commands already use the default
  runtime, and `commands()` is `Builder::<tauri::Wry>` [repo, `lib.rs`].
- **Catch 2: plugins in the same crate (`InlinedPlugin`) give N handler closures in one
  `rustc`.** That splits the one big function but not the crate's volume. It is what Modrinth
  does: 22 inlined plugins (`auth`, `cache`, `import`, `jre`, …) declared in the app's
  `build.rs`, each area a module with its own `init<R: Runtime>()` and `generate_handler!`. Its
  logic lives in a separate library without Tauri, `theseus`. Its `build.rs` points at
  tauri-apps/tauri#10075, which asks for less boilerplate across the handler, `build.rs` and the
  capabilities [cited,
  <https://github.com/modrinth/code/blob/main/apps/app/build.rs> at `72f5aad680`,
  `apps/app/src/api/auth.rs`, <https://github.com/tauri-apps/tauri/issues/10075>].
  This is the nearest large open-source precedent. It is organisation, not build parallelism.
- **Catch 3: a crate cycle.** A plugin crate cannot depend on `purlis-app` while `purlis-app`
  registers it. Moving the commands' code into area crates is blocked: every command module
  reaches `crate::planes`, which is 15k lines and reaches most of the crate (`smartclose`,
  `dispatched`, `handoff`, `chats`, …) [repo]. There is a cheaper shape. Leave the commands'
  code where it is, and put the *registration* in small crates **above** the library: each one
  depends on the library and registers its area's `pub` commands. The macros allow this:
  `#[tauri::command]` marks its two hidden macros `#[macro_export]` for `pub` and `pub(crate)`
  functions and re-exports them at the function's path (`wrapper.rs:163-168`, `:340-345`), and
  `#[specta::specta]` does so for `pub` only (specta-macros `src/specta.rs:122-127`) [cited].
  So the commands must be `pub`, in `pub` modules, and the 71 commands in `lib.rs` move to a
  module, because commands in `lib.rs` cannot be `pub` [cited, docs above]. The `src-tauri`
  package becomes the top. It keeps `main.rs`, `build.rs`, `tauri.conf.json`, `capabilities/`,
  `generate_context!` and the `tauri::Builder`, and the library moves down one package
  [inference].
- **Run time.** A plugin call costs a `strip_prefix`, a split on `|`, a `String`, a
  `Mutex::lock` on the plugin store and a scan of the plugin `Vec` for its name
  (`webview/mod.rs:2068-2075`, `manager/mod.rs:465-471`, `plugin.rs:986-995`) [cited]. That is
  noise next to the JSON both paths parse [inference].
- **One real behavioural difference.** The plugin store's mutex is held while the plugin's
  handler runs (`manager/mod.rs:465-471`). A sync command runs inside the handler
  (`body_blocking`, `wrapper.rs:398-428`) and, per the docs, on the main thread [cited,
  calling-rust, "Async Commands"]. Creating a webview, page-load hooks and navigation hooks take
  the same mutex (`manager/webview.rs:134`, `:303`, `:622`) [cited]. So a sync plugin command
  holds those hooks back for as long as it runs. One that creates a webview on that thread would
  re-lock a `std::sync::Mutex` it already holds, which deadlocks or panics [inference]. Tauri's
  own window and webview `create` commands are `async` (`window/plugin.rs:66`,
  `webview/plugin.rs:34`) [cited]. Rule for purlis: a command that opens a window or webview, or
  any slow sync command, becomes `async` before it moves into a plugin. A rough search found no
  sync command that builds a window [repo]. Check it again during the move.

### Several handlers without plugins

- **Possible, but no Tauri document describes it.** One closure routes by
  `invoke.message.command()` to N `generate_handler!` closures, each in its own function. It
  must choose before it hands over the `Invoke`, because a handler takes the `Invoke` by value
  and its `_ => return false` arm drops it (tauri-macros `src/command/handler.rs`) [cited]. So
  handlers cannot be tried one after another. The only primary source for the pattern is the user
  comment on #14167 above.
- **Names, ACL and bindings stay as they are**, with one wrinkle. `collect_commands!` always
  expands a full `generate_handler!` as well (tauri-specta `src/macros.rs:43-49`) [cited]. Two
  ways around it [inference]:
  1. Build the bindings `Builder` only on the export path (debug, tests), and register the
     routed handler in every build.
  2. Use `tauri_specta::internal::command(handler, collect_functions![…])`. The crate says
     "Nothing in this module has to conform to semver so it should not be used outside of this
     crate" (`src/lib.rs:243-246`) [cited].
- **What it wins** [inference]: no single function holds 726 wrappers, which helps if LLVM's
  cost grows faster than one function's size, and helps the stack frame. Putting
  `#[inline(never)]` on each chunk's function stops LLVM from merging the chunks back. **What it
  does not win**: the 573 task copies stay in `purlis_app_lib`, and the crate is still one
  `rustc`. It tests and fixes only the "one giant function" hypothesis.

### tauri-specta with plugins

- **One `Builder` per plugin.** `Builder::plugin_name` sets the plugin "used when generating
  bindings for plugin commands", and the exporter then writes `plugin:<name>|<cmd>` into each
  call (`src/builder.rs:97`, `:147`, `src/name.rs:32-41`, `src/lang/js_ts.rs:168`) [cited]. A
  builder has one `plugin_name`, and `commands()` "will overwrite any previously registered
  commands" (`src/builder.rs:152-170`) [cited]. So there is one builder and one bindings file per
  plugin.
- **The reference is tauri-specta's `examples/custom-plugin`.** The plugin builds its own
  `Builder` with `.plugin_name(PLUGIN_NAME)`, passes `builder.invoke_handler()` to
  `tauri::plugin::Builder::new(PLUGIN_NAME)`, and writes its own `bindings.ts` from a test. Its
  `build.rs` is `tauri_plugin::Builder::new(COMMANDS).build()`, and the app's capability grants
  `specta-example:allow-add-numbers` [cited,
  <https://github.com/specta-rs/tauri-specta/tree/main/examples/custom-plugin>].
- **Known issues.** tauri-specta#163 asks for top-level exports in plugin mode. specta-rs/tauri-specta#266
  would have made `commands()` accumulate and added top-level exports, but it was closed
  **unmerged** on 2026-10-22. `v2.0.0-rc.25` (2026-05-08) is still the latest release [cited,
  GitHub API]. Types used by several plugins appear in each plugin's file. TypeScript compares
  types by structure, so that does no harm [inference].

### The answer for purlis

**Are plugins the idiomatic answer? Yes, with conditions.** Tauri allows one handler per app.
The only unit it ships with a handler, permissions and bindings of its own is the plugin, in its
own crate or inlined in the app's. The largest open-source Tauri app I could read (Modrinth)
organises its commands into plugins by area. But no Tauri document offers plugins as a cure for
build time or memory. They become one only under two conditions: each plugin is **its own
crate**, and it is **concrete over `tauri::Wry`**. Inlined or generic plugins leave the code in
`purlis_app_lib`. And the cheapest cut of the same code is upstream, not a plugin: Tauri's own
debug-only boxing of async commands (row 4b).

**Recommended shape** [inference]:

1. Keep `ipc_commands.rs` as the one list (ADR 0052), grouped by plugin instead of flat. Each
   group keeps the `value_free` / `vault_values` / `e2e_only` classes.
2. Move the library's code down one package, unchanged (for example `crates/purlis-app-lib`).
   `app/src-tauri` becomes the thin top: `main.rs`, `build.rs`, `tauri.conf.json`,
   capabilities, `generate_context!` and the `tauri::Builder` that adds the plugins.
3. Add one small crate per area, between the two: `pub fn plugin() -> TauriPlugin<tauri::Wry>`
   with its group's `collect_commands!`, a tauri-specta `Builder` with `.plugin_name(area)`, and
   nothing generic. Areas from the current modules, about 30-150 commands each:
   - `skills`: `skill*`, `routergrade`, about 117
   - `pools`: sandboxing, docker/cluster pools, machine and pool profiles, about 119
   - `projects`: opener, workspaces, windows, first run, personas, todos, about 98
   - `dispatch`: dispatch grants, hand-off, asking, stopping, proposals, about 76
   - `look`: extensions, themes, views, settings, about 80
   - `vaults`: vaults, secret asks, forge accounts, about 50. It holds the value-bearing class.
   - `browsers`: about 32
   - `chats`: the 71 now in `lib.rs`, plus the rest, about 150
4. The top `build.rs` declares each group as `InlinedPlugin::new().commands(group)`, and writes
   each area's `value-free` set (and the vault area's `vault-values`) from the list.
   `default.json` grants `<area>:value-free` for each area. `vault-values.json` grants
   `vaults:vault-values`. `InlinedPlugin` is documented for plugins inside the app crate. Its
   permissions are keyed by plugin name, and nothing in the source I read checks which crate the
   handler lives in [inference, to confirm in the proof below]. The standard alternative is a
   `tauri-plugin` `build.rs` and `links` in each crate, which splits the one list into eight.
5. The frontend imports a small barrel that merges the eight `commands` objects, so the 760 files
   that import `bindings` stay unchanged [repo count].

**Migration cost**:

- **Rust**: 726 commands, and the types in their signatures, become `pub`. Modules become `pub`.
  The 71 commands in `lib.rs` move to a module. The package moves. Window-creating or slow sync
  commands become `async`.
- **Frontend**: the generated invoke names change, but behind the barrel the call sites do not.
- **Tests**: 417 test files use `mockIPC` and match raw names (about 4,400
  `cmd === "…"`/`case "…"` sites, a rough count) [repo]. They need `plugin:<area>|<cmd>`, either
  rewritten mechanically from the list or stripped by a mock helper.
  - The e2e specs that call `e2e_only` commands by name need the same change.
  - `ipc.rs`'s ACL tests run on Tauri's mock runtime. A plugin concrete over `Wry` cannot load
    there, so those tests register accept-all stub plugins under the same names. They already
    use an accept-all handler (ADR 0052).
- **Governance**: an ADR 0052 amendment (bindings, ACL and capabilities by plugin), and ADR 0092
  if any profile changes.
- **Bonus**: lever 3's main unknown goes away. An `opt-level` override can then target the
  eight glue crates alone, and leave both the library and the binary that runs the ThinLTO
  alone [inference].

**Prove it before migrating.** Cheapest first, each measured with the peak-RSS wrapper in
[What to measure first](#what-to-measure-first):

1. *No code (minutes of setup, one build).* Turn on Tauri's debug-only boxing for a measurement
   build:
   `cargo build --locked --profile dev-release -p purlis-app --config 'profile.dev-release.package.tauri.debug-assertions=true'`.
   `cfg(debug_assertions)` inside `tauri` is decided when `tauri` compiles, so the boxed path is
   what `purlis_app_lib` instantiates [inference]. If `purlis_app_lib`'s peak falls by most of
   the 26 GB, the task copies are the giant, and both row 4 and row 4b will work. **Never ship
   this build.** The same switch turns on Tauri's devtools API and its detailed ACL error
   messages (`webview/mod.rs:2085`, `:2263`; `webview/plugin.rs:178`) [cited].
2. *One throwaway branch (an hour).* Delete about half the names from `ipc_commands.rs`'s
   registration, leaving the functions compiled, and build. The peak's drop is the upper bound
   of what moving registration out of the crate can win.
3. *One real plugin crate (a day).* Make the `skills` modules' commands `pub`. Add
   `crates/purlis-ipc-skills` with `plugin() -> TauriPlugin<tauri::Wry>` over them, and drop
   them from the library's list. Building `-p purlis-ipc-skills` needs no wiring to measure. If
   `purlis_app_lib`'s peak falls by about the skills' share, and the new crate's peak is that
   share, the shape is proven. Wire it in and run the ACL tests to confirm the `InlinedPlugin`
   assumption.

## The levers

### 1. Measure first

The brief's numbers say *which* `rustc` peaks. They do not say *which phase* inside it. That
decides between levers 3 and 4.

- **`cargo build --timings`** [cited]. It writes `target/cargo-timings/cargo-timing.html`, with
  each unit's duration, its "codegen" part in lavender, build scripts in orange, and a
  concurrency graph. The page's own tips include "Split large crates into smaller pieces" and
  "If there are a large number of crates bottlenecked on a single crate, focus your attention on
  improving that one crate to improve parallelism."
  <https://doc.rust-lang.org/cargo/reference/timings.html>. The `--timings=json` format was
  removed in 1.94.0-nightly. On nightly, `-Zsection-timings` adds per-section timings inside
  each `rustc` to the same report [cited].
  <https://doc.rust-lang.org/cargo/reference/unstable.html#section-timings>
- **`-Z time-passes`** (nightly). The unstable book only says "measure time of each rustc
  pass" [cited] (<https://doc.rust-lang.org/nightly/unstable-book/compiler-flags/time-passes.html>).
  Its output also prints the RSS before and after each pass. The unstable book does not say
  so, but rustc's source does: `print_time_passes_entry` gets `start_rss` and `end_rss` and
  prints the change [cited,
  <https://github.com/rust-lang/rust/blob/main/compiler/rustc_data_structures/src/profiling.rs>].
  That is the quickest way to see whether the memory grows in expansion, type checking,
  monomorphisation collection or LLVM.
- **`-Z self-profile`** (nightly) writes `.events`/`.string_data`/`.string_index` files for
  measureme's `summarize`, `inferno` (flamegraph) and `crox` (Chrome trace) [cited].
  <https://doc.rust-lang.org/nightly/unstable-book/compiler-flags/self-profile.html>
- **`-Z macro-stats`** (nightly, permanently unstable). It "measures the effect on code size of
  all used macros and prints a table": uses, lines and bytes per macro [cited]. This is the
  direct test of whether `#[tauri::command]`, `#[specta::specta]`, `generate_handler!` and the
  serde/specta derives dominate the crate.
  <https://doc.rust-lang.org/nightly/unstable-book/compiler-flags/macro-stats.html>
- **`-Z dump-mono-stats`** / **`-Z print-mono-items`** (nightly). These list the monomorphised
  items, aggregated by definition with a size estimate [cited].
  <https://doc.rust-lang.org/nightly/unstable-book/compiler-flags/dump-mono-stats.html>
- **`cargo llvm-lines`** (dtolnay, third party, works on stable). It counts LLVM IR lines per
  generic function, summed over its instantiations. Its README: it "measures the number and
  size of instantiations of each generic function", which "can disproportionately affect
  compile time, compiler memory usage, and the size of compiled executables" [cited].
  <https://github.com/dtolnay/cargo-llvm-lines>
- **Peak RSS per `rustc` on stable**: wrap `rustc` with `RUSTC_WRAPPER`. Cargo runs the wrapper
  with the real `rustc` path as its first argument [cited].
  <https://doc.rust-lang.org/cargo/reference/config.html#buildrustc-wrapper>. GNU `time`'s `%M` is
  the maximum resident set size in KB. See [What to measure first](#what-to-measure-first).

Why `--jobs 1` did not help [inference]. `build.jobs` caps how many compiler processes run at
once [cited] (<https://doc.rust-lang.org/cargo/reference/config.html#buildjobs>), and `rustc`'s
own codegen threads also take jobserver tokens [cited]
(<https://doc.rust-lang.org/rustc/jobserver.html>). So `-j 1` should already have serialised
both. That the build still died means one `rustc` in one phase holds more than about 19 GB
(16 GB of memory plus 3 GB of swap), and parallelism is not the problem.

### 2. A larger runner, and the macOS time limit

The non-code lever, and the only one sure to remove the OOM without measurement.

- The 90 minutes are the repo's own `timeout-minutes: 90` on the release build job
  (`release.yml`), not a GitHub limit [repo]. On GitHub-hosted runners "Each job in a workflow
  can run for up to 6 hours of execution time" [cited,
  <https://docs.github.com/en/actions/reference/limits>. The old "Usage limits" URL now lands
  on an index page].
  Raising the limit makes the stable build *fit*. It does not make it faster.
- `macos-latest` draws an M1 with 3 CPUs and 7 GB or an M2 Pro with 5 CPUs and 15 GB, by lot
  [repo, `ci.yml`'s `stress` comment]. So the 65-68 min stable build depends partly on the
  draw.
- GitHub's larger runners [cited, <https://docs.github.com/en/actions/reference/runners/larger-runners>]:
  - Linux: 4 vCPU / 16 GB, **8 vCPU / 32 GB**, **16 vCPU / 64 GB**, and up.
  - macOS `large`: Intel, 12 CPUs, 30 GB.
  - macOS `xlarge`: M2, 5 CPUs, **14 GB**.

  "You must have a valid credit card on file in order to use larger runners." **Correction:**
  `xlarge` gives no more memory than the 15 GB M2 Pro that `macos-latest` already draws. On
  macOS it buys a fixed draw, not memory. The only bigger macOS memory is the Intel `large`,
  which builds for x86_64. I did not find the plan requirement or the per-minute prices on that
  page. Read the billing page before you decide. The repo already runs on GitHub Enterprise's
  runners [repo, `ci.yml`].
- Cost at runtime: none. In CI: money. Under ADR 0092 the release job is still one runner per
  platform, cold, with no cache, so a larger runner changes no rule.
- ADR 0092's own amendment names this lever: "Going back to fat LTO is a new amendment, made
  when the build fits the runners again, for example on larger runners." [repo]

The 32 GB swap file works, but at swap speed [inference]. A runner with real memory should
also be faster than the swapping one, which matters on Linux.

### 3. Per-package profile overrides

What the Cargo book says [cited] (<https://doc.rust-lang.org/cargo/reference/profiles.html#overrides>):

- `[profile.<name>.package.<spec>]` overrides settings for one package. `"*"` covers every
  non-workspace member. `build-override` covers build scripts, proc macros and their
  dependencies. The first match wins, in that order.
- **"Overrides cannot specify the `panic`, `lto`, or `rpath` settings."** So LTO cannot be set
  per package. That is verified.
- `opt-level`, `codegen-units`, `debug`, `split-debuginfo`, `strip`, `debug-assertions`,
  `overflow-checks` and `incremental` can be overridden. The list above is the only exclusion.
- **Generics**: "the code may be generated in the crate where it is instantiated, and thus may
  use the optimization settings of that crate." Also: "If the opt-level is 2 or 3, then a crate
  will not use monomorphized generics from other crates, nor will it export locally defined
  monomorphized items to be shared."
- Profile settings are read only from the workspace root's `Cargo.toml`.

What it means for purlis [inference]:

- `purlis-app` holds both the library `purlis_app_lib` *and* the `charter-app` binary, so an
  override on the package covers both. Under `lto = "thin"`, the whole-program ThinLTO runs in
  the `rustc` that links the binary. I expect it to optimise at that `rustc`'s `opt-level`. If
  so, `opt-level = 2` on `purlis-app` lowers the post-link optimisation of purlis-core's code
  too, not only the app's. This is the main unknown of the lever. Two ways to check: compare
  the binary size and the `tools/bench.mjs`/latency medians with and without the override, or
  move `main.rs` to a small package of its own so the override covers only the library.
- The likely gain: LLVM at `opt-level = 3` on the app crate's IR is the step whose memory the
  debug build (`opt-level = 0`, 2-4 GB per `rustc` [repo]) does not show. Lowering it to 2 or 1
  is the cheapest test of whether LLVM is the peak.
- More codegen units (`codegen-units = 64` or `256` on `purlis-app`) make each LLVM module
  smaller, but do not split one huge function [inference]. If the dispatch closure (lever 4) is
  the giant, this helps less than the opt-level.
- Runtime cost: the IPC glue (argument deserialisation, response serialisation) in `purlis-app`
  is optimised less. The cold-start (2 s), latency-median and scenario guards are the test.
  ADR 0086 says every budget is measured on a `release` build, so they will catch it.
- Governance: ADR 0092 decision 7 and the guard test hold `dev-release` to its two settings, and
  `release` now equals it except for the name. Any override means amending ADR 0092.

### 4. Splitting the crate, and how Tauri registers commands

**How registration works** [cited, crate sources]:

- `#[tauri::command]` (tauri-macros 2.7.4, `src/command/wrapper.rs`) emits the function
  unchanged, plus two hidden `macro_rules!` macros: `__tauri_command_name_<fn>` and the wrapper
  `__cmd__<fn>`. The wrapper's body deserialises the arguments, calls the function and
  serialises the reply. It is a macro, so its code is generated **where it is invoked**. Both
  macros get `#[macro_export]` when the function is `pub` or `pub(...)`, and a `use` re-exports
  them at the function's path.
- `generate_handler![a, b, …]` (`src/command/handler.rs`) expands to **one closure**:
  `move |invoke| { match invoke.message.command() { "a" => __cmd__a!(a, invoke), …, _ => return false } }`.
  So all 726 wrappers are expanded inside one function, in the crate that calls the macro.
- tauri-specta's `collect_commands!` (rc.25, `src/macros.rs`) expands to
  `internal::command(::tauri::generate_handler![…], ::specta::function::collect_functions![…])`.
  The types and the handler come from the same list, in the same crate.
- `tauri::Builder::invoke_handler` (tauri 2.12.5, `src/app.rs`) *replaces* the handler
  (`self.invoke_handler = Box::new(invoke_handler)`), so an app has one handler. A plugin has
  its own (`plugin::Builder::invoke_handler`, `src/plugin.rs`), and tauri-specta has a
  `plugin_name` setting so the bindings name a plugin's commands (`src/builder.rs`).

**What follows** [inference]:

- In `lib.rs`, `app_commands!(register)` → `collect_commands!` → `generate_handler!` puts every
  command's wrapper (573 of the 726 are `async`, so most also produce a future type) into one
  closure. At `opt-level = 3` LLVM is likely to inline each single-use wrapper closure back into
  it, which makes one very large function. A large function is the textbook case for LLVM
  memory and time growing faster than its size, and more codegen units cannot split it. This is
  the first hypothesis to test with `-Z time-passes` and `cargo llvm-lines` (lever 1).
- **Moving command functions into new crates without moving the registration moves their
  bodies out, but not their wrappers.** The bodies compile in parallel elsewhere. The wrapper
  closure stays in `purlis-app`, and so does any generic code the bodies instantiate there.
- **To move the wrappers, move the `generate_handler!` call.** There are two shapes:
  1. *Several handlers in one crate.* Partition the command list (it already comes from one
     macro, `app_commands!`, in `ipc_commands.rs`). Give each partition its own
     `generate_handler!` in its own function, and compose them in the one closure Tauri takes,
     choosing the partition by command name *before* handing over the `Invoke` (a handler
     consumes it). The names stay the same, so the ACL that `build.rs` builds from the same list
     stays valid. tauri-specta still needs every command's types in one `Builder` for the
     bindings. Whether that can be had without also generating the full handler is an open
     question.
  2. *Tauri plugins in their own crates*, each with its own `invoke_handler`. This gives real
     crate-level parallelism and incremental isolation. But a plugin command is called as
     `plugin:<name>|<cmd>` and is permissioned per plugin. That changes the window's calls, the
     generated bindings and the capabilities, and is an ADR 0052-level change. The second pass
     found that this is Tauri's sanctioned unit. It also found the two conditions under which it
     moves memory: its own crate, and concrete over `tauri::Wry`. And it gives a shape that
     avoids moving the commands' code. See
     [Tauri's own way to split commands](#tauris-own-way-to-split-commands).
- Splitting `purlis-core` (about 860k lines) is the other half. It compiles before the app on
  the critical path. `--timings` shows whether the app waits on it.
- Cost: none at runtime beyond a name dispatch per IPC call. In CI, more crates also mean more,
  smaller cache entries, which sccache handles better (it does not cache crates that link
  [repo, `ci.yml`]).

### 5. LTO modes and codegen units

What the sources say [cited]:

- Cargo's `lto`: `true`/`"fat"` "attempts to perform optimizations across all crates within the
  dependency graph". `"thin"` "is similar to 'fat', but takes substantially less time to run
  while still achieving performance gains similar to 'fat'". `false` is "thin local LTO"
  across one crate's codegen units only, and does nothing if codegen-units is 1 or opt-level is
  0. `"off"` disables LTO. The `lto` setting also drives `-C linker-plugin-lto` and
  `-C embed-bitcode`. <https://doc.rust-lang.org/cargo/reference/profiles.html#lto>
- The rustc book adds: "For larger projects like the Rust compiler, ThinLTO can even result in
  better performance than fat LTO."
  <https://doc.rust-lang.org/rustc/codegen-options/index.html#lto>
- `codegen-units`: "More code generation units allows more of a crate to be processed in
  parallel possibly reducing compile time, but may produce slower code." The defaults are 16
  non-incremental and 256 incremental.
  <https://doc.rust-lang.org/cargo/reference/profiles.html#codegen-units>
- The Rust Performance Book's "Build Configuration" chapter: LTO "can improve runtime speed by
  10-20% or more, and also reduce binary size, at the cost of worse compile times". `lto =
  false` is thin local LTO, the default. Thin LTO is "likely to improve runtime speed and reduce
  binary size while also increasing compile times". Fat LTO "may improve performance and reduce
  binary size further (but not always)". **Correction:** for `codegen-units = 1` it gives no
  number, only "You may be able to improve runtime speed and reduce binary size, at the cost of
  increased compile times" [cited,
  <https://nnethercote.github.io/perf-book/build-configuration.html>].

The numbers purlis has [repo, ADR 0092]: fat/1 CGU → thin/16 CGU saved 522 s on macOS and 284 s
on Linux over the steps a dev publish keeps. The zipped artifact grew from 31.5 to 33.8 MB on
macOS and from 117.0 to 119.9 MB on Linux. The runtime-speed difference "has not been measured
for purlis" (the 2026-11-06 amendment).

For the memory problem [inference]: with `lto = "thin"` the libraries are compiled to bitcode
and optimised per module first, and the cross-crate ThinLTO happens when the binary links. The
measured peak is in `purlis_app_lib`, before that link, so changing the LTO mode may not move
it. Going from `"thin"` to `false` should shorten the final link and lower its memory, at a
runtime cost that is unmeasured. Without measurement (lever 1) I would not trade runtime for it.

### 6. Linkers

- **Linux x86_64**: rustc uses lld by default on `x86_64-unknown-linux-gnu` since 1.90
  [cited, release notes 1.90, rust-lang/rust#140525,
  <https://doc.rust-lang.org/stable/releases.html>]. `-Clinker-features=-lld` is the stable
  opt-out, and rustc falls back to the system linker if the C compiler rejects `-fuse-ld=lld`
  [cited, <https://doc.rust-lang.org/rustc/codegen-options/index.html#linker-features>]. The
  Inside-Rust/blog announcement is
  <https://blog.rust-lang.org/2025/09/01/rust-lld-on-1.90.0-stable/>. It says the same:
  `x86_64-unknown-linux-gnu`, 1.90.0, `-C linker-features=-lld` to disable [cited]. So
  the `ubuntu-24.04` (x86_64) builds already use lld, unless something in the build turns it
  off.
- **mold**: the Cargo book shows how to configure it (`-C link-arg=-fuse-ld=mold`) and warns
  that "the linker Rust uses is already fairly fast and the gains from switching may not be
  worth it". It notes the trade-off "Might not support all use-cases, in particular if you
  depend on C or C++ dependencies" [cited].
  <https://doc.rust-lang.org/cargo/guide/build-performance.html#use-an-alternative-linker>
- **macOS**: Apple's linker since Xcode 15 is the new `ld` ("ld-prime") [cited, not re-read:
  Apple's Xcode 15 release notes, which are outside this note's source list].
- **For purlis**: the app's debug link was measured at 6 s, and "another linker (6 s to win)"
  was rejected [repo, `ci.yml` scenario-steps comment]. In a thin-LTO build, the cross-crate
  optimisation runs inside `rustc` before the linker starts [inference], so a faster linker
  saves only the last, small step. Low value.

### 7. Caching for PR CI and local builds (not the release job)

- **Swatinem/rust-cache** is in use. Only `main` saves, because pull requests' caches pushed
  `main`'s out of the repository's 10 GB [repo, `ci.yml`]. Its README describes what it keeps
  and the advice for busy repositories. It "only caches the _dependencies_ of a crate", and has
  `save-if` for saving on one branch. It also has an opt-in `cache-workspace-crates`, "Useful if
  the workspace contains libraries that are only updated sporadically" [cited,
  <https://github.com/Swatinem/rust-cache>]. The repo comment confirms the dependencies-only
  default from its own runs.
- **sccache** is in use on the macOS scenario legs with the GitHub Actions cache backend. It
  hit 80-90 %, but "never caches a crate that links", which is why the app library became
  `crate-type = ["rlib"]` [repo]. The Cargo book points to sccache for sharing builds and sets it
  up with `RUSTC_WRAPPER` [cited, <https://doc.rust-lang.org/cargo/reference/build-cache.html#shared-cache>].
  sccache's own README lists what it cannot cache. "Crates that invoke the system linker
  cannot be cached. This includes `bin`, `dylib`, `cdylib`, and `proc-macro` crates", and
  "Incrementally compiled crates cannot be cached" [cited, <https://github.com/mozilla/sccache>].
- **Cargo incremental** is only for workspace members and path dependencies. It is on in `dev`
  and off in `release` by default. `CARGO_INCREMENTAL` overrides it [cited]
  (<https://doc.rust-lang.org/cargo/reference/profiles.html#incremental>). It pays locally, where
  `target/` lives on between builds, not in CI.
- **GitHub's cache limit**: 10 GB per repository, evicting the least recently used. The repo
  hit it [repo]. GitHub's docs: "By default, the limit is 10 GB per repository, but this limit
  can be increased by enterprise owners, organization owners, or repository administrators. Any
  usage beyond 10 GB is billed to your account". Entries unused for 7 days are removed, and
  eviction goes by last access [cited,
  <https://docs.github.com/en/actions/writing-workflows/choosing-what-your-workflow-does/caching-dependencies-to-speed-up-workflows>].
- **Cargo's workspace feature unification** (`[resolver] feature-unification = "workspace"`)
  avoids rebuilding a dependency because another member asked for different features. It is
  nightly-only for now [cited]
  (<https://doc.rust-lang.org/cargo/guide/build-performance.html#resolve-features-for-the-whole-workspace>).
  The repo already builds the app, the harness and the CLI in one cargo run for this reason
  [repo]. The release job builds `purlis-cli`/`persona-statistics` and the app in separate cargo
  runs (`release.yml`). Check with `--timings` whether `purlis-core` compiles twice there.
- Cost: none at runtime. For PR CI the gains are mostly taken already. The release job is out of
  scope by design.

### 8. Debug info, `split-debuginfo`, `strip`

[cited, <https://doc.rust-lang.org/cargo/reference/profiles.html#debug>]: `release` defaults to
`debug = false`. `"line-tables-only"` is the smallest setting that still gives backtraces with
file and line. `split-debuginfo` defaults to `unpacked` on macOS when there is debug info.
`strip = true` means `"symbols"`. The Cargo book's build-performance guide recommends
`debug = "line-tables-only"` for `dev` and `debug = false` for dependencies, for "faster code
generation" and "faster link times" [cited].

For purlis [repo]: `release` has no debug info and strips symbols. `dev` already uses
`line-tables-only`. `profile.mutants` uses `debug = "none"`. And `debug = none` did not stop the
macOS `stress` swapping. The one change left to test is
`[profile.dev.package."*"] debug = false` for dependencies: faster debug builds locally and in
the debug CI jobs, and no backtraces into dependencies' lines. This does not affect the release
build.

### 9. Proc macros and build scripts

- By default build scripts, proc macros and their dependencies are built at `opt-level = 0`
  with 256 codegen units, in every profile. `build-override` changes that. A dependency that is
  both normal and build-time may then be built twice [cited]
  (<https://doc.rust-lang.org/cargo/reference/profiles.html#build-dependencies>).
- [inference]: 726 `#[tauri::command]` + `#[specta::specta]` pairs and the serde/specta derives
  all run proc-macro code compiled at `opt-level = 0`. `[profile.release.build-override]
  opt-level = 3` may speed up expansion, at the cost of compiling the proc macros more slowly.
  Whether it pays depends on how much of `purlis_app_lib`'s time goes to expansion: read it from
  `-Z time-passes` (`expand_crate`) and `-Z macro-stats`.
- `app/src-tauri/build.rs` `include!`s `ipc_commands.rs` and builds the ACL manifest from it
  [repo]. `--timings` shows its cost in orange. It is probably small.
- On nightly, `-Z cache-proc-macros` caches derive expansions in the incremental system, but
  does not check that a macro is pure [cited,
  <https://doc.rust-lang.org/nightly/unstable-book/compiler-flags/cache-proc-macros.html>].
  That is for local builds only.

### 10. The parallel front-end (`-Z threads`)

- The Cargo book: `rustflags = "-Zthreads=8"`, "faster build times (both cargo check and cargo
  build)", "Requires using nightly Rust and an unstable Rust feature", at most 8 threads
  recommended [cited]
  (<https://doc.rust-lang.org/cargo/guide/build-performance.html#enable-the-experimental-parallel-frontend>).
  The unstable book lists `threads` as having no tracking issue [cited].
- The announcement post, "Faster compilation with the parallel front-end in nightly" (2023), also
  reports higher memory use: "Memory usage can increase significantly in multi-threaded mode.
  We have seen increases of up to 35%" [cited,
  <https://blog.rust-lang.org/2023/11/09/parallel-rustc/>].
- For purlis: not on stable 1.99.1, and higher memory is the opposite of what the Linux build
  needs. It could speed up local `cargo check` on a machine with memory to spare.

### 11. Cranelift for dev builds

- The Cargo book [cited]: install `rustc-codegen-cranelift-preview` on nightly, set
  `codegen-backend = "cranelift"` in `[profile.dev]`, and enable the unstable `codegen-backend`
  Cargo feature. Trade-offs: "faster code generation", "Requires using nightly Rust",
  "Worse runtime performance of the generated code", "Only available for certain targets",
  "Might not support all Rust features (e.g. unwinding)".
  (<https://doc.rust-lang.org/cargo/guide/build-performance.html#use-an-alternative-codegen-backend>,
  <https://doc.rust-lang.org/cargo/reference/unstable.html#codegen-backend>). It can be set per
  package (`[profile.dev.package.foo] codegen-backend = "cranelift"`).
- For purlis [inference]: debug builds only, locally, at most. The app depends on unwinding
  through `extern "C-unwind"` frames for AppKit's exceptions (ADR 0092, 2026-11-03), which is
  exactly the case the book warns about. Not for CI, where the scenario tests should run the
  LLVM-built app.

### Not a lever here: `-Z hint-mostly-unused`

It defers code generation for a dependency until a later crate uses it [cited,
<https://doc.rust-lang.org/nightly/unstable-book/compiler-flags/hint-mostly-unused.html>]. It is
meant for large dependencies that are mostly unused, which neither purlis crate is.
Nightly only.

## What to measure first

The commands assume the thin-LTO profile and a built frontend. The app crate compiles `app/dist`
into itself (`frontendDist: "../dist"`). Run on a Linux machine with more than 26 GB of memory, or
on a CI branch job with the 32 GB swap file. Nothing here touches the release job.

```sh
# 0. The frontend the app crate embeds.
npm --prefix app ci && npm --prefix app run build

# 1. Where the time goes, and what waits on what. Stable.
cargo build --locked --profile dev-release -p purlis-app --timings
#    → target/cargo-timings/cargo-timing.html: purlis-core vs purlis_app_lib vs charter-app (the link).
#    Repeat for the CLI step as release.yml runs it, to see whether purlis-core compiles twice:
cargo build --locked --profile dev-release -p purlis-cli -p persona-statistics --timings

# 2. Peak RSS per rustc, stable (Linux, GNU time; `sudo apt-get install time` if it is missing).
cat > "$RUNNER_TEMP/rss-rustc" <<'EOF'
#!/bin/sh
crate=$(printf '%s\n' "$@" | awk 'p{print;exit} /^--crate-name$/{p=1}')
exec /usr/bin/time -a -o "${RSS_LOG:-/tmp/rss.log}" -f "%M KB %e s $crate" "$@"
EOF
chmod +x "$RUNNER_TEMP/rss-rustc"
RUSTC_WRAPPER="$RUNNER_TEMP/rss-rustc" RSS_LOG=$PWD/rss.log \
  cargo build --locked --profile dev-release -p purlis-app
sort -n rss.log | tail -5        # the largest rustc processes, with their wall time

# 3. Which phase of purlis_app_lib holds the memory. Nightly, for the measurement only.
#    (Build the dependencies first with the same toolchain, so only the last crate is timed.)
cargo +nightly rustc --locked --profile dev-release -p purlis-app --lib -- -Z time-passes 2> time-passes.txt
cargo +nightly rustc --locked --profile dev-release -p purlis-app --lib -- -Z macro-stats 2> macro-stats.txt
cargo +nightly rustc --locked --profile dev-release -p purlis-app --lib -- -Z dump-mono-stats=mono

# 4. Which generic functions (and closures) produce the most LLVM IR. Stable, third-party tool.
cargo install cargo-llvm-lines
cargo llvm-lines --locked --profile dev-release -p purlis-app --lib | head -40

# 5. The cheapest experiment for lever 3: the same build with a lower opt-level on purlis-app,
#    without editing Cargo.toml (profile overrides can be set from config).
cargo build --locked --profile dev-release -p purlis-app \
  --config 'profile.dev-release.package.purlis-app.opt-level=2'
#    Compare: rss.log's peak, the build's wall time, the binary's size, and then the cold-start
#    and latency jobs on that build.

# 6. Is the per-async-command task code the peak? Tauri's debug-only boxing, switched on for
#    the tauri crate alone. A MEASUREMENT build only: it also enables Tauri's devtools API.
RUSTC_WRAPPER="$RUNNER_TEMP/rss-rustc" RSS_LOG=$PWD/rss-boxed.log \
  cargo build --locked --profile dev-release -p purlis-app \
  --config 'profile.dev-release.package.tauri.debug-assertions=true'
#    Compare purlis_app_lib's line in rss-boxed.log with rss.log from step 2.
```

Read the results this way [inference]:

- If `time-passes` shows the RSS rising in LLVM (`LLVM_passes`, `codegen_module`) and
  `llvm-lines` shows the handler closure at the top, the dispatch is the giant. Then lever 3
  (opt-level) is the quick fix and lever 4 (split the dispatch) the lasting one.
- If it rises in expansion, type checking or MIR building, the front-end is the giant, and
  `macro-stats` names the macro. Then splitting the crate (lever 4) is the remedy, and the
  opt-level will not help.
- If the CLI step recompiles `purlis-core`, unifying the features of the two steps saves a whole
  `purlis-core` build in the release job.

## Measured

Lever 3 was measured on the hosted runners on 2026-11-06 and 2026-11-07 (#5277), and
`opt-level = 1` on `purlis-app` shipped (ADR 0092 amended 2026-11-07). The build was the one
`ci`'s outcome-bars job runs, `npx tauri build --features e2e ... --profile dev-release`, with
each candidate set by `--config profile.dev-release.package.purlis-app.<key>=<value>`. Main's
settings were built twice (`base-a`, `base-b`), to show the noise between two identical builds.
The throwaway workflow was on the `measure-build-memory` branch; runs 42279014678 and
42299963320.

**Peak memory, the whole build on Linux** (GNU `time -v`, plus a `rustc` wrapper logging each
crate's peak) [measured]:

| | Run 1 | Run 2 |
|---|---|---|
| main (`opt-level = 3`, 16 units), twice | 14.45 / 14.46 GB | 14.41 / 14.46 GB |
| `opt-level = 2` | 14.50 GB | |
| `opt-level = 3`, `codegen-units = 4` | 14.51 GB | |
| `opt-level = 1` | 6.58 GB | 6.61 GB |

In every case the peak was `purlis_app_lib`'s own `rustc`. Level 2 did not move it at all and
level 1 halved it, so the memory is in LLVM's level-3 work on the app crate, not in the
front-end (open question 3). `opt-level = 1` also built faster: 16 min 2 s against 22 min 43 s
and 24 min 40 s. On macOS, where memory is not the limit, cargo's largest child peaked at 4.1 to 4.6 GB with main's
settings and with `opt-level = 1` alike (BSD `time -l`), so that number says little.

**At run time, `opt-level = 1` against `base-a`** [measured]:

- Cold start on Linux (`tools/coldstart-linux.sh`, interleaved ratio; bare, i3, i3 without the
  session bus): 1.011 / 1.009 / 0.995 in run 1, 1.006 / 1.013 / 0.999 in run 2. `base-b` gave
  0.989 to 1.002.
- The outcome bars' openings on codex's stand-in, medians of two rounds each:

  | | Run 1, Linux | Run 1, macOS | Run 2, Linux | Run 2, macOS |
  |---|---|---|---|---|
  | `base-a` | 424 / 408 ms | 236 / 248 ms | 443 / 424 ms | 272 / 288 ms |
  | `base-b` | 407 / 376 ms | 217 / 232 ms | 435 / 451 ms | 218 / 204 ms |
  | `opt-level = 1` | 378 / 375 ms | 236 / 233 ms | 437 / 452 ms | 273 / 213 ms |

  No difference beyond the gap between the two copies of main, which on macOS was 25 %. The
  operator's own Mac opened the `opt-level = 1` app's bars under their 300 ms budget.
- Not measured: claude's and opencode's stand-ins did not run in the measurement job, on main's
  build as well (the picker did not close after *Approve and start*, where `ci` passes at the
  same commit). The session layer's latency bench builds only `purlis-session-protocol`, which
  the override does not reach.

So open question 2 has a practical answer: whether or not ThinLTO now optimises purlis-core's
code at level 1 too, no measured opening or start got slower.

## Open questions

1. **Unverified sources.** In the first pass the sandbox refused web fetches. The second pass
   re-read, and confirmed, these:
   - the Rust Performance Book;
   - both Rust blog posts;
   - GitHub's docs on job limits, larger runners and caches;
   - the rust-cache, sccache and cargo-llvm-lines READMEs;
   - rustc's `time-passes` output format;
   - Tauri's v2 docs.

   It corrected three: the Performance Book gives no number for `codegen-units = 1`; macOS
   `xlarge` has 14 GB; rust-cache can cache workspace crates on request. Still unverified:
   Apple's ld-prime (outside this note's sources), GitHub's per-minute prices and plan
   requirement for larger runners, and whether `doc.rust-lang.org` (1.99) differs from the
   offline 1.98.1 Rust books in anything cited. Those were not re-fetched, because the offline
   copy is the same text one version behind.
2. **Does a per-package `opt-level` on `purlis-app` lower the ThinLTO optimisation of the whole
   binary**, because the binary that performs the LTO is in that package? I believe it does,
   but this is unverified. It decides whether lever 3 is cheap or costly at runtime.
3. **Which phase of `purlis_app_lib` peaks?** LLVM on the one dispatch closure, or the front-end
   on 300k lines plus expansions? Measurement 3 answers it.
4. **Can tauri-specta export the bindings for all commands while the handler is built in
   several parts?** Read in rc.25: `collect_commands!` always expands a full `generate_handler!`
   as well. A `Builder` has one `plugin_name`, and `commands()` overwrites. So either build the
   full bindings `Builder` only on the export path (debug and tests) and register a routed
   handler, or use the non-semver `tauri_specta::internal::command`. For plugins: one `Builder`
   and one bindings file per plugin (see
   [tauri-specta with plugins](#tauri-specta-with-plugins)).
5. **Runtime cost of thin against fat LTO for purlis** is unmeasured (ADR 0092 amendment). If a
   larger runner brings fat LTO back into reach, measure it with `tools/bench.mjs` before
   deciding it is worth the build time.
6. **Larger-runner sizes and prices** for this organisation's plan, and whether the `release`
   environment's jobs may run on them.
7. **Does anything turn off rust-lld** on the Linux release build, for example a `-fuse-ld` from
   a C dependency's build or the AppImage tooling? `-C link-args=-Wl,--version` or the verbose
   link line would show it. Low priority, given the 6 s link.
8. **Does `InlinedPlugin` accept a plugin whose handler lives in another crate?** The ACL is
   keyed by plugin name, and I found nothing that checks where the code lives. The proof's
   step 3 (wire one plugin crate in, run `ipc.rs`'s tests) settles it.
9. **Would Tauri take boxed async commands in release** (the TODO in `respond_async_serialized`,
   tauri-apps/tauri#13464)? Its run-time cost is one allocation and one indirect call per async
   IPC call. Nobody has measured it. Ask upstream once step 1 of the proof shows the size of the
   win.
