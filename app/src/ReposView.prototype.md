# Repos view prototype (#7997, map #7990): never merged

**Question:** what does the one Repos view, which replaces Repos and Changes, look like and
read like at a glance, with 150+ branch folders across two repos?

**Open it:** `cd app && npm ci && npm run storybook`, then
http://localhost:6006/?path=/story/prototypes-repos-view-7997--a-grouped-list.
← and → (or the floating bar) switch variants A, B and C in every story. The bar's
Panel / Tall / Sidebar buttons switch the frame size: the Panel's left half at its default 30%
height, raised to 55%, or moved to the left sidebar. Actions are stubs: what each would do is
written under the frame.

- **A, grouped list:** a header per repo, then foldable groups (Needs you, Working, Finished,
  Dormant, Removed today). Each smart action sits on its group's header. Rows are title first.
- **B, ledger:** a strip per repo with a disk bar, a workspace disk line with Removed today as
  a popover, state chips, and one sortable table.
- **C, triage:** tiles (need you · finished GB · dormant GB · total) that filter one list of
  two-line rows, needs you first. Removed today is a toggle in the list's bar.

**Proposed needs-you states** (only these count on the Repos icon's badge): a conflict with
main, a failed check, a merge waiting for your press, behind main with uncommitted work and no
chat, and unwired (only when the plane carries a layer the folder lacks). Under disk pressure
the icon turns amber.

Data: `ReposView.prototype.fixture.ts` holds the real folder names under `.worktrees/purlis`
with their real ages, plus 38 real branch names standing in as the second repo's. Titles,
states and sizes are made up and scaled to the map's numbers.
