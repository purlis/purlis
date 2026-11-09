# A page declares its data in its page, and reads it on one port per load

**2026-11-09, #5961.** Amends §1 and §7 of ADR 0120 with what building a page's read of its own
data decided.

- **The declaration is a data block of `index.html`**, `<script type="application/json"
  id="purlis">{ "api": 1, "data": ["data.json", "data/rows.json"] }</script>`: inert, so it adds
  no script, and part of the template, so the person's approval covers it (§4). `data` names
  data files only (`data.json`, or under `data/`), as written, every segment plain and no `%`.
  A block that names anything else, or an `api` this build never shipped, declares nothing, and
  the page reads nothing. `crates/purlis-core/src/artifact/data.rs`.
- **Reading one's own data is not a catalogue query.** A query is a read-only MCP tool (§7), and
  no chat has an artifact folder to name. The read is answered in the catalogue's envelope and
  error kinds all the same: an undeclared name is `not-approved`, a link at or on the way to the
  file `refused`, a file over 4 MiB `too-large`, one that is not JSON `failed`. **A declared
  file that is not there answers `data: null`**, the six kinds having none for it; the bar says
  "Not generated on this machine" when none of the declared files is there.
- **The port is purlis's own bridge**, since the page's webview holds no grant and reaches no
  network (§3). purlis's script in every artifact page makes a `MessageChannel` per load and
  hands the page `purlis.port`. The load names itself by a random word: it opens its port on
  its own origin's `/.purlis/port/<word>/open`, and each ask is an image of
  `/.purlis/port/<word>/read/<id>?<name>`, which the CSP admits and nothing else does. purlis
  attributes both by the webview's label, opens one port per webview (a newer load's replaces
  it), revokes it when the page starts loading again, and hands an answer back by evaluating it
  into the page only while the load that asked is still the open one.
- **Each read is gated when it is asked**: the pin read again and still the one served, and this
  machine's approval read again and still running the page's script; otherwise `not-approved`.
  The file is read from disk at the ask, not from the bytes served, so a regenerated file is
  read new.
