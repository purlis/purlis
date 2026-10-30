// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **Every close that would lose a view's unsaved work asks first, through one registry**
 * (#3929, `unsavedWork.ts`): a view tab's close, a project's close and a quit, drawn in the
 * window with the core faked at its command boundary. What the view registers is put in the
 * registry directly, as any view does.
 */
import { StrictMode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, render as renderBare, screen, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import App from "./App";
import { closeFromTheRail, stripNamed } from "./test-strips";
import { settingsView, type ViewRef } from "./tabs";
import { keepUnsaved, unsavedNow, type UnsavedWork } from "./unsavedWork";

vi.mock("./SessionPane", () => ({
  SessionPane: ({ session }: { session: number }) => (
    <div data-testid="pane">session {session}</div>
  ),
}));

const render = (ui: React.ReactElement) => renderBare(<StrictMode>{ui}</StrictMode>);

afterEach(() => {
  cleanup();
  clearMocks();
  for (const one of unsavedNow()) keepUnsaved(one.key, undefined);
});

declare global {
  interface Window {
    __TAURI_INTERNALS__: { runCallback: (id: number, payload: unknown) => void };
  }
}

const ONE = "/home/dev/one";
const TWO = "/home/dev/two";

/** A core holding `restore`'s projects with no chat open, recording each ask. */
function core(restore: string[]) {
  const asked: { cmd: string; args: Record<string, unknown> }[] = [];
  const listeners = new Map<string, number[]>();
  mockIPC((cmd, args) => {
    const given = (args ?? {}) as Record<string, unknown>;
    asked.push({ cmd, args: given });
    if (cmd === "plugin:event|listen") {
      const { event, handler } = given as unknown as { event: string; handler: number };
      listeners.set(event, [...(listeners.get(event) ?? []), handler]);
      return 1;
    }
    const plane = (given.plane as string | undefined) ?? "";
    if (cmd === "plane_at_launch") return { plane: null, from: null, why: null };
    if (cmd === "planes_to_restore")
      return { windows: [{ planes: restore, active: restore.length - 1 }], dropped: [] };
    if (cmd === "projects_handed") return null;
    if (cmd === "charter_windows") return ["main"];
    if (cmd === "open_plane") return { plane: given.path, ask: null };
    if (cmd === "plane_sidebar")
      return { root: plane, workspaces: [], personas: [], persona: null, unfiled: [] };
    if (cmd === "opened_chats") return [];
    if (cmd === "chat_states") return [];
    if (cmd === "chats_that_would_not_start") return [];
    if (cmd === "recent_planes") return { planes: [], dropped: [], forgetful: null };
    if (cmd === "extensions_on") return [];
    return null;
  });
  return {
    sent: (cmd: string) => asked.filter((one) => one.cmd === cmd).map((one) => one.args),
    listening: (event: string) => (listeners.get(event) ?? []).length > 0,
    /** What the core tells the window, as Tauri delivers it. */
    fire: (event: string, payload: unknown) =>
      act(() => {
        for (const handler of listeners.get(event) ?? [])
          window.__TAURI_INTERNALS__.runCallback(handler, { event, id: 1, payload });
      }),
  };
}

/** A view's unsaved work in `plane`, as the view registers it. */
function work(plane: string, view: ViewRef = settingsView("project")): UnsavedWork {
  const one: UnsavedWork = {
    key: `${plane}\u0000deploy`,
    plane,
    view,
    name: "deploy",
    save: vi.fn(() => Promise.resolve(undefined)),
    discard: vi.fn(() => keepUnsaved(one.key, undefined)),
  };
  keepUnsaved(one.key, one);
  return one;
}

const projectTabs = () =>
  within(stripNamed("Projects"))
    .getAllByRole("tab")
    .map((tab) => tab.getAttribute("aria-label"));

const question = () => screen.findByRole("alertdialog", { name: "Save your changes to deploy?" });

describe("a project's close, with a view's unsaved work in it", () => {
  it("asks about the work first: Keep editing keeps the project, Discard and leave closes it", async () => {
    const { sent } = core([ONE, TWO]);
    render(<App />);
    await vi.waitFor(() => expect(projectTabs()).toEqual(["one", "two"]));
    const deploy = work(TWO);

    await closeFromTheRail("two");
    await userEvent.click(within(await question()).getByRole("button", { name: "Keep editing" }));
    expect(projectTabs()).toEqual(["one", "two"]);
    expect(sent("close_plane")).toEqual([]);
    expect(deploy.discard).not.toHaveBeenCalled();

    await closeFromTheRail("two");
    await userEvent.click(
      within(await question()).getByRole("button", { name: "Discard and leave" }),
    );
    // Nothing else to end: it closes at once.
    await vi.waitFor(() => expect(sent("close_plane")).toEqual([{ plane: TWO }]));
    expect(deploy.discard).toHaveBeenCalledOnce();
    await vi.waitFor(() => expect(projectTabs()).toEqual(["one"]));
  });

  it("is not asked about another project's work", async () => {
    const { sent } = core([ONE, TWO]);
    render(<App />);
    await vi.waitFor(() => expect(projectTabs()).toEqual(["one", "two"]));
    const elsewhere = work(ONE);

    await closeFromTheRail("two");
    await vi.waitFor(() => expect(sent("close_plane")).toEqual([{ plane: TWO }]));
    expect(screen.queryByRole("alertdialog")).toBeNull();
    expect(unsavedNow()).toEqual([elsewhere]);
  });

  it("lets the work go with the project when the core let go of it", async () => {
    const { fire, listening } = core([ONE, TWO]);
    render(<App />);
    await vi.waitFor(() => expect(projectTabs()).toEqual(["one", "two"]));
    await vi.waitFor(() => expect(listening("plane-closed")).toBe(true));
    const deploy = work(TWO);

    fire("plane-closed", { plane: TWO });

    await vi.waitFor(() => expect(projectTabs()).toEqual(["one"]));
    expect(deploy.discard).toHaveBeenCalledOnce();
    expect(unsavedNow()).toEqual([]);
  });
});

describe("a quit, with a view's unsaved work in the window", () => {
  it("asks about the work first: Keep editing is a quit not taken, Save and leave quits", async () => {
    const { fire, listening, sent } = core([ONE]);
    render(<App />);
    await vi.waitFor(() => expect(listening("quit-asked")).toBe(true));
    const deploy = work(ONE);

    fire("quit-asked", null);
    await userEvent.click(within(await question()).getByRole("button", { name: "Keep editing" }));
    expect(sent("quit_cancelled")).toHaveLength(1);
    expect(sent("quit")).toEqual([]);

    fire("quit-asked", null);
    await userEvent.click(within(await question()).getByRole("button", { name: "Save and leave" }));
    expect(deploy.save).toHaveBeenCalledOnce();
    // Saved, and nothing else to end: it quits.
    await vi.waitFor(() => expect(sent("quit")).toHaveLength(1));
  });
});

describe("a view tab's close, with its unsaved work", () => {
  it("asks about the work first, and closes the tab on a leave", async () => {
    const { fire, listening } = core([ONE]);
    render(<App />);
    await vi.waitFor(() => expect(listening("settings-asked")).toBe(true));
    fire("settings-asked", null);
    const tabs = () =>
      within(stripNamed("Tabs"))
        .queryAllByRole("tab")
        .map((tab) => tab.getAttribute("aria-label"));
    await vi.waitFor(() => expect(tabs()).toEqual(["Settings"]));
    const deploy = work(ONE);

    await userEvent.keyboard("{F2}");
    await screen.findByRole("dialog", { name: "Command palette" });
    await userEvent.keyboard("Close this view{Enter}");
    await userEvent.click(within(await question()).getByRole("button", { name: "Keep editing" }));
    expect(tabs()).toEqual(["Settings"]);

    await userEvent.keyboard("{F2}");
    await screen.findByRole("dialog", { name: "Command palette" });
    await userEvent.keyboard("Close this view{Enter}");
    await userEvent.click(
      within(await question()).getByRole("button", { name: "Discard and leave" }),
    );
    expect(deploy.discard).toHaveBeenCalledOnce();
    await vi.waitFor(() => expect(tabs()).toEqual([]));
  });
});
