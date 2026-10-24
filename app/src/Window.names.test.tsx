// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterAll, afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import App from "./App";
import type { OpenChat } from "./bindings";
import { expectEveryControlNamed, paidDebts } from "./a11y.testkit";
import { forgetThisLaunch } from "./regions";
import { findStripNamed, goToWorkspace } from "./test-strips";

/**
 * **Every control the window draws has a name** (DS-6, #629): the window itself — its strips,
 * its sidebars, the editor, the Panel and the handles between them — with each of its views
 * shown in turn; the command palette; and each dialog the palette opens. Each is reached the
 * way a person reaches it, through the palette's own rows, and held to `expectEveryControlNamed`
 * with the whole document on screen. Settings and the editor's tabs are held in their own
 * files (`settings/Names.test.tsx`, `editor/Names.test.tsx`); the rule's own cases are
 * `a11y.names.test.tsx`.
 *
 * The core answers the reads the window makes on launch and leaves every other read empty, so
 * a view draws its empty state. A view's rows are held where its own data is mocked.
 */

// jsdom cannot draw a terminal: what is held here is the window around it.
vi.mock("./SessionPane", () => ({
  SessionPane: ({ session }: { session: number }) => <div>session {session}</div>,
}));

/**
 * Nameless controls drawn by files this check cannot mend yet, each as the check writes it. It
 * only shrinks: a debt nothing here draws any more fails the last test.
 */
const DEBT: readonly string[] = [];

const found: string[] = [];

const PLANE = "/home/dev/plane";

function workspace(name: string, chats: OpenChat[] = []) {
  return {
    name,
    path: `${PLANE}/workspaces/${name}`,
    vision: "",
    todos: [],
    chats,
    colour: null,
    live: false,
  };
}

function chat(session: number): OpenChat {
  return {
    session,
    name: `alpha.${session}`,
    cwd: `${PLANE}/workspaces/alpha`,
    harness: null,
    in_front: session === 1,
    resumed: null,
    fresh: null,
    profile: null,
    persona: null,
    unreported: null,
    card: null,
    guessed: null,
    pinned: false,
    label: null,
    from: null,
  };
}

/** What the chat picker draws: one profile and one persona. */
const START_OPTIONS = {
  profiles: [
    {
      name: "claude",
      kind: "claude",
      shown: "claude",
      source: "built-in",
      is_default: true,
      approval: null,
    },
  ],
  refused: [],
  personas: ["steward"],
  persona: "steward",
  persona_profiles: {},
  ignore_fix: null,
  declares_none: true,
};

/** A project with two pinned workspaces, alpha in front with two chats open in it. */
function core() {
  mockIPC(
    (cmd) => {
      switch (cmd) {
        case "plane_at_launch":
          return { plane: PLANE, from: PLANE, why: null };
        case "opened_chats":
          return [chat(1), chat(2)];
        case "chats_that_would_not_start":
        case "running_sessions":
        case "chat_states":
          return [];
        case "plane_pins":
          return { project: false, workspaces: ["alpha", "beta"], missing: [] };
        case "plane_sidebar":
          return {
            root: PLANE,
            personas: [],
            persona: null,
            unfiled: [],
            workspaces: [workspace("alpha", [chat(1), chat(2)]), workspace("beta")],
          };
        case "start_options":
          return START_OPTIONS;
        // The split's second chat (#629).
        case "start_chat":
          return { session: 3 };
        default:
          return null;
      }
    },
    { shouldMockEvents: true },
  );
}

beforeEach(() => {
  forgetThisLaunch();
  core();
});
afterEach(() => {
  cleanup();
  clearMocks();
});

/** The window drawn, with alpha in front. */
async function theWindow() {
  render(<App />);
  await findStripNamed("Projects");
  await goToWorkspace("alpha");
  await within(await findStripNamed("Tabs")).findByRole("tab", { name: /alpha\.1/ });
}

/** Opens the palette on ⌘K, and answers it. */
async function thePalette() {
  await userEvent.keyboard("{Meta>}k{/Meta}");
  return screen.findByRole("dialog", { name: "Command palette" });
}

/** Opens the palette and runs its row whose title is `title`, as a click on it does. */
async function run(title: string) {
  const palette = await thePalette();
  const row = within(palette)
    .getAllByRole("option")
    .find((one) => one.textContent?.startsWith(title));
  if (!row) throw new Error(`the palette has no row "${title}"`);
  await userEvent.click(row);
}

/** Every modal surface on screen but the palette. */
const dialogs = () =>
  [...document.querySelectorAll('[role="dialog"], [role="alertdialog"]')].filter(
    (one) => one.getAttribute("aria-label") !== "Command palette",
  );

/** The views the palette shows, each on its own row. */
const VIEWS = [
  "Chats",
  "Explorer",
  "Changes",
  "Repos",
  "Inbox",
  "Todos",
  "Memory",
  "Personas",
  "Sessions",
  "Vaults",
];

/** The palette's rows that open a dialog of the window's own. */
const OPENS_A_DIALOG = [
  "New tab",
  "Extensions…",
  "New workspace…",
  "New project…",
  "Rename workspace alpha…",
  "Link to work item…",
  "Make alpha live…",
];

describe("every control the window draws has a name", () => {
  it("in the window, with each of its views shown", async () => {
    await theWindow();
    found.push(...expectEveryControlNamed(document.body, DEBT, "the window"));

    for (const view of VIEWS) {
      await run(`Show the ${view} view`);
      await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
      found.push(...expectEveryControlNamed(document.body, DEBT, `the ${view} view`));
    }
  }, 60_000);

  it("with a chat split in two, the handle between the panes among them (#629)", async () => {
    await theWindow();

    await run("Split right");
    await userEvent.click(await screen.findByRole("button", { name: "Start" }));
    await waitFor(() => expect(dialogs()).toHaveLength(0));

    expect(screen.getByRole("separator", { name: "Resize the split" })).toBeInTheDocument();
    found.push(...expectEveryControlNamed(document.body, DEBT, "the split"));
  });

  it("in the command palette", async () => {
    await theWindow();
    await thePalette();

    found.push(...expectEveryControlNamed(document.body, DEBT, "the palette"));
  });

  it("in each dialog the palette opens", async () => {
    await theWindow();

    for (const row of OPENS_A_DIALOG) {
      await run(row);
      await waitFor(() => expect(dialogs()).toHaveLength(1));
      found.push(...expectEveryControlNamed(document.body, DEBT, row));
      await userEvent.keyboard("{Escape}");
      await waitFor(() => expect(dialogs()).toHaveLength(0));
    }
  }, 60_000);
});

afterAll(() => {
  // Run last, over what every surface above drew: a paid debt comes off the list.
  expect(paidDebts(DEBT, found), "debts nothing draws any more").toEqual([]);
});
