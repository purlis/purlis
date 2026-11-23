# Amendment, 2026-11-23: the merge guard is a shell-text layer, the route is the boundary

Amends [ADR 0124](../0124-who-judges-a-chat-and-how-much.md) §7 (the merge hold).

The merge guard reads the text of a command line: `mergehold::held` and the release floor's merge
half (`floorguard::merge`) say what a `gh` or `glab` call *looks like it does*. They exist for an
early refusal a person can read, before the call is made, and they fail closed where the text
cannot be read: a `gh api` word the shell fills in is held unless it is plainly one word that
cannot begin a flag (#7416, #7423). They are not the boundary, and cannot be: shell text can be
built in more ways than a reader lists (#7372). The boundary is the forge route of ADR 0128, which
sees the request the chat actually sends (the method, the path, the GraphQL document) with no
shell between, and holds a merge, an auto-merge setting or a ref move there whatever spelling got
it that far. A shell-text bypass found later is a gap in the early warning, to be closed as one,
and is not a merge that lands.
