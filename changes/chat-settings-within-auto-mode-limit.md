### Fixed

- A Claude Code chat on a plane with many workspaces and branch folders keeps auto mode on. Its settings had grown near 1 MB, past what auto mode's check takes, and auto mode then refused unasked every action it would have checked. Each path a chat is held from is now written once, with no fewer paths held, and a large plane's settings fall from about 620 KB to under 256 KB. A chat whose settings still weigh more says so on its pane, with what to do (#6961).
