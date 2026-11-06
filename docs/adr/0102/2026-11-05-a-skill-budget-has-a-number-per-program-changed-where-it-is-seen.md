# A skill budget has a number per program, changed where it is seen

**Amends [ADR 0102](../0102-a-skill-router-decides-what-a-chat-sees-and-nothing-it-hides-is-out-of-reach.md), 2026-11-05**, ticket #5353. It adds to the [amendment of 2026-11-02](2026-11-02-a-chat-s-skill-budget-is-its-harness-s-and-the-setting-overrides-it.md), whose `settings.skills.budget` set one override for every program.

**Who decided what.** The operator ruled (2026-11-05): Change… on each row of the Routing page's budget table, with the numbers on the Skills panel and chat chip linking to it; an optional override per program, at project and workspace level, with no persona or personal level; the cost as it is typed; In force saying where each number comes from, with Reset to default on each; new chats get it, open chats keep theirs and get the existing restart offer once; smallest 1, empty means default, no maximum, past a limit it warns and never refuses, and below the pins it warns they are still listed. The means below are the steward's, and the operator is to review them.

## The file

```toml
[skills.budget]              # every program, as before
skills = 80
[skills.budget.claude]       # this program, over that
skills = 100
```

A program's table is named by the word a profile's `kind` is written with, and it holds what the budget holds. `settings.skills.budget` in `workspace.json` takes the same shape.

**Each number is read narrowest first:** the workspace's for this program, the workspace's for every program, the project's for this program, the project's for every program, then the adapter's. The level comes before the program, so a workspace's number always beats a project's.

**"One number per program" keeps both of the budget's numbers.** A budget is a number of skills and a number of characters, and a chat is over it past either. A program with only a skill count could be raised to 100 and still hide everything for being over its characters, so each program has both. The window's rows and dialog say them apart.

## Writing it

Settings keeps the numbers: the Routing page's and Settings' rows both write through `budget_setting::write`, to the project's `charter.toml` or the workspace's `workspace.json`.

- **The window never writes a number for every program.** Those already written go on working, and a program's own number stands over them. Where a reset or an empty box takes one program off such a number, the number becomes each other program's own, so nothing moves for them.
- **Reset to default takes out what this level writes.** In a workspace the project's number then stands, if it has one: that is the default a workspace has. There is no way to say "the program's own, whatever the project says", since empty is how to say "the number below".
- **A persona's scope in the studio writes the project's**, as there is no persona level. A chat's link goes to its workspace's Routing, whoever it runs as.

## Where the numbers link from

The Skills view's budget line links to its chat's workspace's Routing. **The chip on a chat's pane is, since #5204, a warning drawn only while the chat has a need**, so its link is there only then: `of 60` beside it, left out of a narrow pane's ⚠. A chat with nothing wrong shows no chip, and its Skills view holds the link. The count stays the warning's way to the Skills view.

## The cost

`budget_setting::preview` routes the set of a chat at the root of the level, by the built-in router, at today's numbers and at the numbers typed, as the Routing page's preview does. It writes nothing.

- **Tokens** are the difference in what the two listings spend, at three characters a token: the 91 tokens per 273-character line measured for the 2026-11-02 record. The window always says "about".
- **An extension's router is not asked**, since it is only ever asked at a chat's start. The dialog says that what it shows is what the built-in router would do.
- **Past the budget the router hides all it may**, so raising a number can show nothing at all while the other number is still over. The preview shows that as it is.

## What a chat gets

Every place that decides by a budget already asks `budget_in`, so a program's number reaches its own chats' routing and Claude Code's handed limit. A change to `charter.toml` or `workspace.json` already has each open chat's stamp read again, and the stamp is of the routed set, so **the restart offer comes only to a chat whose list the change would move**. Raising a number that hides nothing either way offers nothing.

**An open chat keeps what it was handed, and its Skills view and pane say the numbers now**, as the 2026-11-02 record says of Use skill…: that view works a chat's set out as it is read, and it is what the stamp compares. So after a change the Skills view can read `of 100` for a chat handed 60, with the restart offer beside it.
