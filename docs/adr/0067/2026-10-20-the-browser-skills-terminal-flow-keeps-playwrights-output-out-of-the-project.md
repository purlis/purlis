# The browser skill's terminal flow keeps Playwright's output out of the project, 2026-10-20

This note adds to *A browser login is performed in the chat's own browser* and changes none of
its rulings. Its item is **decided in implementation, to be confirmed** (D-2686-1).

1. **The `browser` skill's terminal flow points Playwright's output at a private folder outside
   the project, and removes it when the flow ends** (#2686). `playwright-cli` writes its
   snapshots, logs and traces under `.playwright-cli/` in the folder it runs from. A trace of a
   logged-in run holds that run's cookies, and a project's autosave commits what the project
   does not ignore. The flow makes a folder with `mktemp -d` and stops if it cannot. Before
   `open`, it sets `PLAYWRIGHT_MCP_OUTPUT_DIR` to that folder, which `playwright-cli` reads when
   a session starts, as it reads the secrets file. When the flow exits, however it exits, it
   closes the session and removes the folder. `crates/purlis-core/tests/it/the_browser_skills_terminal_flow_keeps_playwrights_output_out_of_the_project.rs`
   holds the flow to it. *D-2686-1.*

   *Rejected:* having the skill tell the project to ignore `.playwright-cli/`. That needs every
   project to carry the line, and autosave would need a rule of its own, and the traces would
   still be left on disk with live cookies in them. *Rejected:* a folder in purlis's own data,
   because that is a new store, and traces kept after a run are what the skill's hard rules say
   never to keep.
