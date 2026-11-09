# An extension's template is pinned as a copy

**2026-11-09, #5973.** Amends §1 and §9 of ADR 0120 with what building extension artifacts
decided.

- **A pin of an extension's template copies it into the workspace** as
  `artifacts/<extension-id>.<name>/`, then pins that folder through the one pin path, so §1 holds:
  an artifact is a folder of the workspace's `artifacts/`, shared with it. Its data files are not
  copied. A folder already there is never written over; to take a newer template, unpin and
  remove the copy, then pin it again.
- **The copy is the workspace's page from then on.** The extension's approval covered the bytes
  it shipped; the copy's script asks each person, over the copy's fingerprint, as §4 says any
  page's does. A teammate without the extension is answered `needs-extension` for its queries.
- **A template has no producing command**, so no Regenerate: an extension's page reads live data
  through its queries (#5971).
- **Where it is offered**: `purlis artifact templates` and `purlis artifact pin --template`.
- **Not yet**: the window's pin dialog does not list an extension's templates, and the pins file
  does not record which extension a pin was copied from (`docs/plane-format.md` is unchanged).
