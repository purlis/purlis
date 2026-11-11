# PROTOTYPE: links in a pane's output, and a closed chat's card (#6211)

Throwaway. It lives on branch `prototype/pane-links` and never merges to main.

## The question

What do links in a pane look like, and what do they do (spec #6208)? That covers `chat 641`, a
`path:line`, an https address and an OSC 8 hyperlink, on hover, on ⌘-click, from the right-click
menu, and from the keyboard. What is the card a closed chat opens (#5820), and where does "#641"
go on a tab and a chat's head?

## Open it

`cd app && npm run storybook`, then **PROTOTYPE / Pane links**. Each story opens on one moment.
The `AtRest` story is the whole thing, live: hover a link, ⌘-click it, right-click it, press ⇧⌘L
in the pane, and turn on *Show what links* to see what is a link and what only looks like one.

The pane is a picture of a terminal on stand-in data, not xterm. The look is the question here.

## The map

`WAYFINDER.md`: every surface (S1–S10), every action and where it is reached from, the decisions
D1–D12, and the open questions Q1–Q3.

## Verdict

Not seen yet.
