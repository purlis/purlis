# A chat's latest prompt routing is shown beside its start's

**Amends [ADR 0102](../0102-a-skill-router-decides-what-a-chat-sees-and-nothing-it-hides-is-out-of-reach.md), 2026-11-01**, ticket #4297 (SK-15m). §6 says the chat's tab and the studio show the router's timings and its fallbacks. Until now they showed the start's alone, so a router slow on every prompt looked healthy.

## What is kept

Each open chat keeps the routing record of its latest prompt, in memory, for as long as it runs. It is the record `skill.routed` already writes, so it holds no word of the prompt: its reason is purlis's own sentence. Nothing new is written to disk. A chat started again or reattached has none until its next prompt, and a router that reads no further prompt (turned off, or no longer approved) leaves none. The event log keeps every prompt's.

## What is shown

- **The Routing page's row.** It keeps one mark. A start that fell back marks it, as the chat's whole list is the built-in router's. Otherwise a last prompt that fell back marks it, "Last prompt fell back". Otherwise it shows the start's timing. The line under it is cut at its end, so it leads with what the mark says, then the other.
- **The chat's page on Routing.** A "Last prompt" fact says its timing, or that it went on with no pointer and why.
- **The chat's tab.** A last prompt that fell back is a need in the Skills view, with the routing need's fix. One that was answered is a line of the view's header. A start and a last prompt that both fell back are one need.

The row, the fact and the need are the parts `prototype/studio-secondary-pages` and `prototype/skills-sidebar` drew, so no new prototype was drawn for this.

## What it does not do

The window is not told as each prompt is routed. An open page shows a prompt's routing when it next reads: when it is opened, when the project comes back to the front, or at a change it follows. The Skills chip on a chat's pane does not say a prompt's routing, because its set is read at a chat's start and when skills change on disk, so it would stand stale either way until the window is told.
