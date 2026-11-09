# An author gets the port's types, a mock of it, and a check, with no plane

**2026-11-09, #5975.** Amends §7 of ADR 0122 with what building the author's tools decided.

- **Three commands, none reading a plane**: `purlis artifact types` prints `purlis-page.d.ts`,
  `purlis artifact mock` prints `purlis-page-mock.js`, and `purlis artifact check <folder>`
  checks a folder by its path. An author's folder may be in no project, and printing leaves
  where the files go to the author; nothing is written into an artifact's folder, whose every
  byte a person approves.
- **The types are generated from the catalogue** (`artifact::kit::types`), each query typed at
  every `@N` this build answers, from its tool's parameters and its pinned result schema. They
  are kept in `app/artifact-kit/`, where `npm run typecheck` checks what they allow and refuse
  (`purlis-page.check.ts`); a core test fails when the kept file is not what the catalogue
  generates (`ARTIFACT_KIT_BLESS=1` writes it again, and its diff is read).
- **The mock is written by hand, beside them**, run by the app's unit tests in a document of its
  own, and compiled into the binary. It answers in the catalogue's envelope under the page's
  own declaration, as purlis does: undeclared is `not-approved`, a declared data file comes from
  `purlis-fixtures.json` or the folder or is `null`, a query comes from its fixture or fails
  with a console warning, and offers are checked in purlis's closed types and only logged. It
  does not hold the catalogue; `check` does. **It steps aside when `purlis` is the window's own
  property**, not merely defined: a browser names the declaration block itself
  `window.purlis` by its id.
- **`check` reads the folder as purlis serves it** (`serve::Snapshot`): the declaration exactly
  as `data::declared` reads it, then every string shaped `"<source>.<name>@<N>"` in the pages
  and scripts, outside the declaration block, which must be declared as written. That reading
  is a lint of the author's source, for the author: it decides nothing, and the port refuses an
  undeclared call whatever it says.
