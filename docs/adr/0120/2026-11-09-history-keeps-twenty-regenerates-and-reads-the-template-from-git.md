# History keeps twenty Regenerates on this machine, and reads the template from git

**2026-11-09, #5966.** Amends §1 and §6 of ADR 0120 with what building an artifact's History
(spec #5948, story 47) decided.

- **A version is a saved change to the template or a kept Regenerate.** The template's versions
  are the newest twenty commits that changed its folder, read from the plane's git and never
  copied. The data's are the person's own Regenerates: each one whose data was placed keeps a
  copy of the declared data files it wrote. `crates/purlis-core/src/artifact/history.rs`.
- **Twenty Regenerates per artifact, then the oldest goes** (`history::MOST`). The bar's History
  says the number. A copy is written whole into a hidden folder and renamed into place.
- **Kept data is Machine tier, never in the plane** (§1: the data is each person's):
  `<config>/artifact-snapshots/<key>/<id>/`, keyed as the page's approval is, so a fork or another
  clone keeps its own (`docs/plane-format.md`). Only Regenerate keeps a copy; data a skill or a
  person writes by hand is not a version until a Regenerate writes it.
- **As of a version** pairs the template committed by then with the data kept by then. Where no
  commit holds the template by then (a LOCAL workspace's, or one never saved), today's template
  is used, and the bar says so; where no data was kept by then, the page's reads answer
  `data: null`, and the bar says that too.
- **A page as of a version is served as any artifact is, and acts on nothing now**: its own
  webview, the same CSP, no network, and an origin apart from today's page, so it never writes
  today's storage. It reads only the data kept then; a catalogue query, a subscription or an
  offer is `not-approved`, since none of them can be as of a date. Its script runs only when this
  machine's approval is of that very template. **Nothing in its bar approves it**: an approval is
  of the folder's one template, and approving an old one would displace today's; the app refuses
  it as well. It never reloads from the folder; Back to now opens today's page.
- **A version is named by its id and read again in the app**, never taken from the window as a
  commit or a path: only a kept folder's name or one of the folder's listed commits is one.
- **Compare** names the template's files added, removed or changed, and each data value that
  changed, by its path in the JSON (`data.json › open`), up to 200, each value cut at 120
  characters; either side may be now.
