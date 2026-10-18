# Amended 2026-10-14 (the operator's ruling on spec #2008, built in #2242): a chat without the sandbox gets a browser like a sandboxed one

Under the ruling of 2026-10-14 (a chat started without the sandbox gets browsers, logins and vault
values like a sandboxed one) and D-2011-10 (a browser takes only what is listed), the browser of a
chat started without the sandbox is built as follows. It supersedes item 5 of the amendment *a
browser's new sites are judged by its driver*.

1. **It lists what a sandboxed chat of its persona would list here.** Such a chat has no record of
   a sandbox compiled at its start, so its browser's listing is compiled at each browser call from
   the project's sandbox settings, as a chat's start compiles them, for the persona the app
   records the chat as (`purlis_core::sandbox::browser_listing`): the project's hosts, the
   persona's hosts as the person on this machine allowed them, the hosts and services on this
   machine the person allowed for the project, and the hosts an administrator's policy pins as
   never allowed; and the browser sites allowed here. Nothing more: never the floor's reach, and
   never another persona's hosts. An Allow given since reaches its next call. *Decided in
   implementation, D-2242-1, to be confirmed.*
2. **Its sites ask as any chat's do.** A site new to the project that its browser held back waits
   on the person in the registry, with the same answers. *Decided in implementation, D-2242-2,
   to be confirmed.*
3. **Its Ask says what each answer gives it.** The answers are the same and stand in the same
   place. The line says that the chat has no sandbox but its browser does, and that this
   project's sites are the hosts a sandboxed chat of its persona lists and the services on this
   machine the person allowed. *Decided in implementation, D-2242-3, to be confirmed.*
4. **A chat without the sandbox is one with no sandbox compiled for it and none holding it.** A
   sandboxed chat reattached under a sandbox that changed while no window was there has no
   compiled sandbox recorded but is still held: its browser lists nothing, and its Ask is a
   sandboxed chat's. This is the rule a browser login of such a chat is read by (#2560).
