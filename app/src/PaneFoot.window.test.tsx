// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { StrictMode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import {
  act,
  cleanup,
  fireEvent,
  render as renderBare,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import App from "./App";
import { aTabForEachChat } from "./laidOut.testkit";

/**
 * **What a chat's pane foot says and what a press on it does, against the whole window** (#5204,
 * spec #5191 stories 33 to 41): `↳ from`, the skills warning and the sandbox moved here from the
 * head, and every part that can be pressed is a shortcut to a row of the chat's list.
 */

vi.mock("./SessionPane", () => ({
  SessionPane: ({ session }: { session: number }) => (
    <div data-testid="pane" tabIndex={0}>
      session {session}
    </div>
  ),
}));

const render = (ui: React.ReactElement) => renderBare(<StrictMode>{ui}</StrictMode>);

afterEach(() => {
  usage = USAGE;
  cleanup();
  clearMocks();
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

const PLANE = "/home/dev/plane";
const ALPHA = `${PLANE}/workspaces/alpha`;

const USAGE = {
  context: { value: 62, tone: "ok" },
  cache: { value: 91, tone: "ok" },
  rebuilds: null,
  turns: [],
  cold: null,
};
let usage: unknown = USAGE;

const PARENT = {
  session: 3,
  name: "steward 3",
  cwd: `${ALPHA}/repo`,
  harness: "claude",
  in_front: false,
  resumed: null,
  fresh: null,
  profile: "claude",
  persona: null,
  unreported: null,
  guessed: null,
  pinned: false,
  label: "steward 3",
  from: null,
  sandbox: "floor",
  sandbox_why: null,
  browser_sites: null,
  card: { name: "claude", title: "Claude Code", label: "What Claude Code can do here", lines: [] },
};

/** A chat handed off from chat 3. */
const HANDED = {
  ...PARENT,
  session: 4,
  name: "claude 4",
  label: "work",
  in_front: true,
  from: {
    chat: 3,
    name: "steward 3",
    workspace: "alpha",
    task: false,
    tab: true,
    reported: false,
    unreported: false,
  },
};

function core(
  open: readonly (typeof PARENT | typeof HANDED)[],
  stripOf: (session: number) => string = () => "alpha",
) {
  mockIPC(
    (cmd) => {
      if (cmd === "chat_usage") return usage;
      if (cmd === "plane_at_launch") return { plane: PLANE, from: PLANE, why: null };
      if (cmd === "opened_chats") return open;
      if (cmd === "reopened_layout") return aTabForEachChat([...open], stripOf);
      if (cmd === "plane_sidebar")
        return {
          root: PLANE,
          personas: [],
          persona: null,
          unfiled: [],
          workspaces: [
            { name: "alpha", path: ALPHA, vision: "", todos: [], chats: open },
            { name: "beta", path: `${PLANE}/workspaces/beta`, vision: "", todos: [], chats: [] },
          ],
        };
      if (cmd === "chat_states") return [];
      if (cmd === "chats_that_would_not_start") return [];
      if (cmd === "running_sessions") return [];
      if (cmd === "owed_restarts") return [];
      if (cmd === "asks_waiting") return { plane: PLANE, asks: [] };
      if (cmd === "mcp_failures") return [];
      if (cmd === "smart_closing") return [];
      if (cmd === "open_view") return { kind: "gone", why: "Not read in this test." };
      return null;
    },
    { shouldMockEvents: true },
  );
}

async function aPane(
  open: readonly (typeof PARENT | typeof HANDED)[] = [PARENT, HANDED],
  stripOf?: (session: number) => string,
) {
  core(open, stripOf);
  render(<App />);
  await screen.findAllByTestId("pane");
  await act(async () => {
    await new Promise((resolve) => setTimeout(resolve, 0));
  });
}

const frames = () => [...document.querySelectorAll<HTMLElement>(".pane-frame")];
/** The pane in front, by what it shows: the handed-off chat's, unless it is alone. */
const frameOf = (session: number) =>
  frames().find((one) => one.textContent?.includes(`session ${session}`)) as HTMLElement;
const foot = (session = 4) =>
  frameOf(session).querySelector('[data-scope="pane-foot"][data-part="root"]') as HTMLElement;
/** The tab in front on the strip of tabs. */
const tabInFront = () =>
  document.querySelector('[data-strip="Tabs"] [role="tab"][aria-selected="true"]');
const head = (session = 4) => frameOf(session).querySelector(".pane-head") as HTMLElement;

describe("a chat's foot holds the rest of its facts (#5204)", () => {
  it("says harness · workspace, ↳ from and the sandbox, and the head says none of them", async () => {
    await aPane();

    await waitFor(() => expect(foot().querySelector(".pane-foot-from")).not.toBeNull());
    expect(foot().querySelector(".pane-foot-from")).toHaveTextContent("↳ from steward 3 · alpha");
    expect(foot().querySelector(".pane-foot-sandbox [data-sandbox]")).not.toBeNull();
    expect(head().textContent).not.toContain("↳ from");
    expect(head().querySelector("[data-sandbox]")).toBeNull();
  });

  it("is a group a screen reader hears naming the chat", async () => {
    await aPane();

    await waitFor(() => expect(foot().querySelector(".pane-where")).not.toBeNull());
    expect(foot()).toHaveAttribute("role", "group");
    expect(foot().getAttribute("aria-label")).toMatch(/work: where it runs and its context$/);
  });

  it("asks for nothing: every button on it opens something, and it holds no Notice", async () => {
    await aPane();

    await waitFor(() => expect(foot().querySelector(".pane-foot-from")).not.toBeNull());
    expect(foot().querySelector('[data-scope="notice"]')).toBeNull();
    expect(frameOf(4).querySelector(".pane-top")?.contains(foot())).toBe(false);
  });
});

describe("a press on a foot's part runs its row (#5204, stories 33 to 36)", () => {
  it("goes to the chat that started it, on ↳ from", async () => {
    await aPane();

    const from = await waitFor(() =>
      within(foot()).getByRole("button", { name: /↳ from steward 3/ }),
    );
    fireEvent.click(from);
    await waitFor(() => expect(tabInFront()).toHaveTextContent("steward 3"));
  });

  it("opens the harness's card, on the harness", async () => {
    await aPane();

    const harness = await waitFor(() =>
      within(foot()).getByRole("button", { name: "What Claude Code can do here" }),
    );
    fireEvent.click(harness);
    await waitFor(() => expect(tabInFront()).toHaveTextContent("What Claude Code can do here"));
  });

  it("opens the chat's Activity, on ctx", async () => {
    await aPane();

    const ctx = await waitFor(() => within(foot()).getByRole("button", { name: /^ctx 62%/ }));
    fireEvent.click(ctx);
    await waitFor(() => expect(tabInFront()).toHaveTextContent(/Activity/));
  });

  it("says its workspace in words where that workspace is the one focused: no press would go anywhere", async () => {
    await aPane();

    await waitFor(() => expect(foot().querySelector(".pane-where")).toHaveTextContent("alpha"));
    expect(within(foot()).queryByRole("button", { name: /^Focus workspace/ })).toBeNull();
  });

  it("focuses the workspace a chat is filed in, from a tab of another", async () => {
    await aPane([HANDED], () => "beta");

    const workspace = await waitFor(() =>
      within(foot()).getByRole("button", { name: /^Focus workspace alpha/ }),
    );
    expect(workspace).toHaveTextContent("alpha");
    fireEvent.click(workspace);
    // The tab was on beta's strip, which is no longer the one in front.
    await waitFor(() => expect(screen.queryByText("session 4")).toBeNull());
  });
});

describe("the foot's keyboard order (#5204, story 38)", () => {
  it("takes the foot's parts after the terminal, in the order they are drawn", async () => {
    await aPane();
    await waitFor(() => within(foot()).getByRole("button", { name: /^ctx 62%/ }));

    const user = userEvent.setup();
    screen
      .getAllByTestId("pane")
      .find((one) => one.textContent === "session 4")
      ?.focus();
    const heard: string[] = [];
    for (let at = 0; at < 3; at++) {
      await user.tab();
      heard.push(
        document.activeElement?.getAttribute("aria-label") ??
          document.activeElement?.textContent ??
          "",
      );
    }
    expect(heard[0]).toBe("What Claude Code can do here");
    expect(heard[1]).toMatch(/↳ from steward 3/);
    expect(heard[2]).toMatch(/^ctx 62%/);
  });
});
