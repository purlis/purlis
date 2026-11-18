### Fixed

- A Claude Code chat on a plane with many workspaces and branch folders keeps auto mode on. Its permission rules had grown past what auto mode's check takes, and auto mode then refused unasked every action it would have checked. Each rule a chat is held by is now written once, with nothing less held, and every branch folder's git stays held even in a folder granted to the chat while it runs. A chat whose permission rules still weigh more says so on its pane, with what to do (#6961).
