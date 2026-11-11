// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * PROTOTYPE, throwaway (#6211, branch prototype/pane-links). Never merges to main.
 *
 * The stand-in data of the pane links prototype: one chat's pane (chat #652, in smart-ide),
 * the output it holds, and what the core's resolver would answer for each candidate in it
 * (spec #6208, "A core link resolver"). Nothing here resolves anything: it is what the person
 * would see.
 */

/** The three kinds of target v1 links (#6208 decision 1). */
export type Kind = "chat" | "file" | "url";

/** What ⌘-click does with a link, as the core's open command would answer it (#6208). */
export type Opens =
  | { how: "focus"; where: string }
  | { how: "card" }
  | { how: "editor"; where: string }
  | { how: "browser" }
  | { how: "ask-outside" }
  | { how: "ask-osc8" };

/** One link on screen. */
export type Link = {
  id: string;
  kind: Kind;
  /** The words as the program printed them. */
  text: string;
  /** Where it resolves to, said in full: the tooltip's first line and the list's. */
  target: string;
  /** The tooltip's second line: where it is, and what ⌘-click does. */
  does: string;
  opens: Opens;
  /** A hyperlink the program marked itself (OSC 8), not one purlis found. */
  osc8?: boolean;
  /** What Copy link puts on the clipboard: the resolved target, never the text (D6). */
  copies: string;
};

/** Text that looks like a link and is not one, with why: the "Show what links" overlay's. */
export type NotALink = { text: string; why: string };

/** One run of a pane's line. */
export type Run = string | { link: string } | { not: NotALink } | { faint: string };

/** The pane's own chat: the chat this output is in. */
export const THIS_CHAT = { number: 652, title: "Grill #5820: chat links", workspace: "smart-ide" };

/** The workspace's folder, which a file outside asks before it opens. */
export const WORKSPACE_DIR = "~/IdeaProjects/charter/workspaces/smart-ide";

/** The modifier a link opens with: ⌘ on a Mac, Ctrl elsewhere (#6208 decision 5). */
export const MOD = "⌘";

export const LINKS: Record<string, Link> = {
  chat641: {
    id: "chat641",
    kind: "chat",
    text: "chat 641",
    target: "Chat #641 · Signed commits #5607",
    does: `Finished 11:08 · ${MOD}-click shows its card`,
    opens: { how: "card" },
    copies: "chat 641",
  },
  chat652: {
    id: "chat652",
    kind: "chat",
    text: "chat 652",
    target: "Chat #652 · Grill #5820: chat links",
    does: `This chat · ${MOD}-click does nothing more`,
    opens: { how: "focus", where: "This chat, in front already" },
    copies: "chat 652",
  },
  chat660: {
    id: "chat660",
    kind: "chat",
    text: "chat 660",
    target: "Chat #660 · Docs site: the setup page",
    does: `Open in workspace docs · ${MOD}-click shows it there`,
    opens: { how: "focus", where: "Showed chat #660 in docs, tab 3" },
    copies: "chat 660",
  },
  resolve: {
    id: "resolve",
    kind: "file",
    text: "crates/purlis-core/src/links/resolve.rs:42:7",
    target: "purlis/crates/purlis-core/src/links/resolve.rs, line 42, column 7",
    does: `In smart-ide · ${MOD}-click opens it in an editor tab`,
    opens: { how: "editor", where: "Opened resolve.rs at 42:7 in an editor tab" },
    copies: `${WORKSPACE_DIR}/purlis/crates/purlis-core/src/links/resolve.rs:42:7`,
  },
  test: {
    id: "test",
    kind: "file",
    text: "app/src/paneLinks.test.ts:18",
    target: "purlis/app/src/paneLinks.test.ts, line 18",
    does: `In smart-ide · ${MOD}-click opens it in an editor tab`,
    opens: { how: "editor", where: "Opened paneLinks.test.ts at line 18 in an editor tab" },
    copies: `${WORKSPACE_DIR}/purlis/app/src/paneLinks.test.ts:18`,
  },
  notes: {
    id: "notes",
    kind: "file",
    text: "~/IdeaProjects/charter/workspaces/smart-ide/workspace.md",
    target: "workspace.md",
    does: `In smart-ide · ${MOD}-click opens it in an editor tab`,
    opens: { how: "editor", where: "Opened workspace.md in an editor tab" },
    copies: `${WORKSPACE_DIR}/workspace.md`,
  },
  pr: {
    id: "pr",
    kind: "url",
    text: "https://github.com/purlis/purlis/pull/6148",
    target: "https://github.com/purlis/purlis/pull/6148",
    does: `${MOD}-click opens it in your browser`,
    opens: { how: "browser" },
    copies: "https://github.com/purlis/purlis/pull/6148",
  },
  setup: {
    id: "setup",
    kind: "url",
    text: "docs.purlis.dev/setup",
    target: "https://purlis-setup.example.net/s?id=7",
    does: `Its text says docs.purlis.dev · ${MOD}-click asks first`,
    opens: { how: "ask-osc8" },
    osc8: true,
    copies: "https://purlis-setup.example.net/s?id=7",
  },
  hosts: {
    id: "hosts",
    kind: "file",
    text: "/etc/hosts:3",
    target: "/etc/hosts, line 3",
    does: `Outside smart-ide · ${MOD}-click asks first`,
    opens: { how: "ask-outside" },
    copies: "/etc/hosts:3",
  },
};

/** The pane's sample output, row by row, top to bottom: a dispatch list, then a harness's turn. */
export const ROWS: Run[][] = [
  [{ faint: "> " }, "purlis dispatch list"],
  ["  ", { link: "chat641" }, " · Signed commits #5607 · finished 11:08 · report sent"],
  ["  ", { link: "chat652" }, " · Grill #5820: chat links · working (this chat)"],
  ["  ", { link: "chat660" }, " · Docs site: the setup page · working in docs"],
  [
    "  ",
    { not: { text: "chat 9999", why: "No chat 9999 in this project, so it is not a link." } },
    " · (a number from another project)",
  ],
  [""],
  ["● Read(", { link: "resolve" }, ")"],
  [{ faint: "  ⎿ Read 120 lines" }],
  ["● The resolver's tests are in ", { link: "test" }, ";"],
  ["  the workspace notes in ", { link: "notes" }, "."],
  [
    "  It follows ",
    { link: "pr" },
    " (issue ",
    { not: { text: "#5820", why: "#N is an issue's number, never a chat (v1 links no issues)." } },
    ").",
  ],
  ["  Setup guide: ", { link: "setup" }, { faint: "   ← an OSC 8 link to another address" }],
  [
    "  The container wrote ",
    { link: "hosts" },
    " and ",
    { not: { text: "src/old/gone.rs:10", why: "No such file, so it is not a link." } },
    ".",
  ],
  [
    "  Mirror: ",
    {
      not: {
        text: "ftp://mirror.example.org/pub",
        why: "Not http or https: every other scheme is refused, never a link.",
      },
    },
  ],
  [""],
  [{ faint: "> " }],
];

/** The links on screen, newest (lowest on screen) first: the Links list's order (#6208). */
export function linksNewestFirst(): Link[] {
  const seen: Link[] = [];
  for (const row of ROWS)
    for (const run of row) if (typeof run === "object" && "link" in run) seen.push(LINKS[run.link]);
  return seen.reverse();
}

/** How each kind is said in the Links list. */
export const KIND_SAID: Record<Kind, string> = { chat: "Chat", file: "File", url: "Web address" };

/** The closed chat's card (#5820): what the core keeps of chat #641 after it finished. */
export const CLOSED_CHAT = {
  number: 641,
  title: "Signed commits #5607",
  finished: "11:08",
  outcome: "done" as "done" | "blocked" | "failed" | "closed",
  startedBy: "steward · chat #612 (Grill #5607: signed commits)",
  ran: "smart-ide · Claude Code · 41 min",
  report:
    "Every commit the release job makes is signed now: the job reads the key from the vault and " +
    "git verifies it before the push. PR #5611 merged on green; #5607 closed. One leftover filed " +
    "as #5622 (the dev channel's tag is still unsigned).",
};
