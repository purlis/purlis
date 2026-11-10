# PROTOTYPE: the "Whose chats" picker lists the project's personas (#2935)

Throwaway. It lives on branch `prototype/studio-whose-personas` and never merges to main.

## The question

How does the Skill studio's "Whose chats" picker list the project's personas beside its root and
workspaces, and does it offer a persona in a workspace (a persona dispatched into one), which the
core already answers (`in_the_studio(…, workspace, persona)`)?

## Open it

- Storybook: `cd app && npm run storybook`, then **PROTOTYPE / Whose chats, personas**.
- One page: `cd app && SP_OUT=$TMPDIR/whose npx vite build --config src/prototype-studio-whose-personas/vite.standalone.config.ts`,
  then open `$TMPDIR/whose/standalone.html`.

`?variant=A|B|C` and `?moment=library|persona-page|many`; ← → switch variants. The bar at the
bottom is the prototype's. Names and counts are stand-in.

## Variants

- **A, one menu, a Personas section.** Today's menu grows "As a persona"; with a persona picked,
  "In which workspace" (the workspaces it was dispatched into) takes the place of "Where it starts".
- **B, two pickers, Who and Where.** Any persona with any workspace or clone.
- **C, one list, pairings named.** One filtered select; a persona in a workspace only where it was
  dispatched.

## Verdict

Not seen yet.
