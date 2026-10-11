### Fixed

- **A harness's card row follows a profile file edited by hand.** After `charter.local.toml`
  was changed outside the window, the Curate rows already said why they could not run and
  pointed at the harness's card ("See What opencode can do here."), but the card's own row
  (in the palette and under the Curate menu) came only once the project was brought to the
  front again. The window now reads the harnesses' cards again when it comes back into focus,
  as it does the curation rows (#1134).
