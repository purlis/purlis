import { StrictMode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, render as renderBare, screen, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import App from "./App";
import { stripNamed } from "./test-strips";

/**
 * **The window's own lines survive a project coming and going** (D-LB-1). They are written once
 * in App.tsx and only drawn in two places: under the title bar while a project is in front, at
 * the top of the opener's page while none is. Moving between the two must move the line, not
 * draw a new one, so what the operator did with it (a dismissal, an answer) is never lost.
 */

vi.mock("./SessionPane", () => ({
  SessionPane: ({ session }: { session: number }) => (
    <div data-testid="pane">session {session}</div>
  ),
}));

const render = (ui: React.ReactElement) => renderBare(<StrictMode>{ui}</StrictMode>);

afterEach(() => {
  cleanup();
  clearMocks();
});

declare global {
  interface Window {
    __TAURI_INTERNALS__: { runCallback: (id: number, payload: unknown) => void };
  }
}

const ONE = "/home/dev/one";
const SLOW = { said: "purlis took 31 s to start, against a 2 s limit.", relaunch: null };

function core() {
  const listeners = new Map<string, number[]>();
  mockIPC((cmd, args) => {
    const given = (args ?? {}) as Record<string, unknown>;
    if (cmd === "plugin:event|listen") {
      const { event, handler } = given as unknown as { event: string; handler: number };
      listeners.set(event, [...(listeners.get(event) ?? []), handler]);
      return 1;
    }
    const plane = (given.plane as string | undefined) ?? "";
    if (cmd === "first_frame") return SLOW;
    if (cmd === "plane_at_launch") return { plane: null, from: null, why: null };
    if (cmd === "planes_to_restore")
      return { windows: [{ planes: [ONE], active: 0 }], dropped: [] };
    if (cmd === "charter_windows") return ["main"];
    if (cmd === "open_plane") return { plane: given.path, ask: null };
    if (cmd === "plane_sidebar")
      return { root: plane, workspaces: [], personas: [], persona: null, unfiled: [] };
    if (cmd === "opened_chats") return [];
    if (cmd === "chat_states") return [];
    if (cmd === "chats_that_would_not_start") return [];
    if (cmd === "recent_planes")
      return { planes: [{ path: ONE, name: "one", approved: true }], dropped: [], forgetful: null };
    if (cmd === "extensions_on") return [];
    return null;
  });
  const fire = (event: string, payload: unknown) => {
    const handlers = listeners.get(event) ?? [];
    if (handlers.length === 0) throw new Error(`the window is not listening for ${event}`);
    act(() => {
      for (const handler of handlers)
        window.__TAURI_INTERNALS__.runCallback(handler, { event, id: 1, payload });
    });
  };
  return { fire, listening: (event: string) => (listeners.get(event) ?? []).length > 0 };
}

const projectTabs = () =>
  within(stripNamed("Projects"))
    .getAllByRole("tab")
    .map((tab) => tab.querySelector(".project-name")?.textContent);

/** The one slow-start line in the document, wherever it is drawn. */
const slowLine = () => {
  const all = document.querySelectorAll('[data-cause="slow-start"]');
  expect(all).toHaveLength(1);
  return all[0];
};

describe("the window's own lines", () => {
  it("move with the opener and back, as the same line, and stay dismissed", async () => {
    const { fire, listening } = core();
    render(<App />);
    await vi.waitFor(() => expect(projectTabs()).toEqual(["one"]));
    await vi.waitFor(() => expect(listening("plane-closed")).toBe(true));

    // A project in front: the line stands under the title bar, outside the opener's page.
    const line = await vi.waitFor(() => slowLine());
    expect(line.closest(".window-notices-at")).toBeNull();

    // The project goes: the same line, now at the top of the opener's page.
    fire("plane-closed", { plane: ONE });
    const heading = await screen.findByRole("heading", { level: 1 });
    await vi.waitFor(() => expect(slowLine().closest(".window-notices-at")).not.toBeNull());
    expect(slowLine()).toBe(line);
    expect(slowLine().compareDocumentPosition(heading) & Node.DOCUMENT_POSITION_FOLLOWING).toBe(
      Node.DOCUMENT_POSITION_FOLLOWING,
    );

    // A project comes back from the opener: the same line again, under the title bar.
    await userEvent.click(screen.getByRole("button", { name: /^one/ }));
    await vi.waitFor(() => expect(projectTabs()).toEqual(["one"]));
    await vi.waitFor(() => expect(slowLine().closest(".window-notices-at")).toBeNull());
    expect(slowLine()).toBe(line);

    // Dismissed with a project in front, it stays dismissed when the project goes.
    await userEvent.click(within(line as HTMLElement).getByRole("button", { name: "Dismiss" }));
    expect(document.querySelector('[data-cause="slow-start"]')).toBeNull();
    fire("plane-closed", { plane: ONE });
    await screen.findByRole("heading", { level: 1 });
    expect(document.querySelector('[data-cause="slow-start"]')).toBeNull();
  });
});
