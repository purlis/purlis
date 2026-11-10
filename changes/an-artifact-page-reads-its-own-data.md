### Added

- **An approved artifact reads the data files beside it.** A page declares the API version it
  was written for and the data files it reads in a `<script type="application/json"
  id="purlis">{ "api": 1, "data": ["data.json"] }</script>` block, which your approval covers.
  Its script then asks for them on `purlis.port`, a message port purlis hands each load of the
  page and takes back when it loads again, and hears `{ id, data | error: { kind }, at }`. Only
  a declared file of its own folder is read, never through a link and never over 4 MiB. When
  none of its declared files is on this machine, its tab's bar says "Not generated on this
  machine".
