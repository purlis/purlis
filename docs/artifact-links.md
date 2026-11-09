# Links in an artifact's page

A page pinned to a workspace (ADR 0122) may link out of itself. purlis follows a link only when a
person clicks it and it is written in the page's template as served; a link the page's script
made is refused. Two kinds are followed; every other is refused with a sentence the window shows.

## `https:` links

They open in the system browser, after purlis asks, naming the host and showing the whole
address. _Always open_ stops it asking for that host on this machine
(`<config>/artifact-link-hosts.json`, `docs/plane-format.md`). A link carrying a name or a
password is refused (#5956).

## `purlis://` links

They go to a place in purlis, without asking, and never do anything there (#5967; ADR 0122 §5 as
amended in [`adr/0122/2026-11-09-a-purlis-link-names-a-place-of-a-closed-list-and-only-navigates.md`](adr/0122/2026-11-09-a-purlis-link-names-a-place-of-a-closed-list-and-only-navigates.md)).
There are four, each of one shape:

| Link                               | Goes to                                                     |
| ---------------------------------- | ----------------------------------------------------------- |
| `purlis://workspace/<workspace>`   | the workspace, focused                                      |
| `purlis://todo/<workspace>/<todo>` | the open todo's view; `<todo>` is its file's name, no `.md` |
| `purlis://chat/<chat id>`          | the chat, shown; `<chat id>` is its ULID                    |
| `purlis://issue/<tracker key>`     | the chat open here that works on that work item             |

```html
<a href="purlis://workspace/smart-ide">The workspace</a>
<a href="purlis://todo/smart-ide/fix-login">Fix the login</a>
<a href="purlis://chat/01M70V414APWHKNRPG7YHK6J20">The chat that wrote this</a>
<a href="purlis://issue/github:github.com/purlis/purlis%235967">#5967</a>
```

- **A tracker key's `#` is written `%23`**, or the browser reads the rest as a fragment. A todo's
  key (`todo:<workspace>/<todo>`) goes to the todo.
- **The place must be there now**: a workspace of the artifact's project, an open todo of it, a
  chat open in purlis, a chat open in purlis linked to the issue (its work link, ADR 0088:
  _Link to work item…_ on the chat). Where it is not, purlis says so and goes nowhere. To show
  an issue's own page, link to it over `https:`.
- **Nothing else is a purlis link.** Another first word (`purlis://run/…`), anything after a
  place (`purlis://todo/smart-ide/fix-login/done`), a query (`?…`), a fragment (`#…`), a name or
  a port is refused. A page that wants the person to act offers a catalogue row instead, which
  purlis draws in its own bar (ADR 0122 §8).
