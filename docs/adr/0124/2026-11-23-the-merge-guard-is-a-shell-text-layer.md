# Amendment, 2026-11-23: the merge guard is a shell-text layer, the route is the boundary

Amends [ADR 0124](../0124-who-judges-a-chat-and-how-much.md) §7 (the merge hold).

The merge guard reads the text of a command line: `mergehold::held` and the release floor's merge
half (`floorguard::merge`) say what a `gh` or `glab` call *looks like it does*. They exist for an
early refusal a person can read, before the call is made, and they fail closed where the text
cannot be read: a `gh api` word the shell fills in is held unless it is plainly one word that
cannot begin a flag (#7416, #7423). They are not the boundary, and cannot be complete: a shell
builds text in more ways than a reader lists.

## The boundary

The boundary is the forge route of ADR 0128, which sees the request the chat actually sends (the
method, the path, the GraphQL document) with no shell between, and holds a merge, an auto-merge
setting or a ref move there whatever spelling got it that far. Three conditions, each of which
the text layer does not depend on:

- **The route refuses merges only under `guards.merge = enforce`.** Under `log` or `off` it
  forwards them, by design (#7372 made the route honour `guards.merge`).
- **It is the boundary only where the chat's forge traffic goes through it:** a build with
  forge-wiring, a proved sentinel and a recorded repo. Elsewhere the text layer is the only
  guard, and what it cannot read it holds.
- **A bypass of the text layer is a gap in the early warning**, to be closed as one when it is
  cheap and common. Under `enforce`, with the route in place, it is not a merge that lands.

## What round 3 of #7423 closed

The cheap and common shapes, and the speed: a literal start deleted by a line continuation, a
quoted operator word taken for a redirection, a line the lexer rejects, a `'` inside double
quotes, a redirection with a descriptor number before the call (`gh 2>/dev/null api …`,
`gh pr 2>/dev/null merge 5`), zsh's `-` after `noglob`, `nocorrect` or an assignment and when
repeated, and a flag before the noun that may take the noun as its value (`gh -b x pr merge 5`).
A line of tens of thousands of `$N` references reads in linear time.

## Known limits of the text layer

Not closed, and held at the boundary by the route under `guards.merge = enforce`:

- `bash <<< "…"`, a here-string into a shell;
- `echo "…" | sh` and `printf … | sh`, a script piped into a shell;
- `xargs -I{} sh -c '…{}…'`, words handed to a shell string;
- a heredoc with an unquoted delimiter into a shell;
- `trap '…' EXIT`;
- nested escaped backticks;
- a program or alias spelled `GH`, `g\h`, or made with `alias -g`.
