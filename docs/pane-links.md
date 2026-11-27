# Links in a pane

Text a chat, a shell or a one-job program prints in its pane can be a link: a web address, a
file's path, a chat's number, or words the program marked as a link itself. Nothing is drawn
differently while it rests. Under the pointer a link is underlined, and a tooltip says where it
goes and what opening it does. The pane only finds the text. The core decides what it is, and
reads it again when it is opened, so a pane never chooses what opens (spec #6208).

## Opening one

- **⌘-click** (Ctrl+click off a Mac) opens it. A plain click selects text, as in any terminal.
- **Right-click** on a link offers _Open link_ and _Copy link_. _Copy link_ copies where the
  link goes: a file's whole path, or the address a program marked under its words.
- **⇧⌘L** (Ctrl+Shift+L off a Mac) in a focused pane lists the links on its screen, newest
  first. Return opens the row it is on, and ⌘C (Ctrl+C) copies it. _Links in this pane…_ on the
  pane's ⋯ menu and in the palette opens the same list.

## What links

| Text                                                                                     | Opens                                                    |
| ---------------------------------------------------------------------------------------- | -------------------------------------------------------- |
| An `http:` or `https:` address                                                           | in your browser                                          |
| A file's path (`src/main.rs:42:7`, `~/notes.md`, `/etc/hosts:3`) or a `file:///` address | in an editor tab, at its line and column. Nothing is run |
| `chat 641`                                                                               | that chat in its tab, or an ended chat's card            |
| Words a program marked as a link (OSC 8)                                                 | where the program said, read by that address's own rules |

- **A path links only when the file is there**, read from the folder the chat started in (or
  `~` for your home folder). Path-shaped words that name no file are no link, and their hover
  says nothing.
- **A file in a branch links only when the branch offers it**: what git tracks there, or does
  not ignore, which is all the editor opens. A file git ignores says so on its hover. Where
  purlis could not read the branch's list of files, the file still links, and its hover says
  the editor checks it as it opens.
- **A link too long for one row is one link over all its rows**, whether the terminal wrapped
  it or the program broke the line itself at the pane's edge: hovering any row draws all of it,
  and ⌘-click on any row opens the whole. A break the program wrote is read across only on the
  normal screen (never in an editor, pager or multiplexer on the alternate screen), where the
  row fills the pane to its edge with cells that draw and the next row goes straight on at the indent of the
  line's first row's words (so a paragraph a harness indents two blanks on every row joins,
  and so does a reply or list item whose first row opens with a marker such as `⏺ `, `• `,
  `- ` or a number (`1. `, `12) `) and whose next rows are indented to its words), not with a
  frame's border, a link of its own or an item of its own (a nested list's `- ` or `1. `, a
  tool result's `⎿ `). Where the joined words name no file, each row's piece links on its own
  (ADR 0132).
- **A chat's number is looked up in the pane's own project.** A number no chat there had is no
  link. A title never links, and `#641` alone is an issue's number, not a chat.

## What asks first

- **A web address read across rows the program broke at the pane's edge.** purlis joins the
  rows only where they plainly go on, but the join is still a guess about what the program
  meant, so the ask shows the whole address as it was read. One on a single row, or one the
  terminal wrapped, opens without asking (ADR 0132).
- **A file outside the chat's workspace** (the project, for a chat at its root). The ask names
  the file, the workspace and the chat whose output named it. Opening one only shows it, so the
  ask starts on _Open file_.
- **Words a program marked whose text names somewhere else.** A program can print
  `github.com/login` and mark it as a link to another site. purlis reads the words, and when a
  word names an address, a host, an IP address or a path that is not where the link goes, or
  its letters may not be what they look like (a Cyrillic `о` in `gооgle.com`), it asks, showing
  the words and where the link really goes, with the real host in bold. The ask starts on
  _Don't open_, and Return never opens it. Words that name nothing (`Click here`, `v1.2.3`,
  `#6384`) open without asking, as do words that name the link's own address
  (`github.com/purlis` over `https://github.com/purlis/purlis`).

An ask is answered once: purlis issues it a one-use token for that pane and that target, and
only that token opens it.

## What is refused

The hover says why, the link is not underlined, and ⌘-click does nothing. A link a program
marked is drawn as a link whatever it is, so ⌘-click on a refused one shows why.

- **Any scheme but `http:`, `https:` and `file:`**: `ftp:`, `javascript:`, `vscode:` and every
  other. purlis opens nothing else from a pane.
- **An address carrying a name or a password** (`https://me@example.com/`).
- **A `file:` address naming another machine** (`file://server/share/x`).
- **A marked link's address longer than 2,048 bytes**, refused unread, and one the program left
  empty.
- **A file in the folders the system guards** (Documents, Desktop, Downloads and the like) or on
  a removable, network or automounted volume, unless it is in the chat's own project or folder:
  a look there could stall or make the system ask you.
