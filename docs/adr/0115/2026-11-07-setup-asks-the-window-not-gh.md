# Amendment, 2026-11-07: setup asks the window, not `gh`

Decision 6 had `purlis signing setup` add the key with `gh api` where `gh` could, and
`purlis signing check` ask GitHub through `gh`. Neither runs `gh` any more (#5625). The command
holds no forge sign-in (ADR 0070 ruling 5), so its reads go to the running window, which asks
GitHub with its own sign-in. From the command line the key is added on GitHub's page by hand.
The window's own setup adds it through the forge account (ADR 0116 §5, and its amendment of
the same day). The three steps and the signing itself are unchanged.
