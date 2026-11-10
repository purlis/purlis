### Added

- **An open artifact stays current and looks like purlis.** It loads again when its files or its
  data change, in every window showing it, and keeps its scroll position. The page is drawn in
  the purlis theme: its colours are handed to the page as `--purlis-` CSS variables, and light
  or dark follows purlis, not the operating system. Each artifact's icon says when its data was
  last updated and is dimmed once that is a day old; the date is the one `data.json` declares
  under `"updated"`, else the data's commit or write time (#5955).
