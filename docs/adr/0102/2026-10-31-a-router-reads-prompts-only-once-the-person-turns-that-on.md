# A router reads prompts only once the person turns that on

**Amends [ADR 0102](../0102-a-skill-router-decides-what-a-chat-sees-and-nothing-it-hides-is-out-of-reach.md), 2026-10-31**, operator ruling, ticket #3231 (SK-15c). It adds to [the 2026-10-28 amendment](2026-10-28-a-router-that-reads-prompts-points-each-prompt-as-built.md), which let a router's approval alone decide. An approval is now not enough.

## The ruling

Prompt-reading routers are allowed. They are opt-in per router and off by default.

- A router sees prompts only after the person turns that on for that router, on a surface with a line saying it reads every prompt.
- It stays confined, with the 100 ms deadline. Past it the prompt goes on with no pointer, and the record says it fell back.
- Prompt text is never logged or written to any record.

## As built

**The switch is on the Skill studio's Routing page**, a **Prompts** row among the page's facts. It shows only where the chosen router's approval said it reads prompts. The line is on screen whether the switch is on or off.

**The choice is this machine's.** It is the `reads-prompts` list in `<config>/extensions.json`, beside the approvals: a list of its own, so a new approval leaves it. No project file, workspace setting or persona can turn a router on. The command that writes it is window-only, so a chat or another link cannot press it. A purlis that does not know the list drops it when it rewrites the record, which leaves every router off. Removing an extension takes it off the list, and so does approving the same id from another folder, which is another program. An approved update in the same folder keeps it, since its new approval prompt says it reads every prompt.

**One switch per router, for every chat it routes on this machine.** The ruling says per router, and a router's reading is a fact about that program, so there is no second choice per workspace.

**On reaches chats started after; off reaches the next prompt.** A chat's prompt hook is armed at its start, so a chat already open does not begin handing prompts over. Each prompt reads the record again before the deadline's clock starts, so a router turned off reads nothing more, and no record is made. A reattached chat keeps what its list left out and is held by the same read. A turn whose prompt no router routed names no earlier prompt's routing on its skill runs.

**The executor holds the same line.** Its prompt question is refused before any program starts unless the router is on the list.

**A record holds none of what a router says on a prompt.** A router could echo the prompt in an error or a wrong answer, so the routing record's reason is purlis's own sentence (`could not be asked about this prompt`, `gave an answer about this prompt that purlis does not take`). Its `pointed` are only names from the skills it was asked about. What a confined router does with a prompt inside its own state folder is its own, which is why the line says it reads every prompt.

## Not yet

The routing record is where a prompt's timing and fallback are said today. The Routing page and the chat's tab still show the start's alone (see the M65 ticket named in #3231's PR).

## Grades

A grade's chat is a real chat. A router's arm gets its pointer only where the switch is on, as a person's chat does. See [the amendment on grades](2026-10-28-a-router-is-graded-on-its-library-s-trigger-tests-as-built.md).
