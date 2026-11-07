// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { StrictMode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import App from "./App";
import { onAMac } from "./tabKeys";

/**
 * **A project of 1,000 workspaces** (#5501, the scale spec #5486, story 19): the title bar's
 * workspace popover and the command palette draw only the rows in sight, out of every one they
 * hold, and stay whole to the keyboard and to a screen reader: the arrows reach every row, typing
 * narrows to the one named, the row Enter would go to is always drawn for the field's
 * `aria-activedescendant`, and each row says where it stands in the whole (`aria-posinset` of
 * `aria-setsize`). How fast is the nightly scale run's to say, never jsdom's (ADR 0086).
 */

vi.mock("./SessionPane", () => ({
  SessionPane: ({ session }: { session: number }) => (
    <div data-testid="pane">session {session}</div>
  ),
}));

afterEach(() => {
  cleanup();
  clearMocks();
});

const PLANE = "/home/dev/big";
/** The scale spec's target: 1,000 workspaces, `ws-0000` to `ws-0999`. */
const NAMES = Array.from({ length: 1_000 }, (_, at) => `ws-${String(at).padStart(4, "0")}`);
/** Far more than a list in sight, far fewer than a list drawn whole. */
const DRAWN_AT_MOST = 100;

function chat(session: number, workspace: string) {
  return {
    session,
    name: `${workspace}.${session}`,
    cwd: `${PLANE}/workspaces/${workspace}`,
    harness: null,
    in_front: true,
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

/** One project of 1,000 workspaces, a chat in the first, none pinned. */
function core() {
  mockIPC((cmd, args) => {
    const given = (args ?? {}) as Record<string, unknown>;
    if (cmd === "plane_at_launch") return { plane: null, from: null, why: null };
    if (cmd === "planes_to_restore")
      return { windows: [{ planes: [PLANE], active: 0 }], dropped: [] };
    if (cmd === "open_plane") return { plane: given.path, ask: null };
    if (cmd === "plane_sidebar")
      return {
        root: PLANE,
        workspaces: NAMES.map((name, at) => ({
          name,
          path: `${PLANE}/workspaces/${name}`,
          vision: "",
          open_todos: at % 3,
          chats: at === 0 ? [chat(1, name)] : [],
          colour: null,
          live: true,
        })),
        personas: [],
        persona: null,
        unfiled: [],
      };
    if (cmd === "opened_chats") return [chat(1, NAMES[0])];
    if (cmd === "plane_pins")
      return { project: false, workspaces: [], missing: [], order: [], certain: true };
    if (cmd === "chat_states") return [];
    if (cmd === "chats_that_would_not_start") return [];
    if (cmd === "running_sessions") return [];
    if (cmd === "recent_planes") return { planes: [], dropped: [], forgetful: null };
    if (cmd === "extensions_on") return [];
    return null;
  });
}

const opened = () =>
  render(
    <StrictMode>
      <App />
    </StrictMode>,
  );

/** The title bar's workspace popover's key: ⌥⌘P on a Mac, Ctrl+Alt+Shift+P elsewhere. */
const switcherKey = () =>
  fireEvent.keyDown(
    window,
    onAMac()
      ? { key: "π", code: "KeyP", metaKey: true, altKey: true }
      : { key: "P", code: "KeyP", ctrlKey: true, altKey: true, shiftKey: true },
  );

/** The option the combobox `field` says Enter would run: drawn, chosen, and named. */
function aimedOf(field: HTMLElement): HTMLElement {
  const id = field.getAttribute("aria-activedescendant");
  expect(id).not.toBeNull();
  const option = document.getElementById(id ?? "");
  expect(option).not.toBeNull();
  expect(option).toHaveAttribute("role", "option");
  expect(option).toHaveAttribute("aria-selected", "true");
  return option as HTMLElement;
}

async function popover() {
  core();
  opened();
  await screen.findByTestId("title-bar");
  await waitFor(() => expect(screen.getByTestId("title-project")).toHaveTextContent("big"));
  switcherKey();
  const field = await screen.findByRole("combobox", { name: "Go to a workspace in big…" });
  const list = screen.getByRole("listbox", { name: "Workspaces in big" });
  return { field, list };
}

describe("the title bar's workspace popover over 1,000 workspaces", () => {
  it("draws only the rows in sight, each saying its place among all 1,002", async () => {
    const { list } = await popover();
    const drawn = within(list).getAllByRole("option");
    expect(drawn.length).toBeGreaterThan(0);
    expect(drawn.length).toBeLessThan(DRAWN_AT_MOST);
    // The root, 1,000 workspaces and New workspace…, said to a reader as one whole.
    for (const row of drawn) expect(row).toHaveAttribute("aria-setsize", "1002");
    expect(drawn[0]).toHaveAccessibleName("Plane root");
    expect(drawn[0]).toHaveAttribute("aria-posinset", "1");
    // A pin toggle for each drawn workspace's row, and no more.
    const toggles = screen.getAllByRole("button", { name: /^Pin ws-\d{4} to the title bar$/ });
    expect(toggles.length).toBeLessThan(DRAWN_AT_MOST);
  });

  it("walks every row with the arrows, the chosen one always drawn and named", async () => {
    const { field, list } = await popover();
    // The focused workspace is chosen as it opens: ws-0000, the root's next.
    expect(aimedOf(field)).toHaveAccessibleName("ws-0000");
    for (let step = 0; step < 150; step++) fireEvent.keyDown(field, { key: "ArrowDown" });
    const aimed = aimedOf(field);
    expect(aimed).toHaveAccessibleName("ws-0150");
    expect(aimed).toHaveAttribute("aria-posinset", "152");
    expect(within(list).getAllByRole("option").length).toBeLessThan(DRAWN_AT_MOST);
  });

  it("narrows to the workspace typed, and goes there on Enter", async () => {
    const { field } = await popover();
    await userEvent.keyboard("ws-0999");
    expect(aimedOf(field)).toHaveAccessibleName("ws-0999");
    await userEvent.keyboard("{Enter}");
    await waitFor(() => expect(screen.queryByRole("listbox")).toBeNull());
    await waitFor(() =>
      expect(
        screen.getByTestId("title-bar").querySelector('[aria-current="true"] [data-part="name"]'),
      ).toHaveTextContent("ws-0999"),
    );
  });
});

describe("the command palette over 1,000 workspaces", () => {
  async function palette() {
    core();
    opened();
    await waitFor(() => expect(screen.getByTestId("title-project")).toHaveTextContent("big"));
    await userEvent.keyboard("{F2}");
    const dialog = await screen.findByRole("dialog", { name: "Command palette" });
    const field = within(dialog).getByRole("combobox");
    await userEvent.keyboard("pin workspace");
    const list = await within(dialog).findByRole("listbox", { name: "Actions" });
    return { field, list };
  }

  it("draws only the rows in sight of a row for each workspace", async () => {
    const { list } = await palette();
    await waitFor(() =>
      expect(
        Number(within(list).getAllByRole("option")[0].getAttribute("aria-setsize")),
      ).toBeGreaterThanOrEqual(1_000),
    );
    expect(within(list).getAllByRole("option").length).toBeLessThan(DRAWN_AT_MOST);
  });

  it("reaches the last row with the arrows and finds a workspace's row by typing", async () => {
    const { field, list } = await palette();
    await waitFor(() =>
      expect(
        Number(within(list).getAllByRole("option")[0].getAttribute("aria-setsize")),
      ).toBeGreaterThanOrEqual(1_000),
    );
    const of = within(list).getAllByRole("option")[0].getAttribute("aria-setsize");
    // ↑ from the top goes round to the last row, out of sight until now.
    fireEvent.keyDown(field, { key: "ArrowUp" });
    await waitFor(() => expect(aimedOf(field)).toHaveAttribute("aria-posinset", of));
    await userEvent.keyboard(" ws-0742");
    await waitFor(() => expect(aimedOf(field)).toHaveTextContent(/Pin workspace ws-0742/));
  });
});
