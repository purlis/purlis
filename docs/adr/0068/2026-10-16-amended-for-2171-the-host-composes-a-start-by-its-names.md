# Amended for #2171 and #2174, the session host composes a start by its names, 2026-10-16

Spec #1974 (ruled 2026-10-14) lists "a full start (persona, profile, size, resume, first message,
sandbox opt-out, shell)" among the session protocol's additions, and the amendment for #1998
left a start with no `launch` refused until the host could compose one. With the sandbox and
the egress proxy (#1999) and the hook socket and tokens (#2000) in the host, it composes one.
§1, §4, §5 and the amendments above are left as accepted, and this note adds to them; the
operator's rulings of 2026-10-15 on starts relayed over a link (the amendment for #2042) and on
a sandbox-only Codex profile (#2037) stand as ruled. Every item is **decided in implementation,
to be confirmed** (D-2171-1 to D-2171-9).

1. **One adapter from a start to its program** (#2174). Both binaries' hosts compose each start
   through one function of the core, `purlis_core::hoststart::spec`: a start with the window's
   `launch` is run by its words (`purlis_core::chatspec::from_launch`), and one with none is
   composed first. The host crate still links no core. *D-2171-1.*
2. **A start with no launch is composed with the code the window's start runs.**
   `purlis_core::start::ready` decides the profile, the persona and the sandbox; the harness is
   armed by the same function the window arms it with; its line under the sandbox, its
   environment and what its hooks read are made as the window makes them. The result is the
   launch the window would have composed, run by item 1's adapter. *D-2171-2.*
3. **Only what the project's files and this machine allow.** A start names its profile,
   persona, workspace, conversation and first message; each is looked up on the host's machine
   and held to the gates the window's start passes: a profile this machine declares and the
   person approved, a persona the project has, a workspace folder inside the project reached
   through no link, a conversation id that is one. A composed start holds no grant the window
   holds for one chat, and a person's own level of the sandbox is not taken at it. A shell and a
   start that names no profile are not composed. *D-2171-3.*
4. **Credentials are never composed in.** A profile that hands its program a key or a cloud's
   credentials is refused here, as it is on the window's hosted path: credentials are read where
   the window starts a program and never pass in a launch (#2036). *D-2171-4.*
5. **Its confinement is the host's, the program's own from the first.** The host starts the
   confinement the sandbox plans as the start is composed, owned by the program being started:
   it ends with the program, or with the start where the start fails. Its proxy holds no
   connection to ask the person about, and refuses instead. A host given no way to start a
   confinement composes no sandboxed start. *D-2171-5.*
6. **An opt-out is written before it runs.** A person's opt-out of the sandbox, which only
   `local-ui` may send (the amendment for #2042), is written in the host's event log under the
   start's own chat and run before anything runs; a start whose opt-out cannot be written is
   not started (ADR 0067 §7). *D-2171-6.*
7. **A first message goes on the harness's line, held to a handoff's rules.** It is the last
   word, by the route its harness takes one (`purlis_core::handoff::first_message_argv`); one a
   handoff would refuse, and one for a harness with no such route, is refused, never dropped.
   *D-2171-7.*
8. **No watcher of a project's Claude Code settings runs in the host**, so a sandboxed Claude
   Code chat it composes starts on the person's settings and purlis's, as the window starts one
   whose settings it cannot watch (ADR 0067, amended 2026-10-16). *D-2171-8.*
9. **No wire change.** The session protocol already carries every field a composed start names,
   so it stays at 1.11 and FD-27's table is unchanged: a start's `launch` stays `local-ui`'s,
   relayed over a link, and every scope that may start a program may start one by its names.
   *D-2171-9.*
