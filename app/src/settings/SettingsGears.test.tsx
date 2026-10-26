// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { emit } from "@tauri-apps/api/event";
import App from "../App";
import { forgetThisLaunch } from "../regions";
import { GLOBAL } from "../windowprefs";
import {
  chipName,
  goToWorkspace,
  stripNamed,
  workspaceChips,
  workspaceInFront,
} from "../test-strips";

/**
 * **The quiet gears, and ⌘, at the focused level** (SE-23, #1173; V89g, V89i on #558).
 *
 * A gear on the project in front's tab and one on the focused workspace's tab, each the same
 * catalogue row its right-click menu and the palette run, and nowhere else: no gear on a tab
 * behind, and no settings button of its own in the title bar. The app menu's Settings… (`⌘,`,
 * `Ctrl+,` off a Mac, which the core says with an event) opens Settings at the focused
 * workspace's level, else the project's, else You. What each level shows is `ProjectLevel`'s and `WorkspaceLevel`'s.
 */

vi.mock("../SessionPane", () => ({
  SessionPane: ({ session }: { session: number }) => (
    <div data-testid="pane">session {session}</div>
  ),
}));

const PLANE = "/home/dev/plane";

const FILE = (which: "shared" | "local") => ({
  which,
  file: which === "shared" ? "charter.toml" : "charter.local.toml",
  exists: which === "shared",
  text: "",
  refusals: [],
  parsed: true,
  fields: [],
});

const WORKSPACE = (workspace: string) => ({
  workspace,
  file: `workspaces/${workspace}/workspace.json`,
  exists: true,
  text: "{}\n",
  refusals: [],
  parsed: true,
  fields: [],
  live: false,
});

beforeEach(() => {
  forgetThisLaunch();
  (globalThis as Record<string, unknown>)[GLOBAL] = {
    layout: { path: "/home/op/.config/charter/layout.json", found: false, document: null },
    theme: { path: "", found: false, document: null, trouble: null },
  };
});
afterEach(() => {
  cleanup();
  clearMocks();
  Reflect.deleteProperty(globalThis, GLOBAL);
});

function core(plane: string | null) {
  mockIPC(
    (cmd, args) => {
      if (cmd === "plane_at_launch")
        return { plane, from: plane, why: plane === null ? "no plane here" : null };
      if (cmd === "opened_chats") return [];
      if (cmd === "reopened_views") return [];
      if (cmd === "chats_that_would_not_start") return [];
      if (cmd === "running_sessions") return [];
      if (cmd === "chat_states") return [];
      if (cmd === "plane_sidebar")
        return {
          root: plane,
          personas: [],
          persona: null,
          unfiled: [],
          workspaces: ["alpha", "beta"].map((name) => ({
            name,
            path: `${PLANE}/workspaces/${name}`,
            vision: "",
            todos: [],
            chats: [],
          })),
        };
      if (cmd === "project_settings") return { shared: FILE("shared"), local: FILE("local") };
      if (cmd === "workspace_settings") return WORKSPACE((args as { workspace: string }).workspace);
      return null;
    },
    { shouldMockEvents: true },
  );
}

const projects = () => stripNamed("Projects");
const chatTabs = () => within(stripNamed("Tabs")).queryAllByRole("tab");
const inFront = () =>
  within(stripNamed("Tabs"))
    .queryAllByRole("tab")
    .find((tab) => tab.getAttribute("aria-selected") === "true");

/** The level the Settings tab on screen is at, by its switcher. */
const level = () =>
  within(screen.getByRole("radiogroup", { name: /level/i }))
    .getAllByRole("radio")
    .find((radio) => (radio as HTMLInputElement).checked)
    ?.closest("label")
    ?.textContent?.trim();

async function settled() {
  // The listeners register asynchronously; give them a turn.
  await act(async () => {
    await new Promise((resolve) => setTimeout(resolve, 0));
  });
}

async function menuSettings() {
  await act(async () => {
    await emit("settings-asked");
  });
}

describe("⌘, opens Settings at the focused level", () => {
  it("opens the focused workspace's level when a workspace is focused", async () => {
    core(PLANE);
    render(<App />);
    await goToWorkspace("alpha");
    await settled();

    await menuSettings();

    await waitFor(() => expect(level()).toBe("Workspace"));
    expect(inFront()).toHaveTextContent("Settings · alpha");
  });

  it("opens the project's level when no workspace is focused", async () => {
    core(PLANE);
    render(<App />);
    // The plane root, first in the title bar's ⌄ (#2916), is not a workspace.
    await goToWorkspace("Plane root");
    await waitFor(() => expect(workspaceInFront()).toBeUndefined());
    await settled();

    await menuSettings();

    await waitFor(() => expect(level()).toBe("Project"));
  });

  it("opens You when no project is open", async () => {
    core(null);
    render(<App />);
    await waitFor(() =>
      expect(
        screen.queryByRole("heading", { name: "Welcome to purlis", level: 1 }),
      ).toBeInTheDocument(),
    );
    await settled();

    await menuSettings();

    await waitFor(() => expect(level()).toBe("You"));
  });

  it("brings the tab already open forward rather than opening a second", async () => {
    core(PLANE);
    render(<App />);
    await goToWorkspace("alpha");
    await settled();

    await menuSettings();
    await waitFor(() => expect(level()).toBe("Workspace"));
    await menuSettings();

    expect(chatTabs().filter((tab) => /Settings · /.test(tab.textContent ?? ""))).toHaveLength(1);
  });
});

describe("Your settings…, in the palette", () => {
  it("opens You while a workspace is focused, where Settings… would open the workspace's", async () => {
    core(PLANE);
    render(<App />);
    await goToWorkspace("alpha");

    await userEvent.keyboard("{F2}");
    await screen.findByRole("dialog", { name: "Command palette" });
    await userEvent.keyboard("Your settings");
    await userEvent.keyboard("{Enter}");

    await waitFor(() => expect(level()).toBe("You"));
  });
});

/**
 * **The context-menu key still opens a tab's menu** (SE-23 review). The menu's trigger is the
 * tab itself and not the cell that holds it: the family's context menu (`ui/menu/ContextMenu.tsx`) opens only
 * when the trigger itself has the keyboard, so a trigger on the cell never opened from it.
 */
describe("a tab's menu, from the keyboard", () => {
  for (const key of [{ key: "F10", shiftKey: true }, { key: "ContextMenu" }]) {
    const said = key.shiftKey ? "Shift+F10" : "the menu key";

    it(`opens a workspace chip's menu on ${said}`, async () => {
      core(PLANE);
      render(<App />);
      // alpha's chip in the title bar (#2916), whose menu is the workspace's.
      const tab = await waitFor(() => {
        const chip = workspaceChips().find((one) => chipName(one) === "alpha");
        if (chip === undefined) throw new Error("no alpha chip yet");
        return chip;
      });
      tab.focus();

      fireEvent.keyDown(tab, key);

      const menu = await screen.findByRole("menu");
      expect(within(menu).getByRole("menuitem", { name: "Settings…" })).toBeVisible();
    });

    it(`opens the project tab's menu on ${said}`, async () => {
      core(PLANE);
      render(<App />);
      await waitFor(() => expect(workspaceChips().map(chipName)).toContain("alpha"));
      const tab = within(projects()).getByRole("tab", { selected: true });
      tab.focus();

      fireEvent.keyDown(tab, key);

      const menu = await screen.findByRole("menu");
      expect(within(menu).getByRole("menuitem", { name: "Settings…" })).toBeVisible();
    });
  }
});
