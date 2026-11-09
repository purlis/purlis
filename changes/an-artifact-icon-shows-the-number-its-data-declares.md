### Added

- **An artifact's icon shows the number its data declares.** A whole number under `badge` in an
  artifact's `data.json` (open issues, say) is drawn on its icon in the activity bar, and changes
  when the data is regenerated. Past 99 it reads "99+". When the data is stale, the number dims
  with the icon. A zero, or data that declares no number, draws no badge.
