# A data file is read on the port and never served as a document

**2026-11-10, spec #5948, from the adversarial review of #5978.** Amends §3 and §4 of ADR 0122.

§4 leaves the data files (`data.json`, and anything under `data/`) out of the approval because
they are inert JSON. The artifact scheme still served them by path, as any file of the folder,
so a page placed there (a teammate's `git add -f data/details.html`, or a Regenerate writing
it) loaded as a document of the approved template, under its CSP, and ran its script with the
page's port. Being outside the approval is only safe if being outside the document is too.

- **The artifact scheme never serves a data file.** A request for `data.json` or a path under
  `data/` is answered not found, whatever its type, and the webview's navigation guard refuses
  it before a request is made (`purlis_core::artifact::serve::Snapshot::served`,
  `Artifacts::may_navigate`). The snapshot still reads them, so History and the size cap see
  the folder whole.
- **A page reads its data only on its port**, as the amendment of 2026-11-09 on declaring data
  already said: a declared name, read from disk at the ask, answered as JSON data. That read is
  unchanged, so no declared read is lost by this.
- **This was chosen over serving data with a type that cannot run** (`application/octet-stream`
  and a `sandbox` CSP): nothing served at all leaves nothing to get wrong, and no page needed it.
