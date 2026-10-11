import { StrictMode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import App from "./App";
import { stripNamed } from "./test-strips";

/**
 * **About's licences, in a view tab** (#1719): About sits in the title bar, outside any
 * project, so its licences open as a tab of the project in front, once however often they are
 * asked for. What About draws with no project open is `About.test.tsx`'s.
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

const PLANE = "/home/dev/plane";

function core() {
  mockIPC((cmd) => {
    if (cmd === "plane_at_launch") return { plane: PLANE, from: PLANE, why: null };
    if (cmd === "opened_chats") return [];
    if (cmd === "chats_that_would_not_start") return [];
    if (cmd === "running_sessions") return [];
    if (cmd === "chat_states") return [];
    if (cmd === "plane_sidebar")
      return { root: PLANE, personas: [], persona: null, unfiled: [], workspaces: [] };
    if (cmd === "about_charter") return new Promise(() => {});
    return null;
  });
}

describe("About's licences, from the window", () => {
  it("open as a tab of the project in front, and once", async () => {
    core();
    render(
      <StrictMode>
        <App />
      </StrictMode>,
    );
    await screen.findByTestId("title-about");
    const open = async () => {
      await userEvent.click(screen.getByTestId("title-about"));
      const dialog = await screen.findByRole("dialog");
      await userEvent.click(within(dialog).getByRole("link", { name: "their licences" }));
      await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    };
    const tabs = () =>
      within(stripNamed("Tabs"))
        .getAllByRole("tab")
        .filter((tab) => tab.textContent?.includes("Licences"));

    await open();
    const view = await screen.findByTestId("licences-view");
    expect(within(view).getByText(/material-icon-theme .*, under the MIT licence/)).toBeTruthy();
    expect(tabs()).toHaveLength(1);

    await open();
    expect(tabs()).toHaveLength(1);
  });
});
