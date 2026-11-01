# PROTOTYPE: Skills view, a plugin the chat does not have (#4477)

Throwaway. It lives on branch `prototype/skills-plugin-off` and never merges to main.

## The question

How should the Skills view say that a Claude Code plugin is installed on the machine but not in this
chat, and which way of writing the off row reads best?

## Open it

The published page, or rebuild it: `cd app && npx vite build --config src/prototype-skills-plugin-off/vite.standalone.config.ts`.
The bar at the bottom is the prototype's: it switches the state, the way of writing and dark or light.
Nothing reads a real plugin: names and counts are stand-in, and the states were measured on 2.1.316.

## Verdict

Not seen yet.
