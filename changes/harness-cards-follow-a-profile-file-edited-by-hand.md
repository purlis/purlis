### Fixed

- **The harness cards follow a declaration edited by hand.** The palette's *What … can do
  here* rows were read again only when the project came to the front or its settings changed
  in the window, so a harness declared in the project's files from an editor got no card row
  until then, while the curation rows already followed the file. The window now reads the cards
  again when it comes back into focus, as it does the curation rows (#1134).
