// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { StrictMode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, render as renderBare, screen, waitFor } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import App from "./App";

/**
 * **The palette opening and closing draw no project again** (#4351): the window reads the
 * project's profiles as the palette opens over it (#1201) and gives the project's view only its
 * own list, so nothing the view is drawn from changes as the palette comes and goes, and a row
 * run from the palette does not pay for a draw of the whole project on its way to what it
 * opens.
 */

/** How many times a chat's pane has been drawn: drawn by the project's view on each of its own
 *  draws. */
const paneDrawn = vi.hoisted(() => ({ times: 0 }));
vi.mock("./SessionPane", () => ({
  SessionPane: ({ session }: { session: number }) => {
    paneDrawn.times += 1;
    return <div data-testid="pane">session {session}</div>;
  },
}));

const render = (ui: React.ReactElement) => renderBare(<StrictMode>{ui}</StrictMode>);

afterEach(() => {
  cleanup();
  clearMocks();
});

const PLANE = "/home/dev/plane";
const ALPHA = `${PLANE}/workspaces/alpha`;

const CHAT = {
  session: 1,
  name: "steward 1",
  cwd: ALPHA,
  harness: "claude",
  in_front: true,
  resumed: null,
  fresh: null,
  profile: null,
  persona: "steward",
  unreported: null,
  card: null,
  guessed: null,
  pinned: false,
  label: null,
  from: null,
};

function core(entries: unknown[] = []) {
  const asked: string[] = [];
  /** The profiles' answer, held until the test lets it land. */
  let answer: () => void = () => undefined;
  mockIPC(
    (cmd) => {
      asked.push(cmd);
      if (cmd === "plane_at_launch") return { plane: PLANE, from: PLANE, why: null };
      if (cmd === "opened_chats") return [CHAT];
      if (cmd === "plane_sidebar")
        return {
          root: PLANE,
          personas: [],
          persona: null,
          unfiled: [],
          workspaces: [{ name: "alpha", path: ALPHA, vision: "", todos: [], chats: [{ ...CHAT }] }],
        };
      if (cmd === "project_settings")
        return new Promise((land) => {
          answer = () =>
            land({
              shared: { which: "shared", parsed: true, entries: [] },
              local: { which: "local", parsed: true, entries },
            });
        });
      if (cmd === "about_charter") return new Promise(() => {});
      if (cmd === "chat_states" || cmd === "chats_that_would_not_start") return [];
      if (cmd === "running_sessions" || cmd === "chats_plane_updated") return [];
      return null;
    },
    { shouldMockEvents: true },
  );
  return { asked, land: () => answer() };
}

const settle = () =>
  act(async () => {
    await new Promise((done) => setTimeout(done, 0));
  });

describe("the palette coming and going (#4351)", () => {
  it("draws no project again, even when the profiles' answer lands after it closed", async () => {
    const { asked, land } = core();
    render(<App />);
    await screen.findByTestId("pane");
    await settle();
    // A project's view mounting reads nothing: only an opening does.
    expect(asked).not.toContain("project_settings");
    const drawn = paneDrawn.times;

    await userEvent.keyboard("{F2}");
    await screen.findByRole("dialog", { name: "Command palette" });
    await waitFor(() => expect(asked).toContain("project_settings"));
    await userEvent.keyboard("{Escape}");
    await waitFor(() =>
      expect(screen.queryByRole("dialog", { name: "Command palette" })).toBeNull(),
    );
    await act(async () => land());
    await settle();

    expect(paneDrawn.times).toBe(drawn);
  });

  it("draws no project again for a reading of the same profiles", async () => {
    const { asked, land } = core([
      { collection: "profiles", id: "profiles:claude", label: "claude", keys: [], values: [] },
    ]);
    render(<App />);
    await screen.findByTestId("pane");
    await settle();
    const opening = async () => {
      const before = asked.filter((one) => one === "project_settings").length;
      await userEvent.keyboard("{F2}");
      await screen.findByRole("dialog", { name: "Command palette" });
      await waitFor(() =>
        expect(asked.filter((one) => one === "project_settings").length).toBeGreaterThan(before),
      );
      await act(async () => land());
      await settle();
      await userEvent.keyboard("{Escape}");
      await waitFor(() =>
        expect(screen.queryByRole("dialog", { name: "Command palette" })).toBeNull(),
      );
      await settle();
    };
    // The first reading gives the project its list: one draw, which is the point of it.
    await opening();
    const drawn = paneDrawn.times;

    await opening();

    expect(paneDrawn.times).toBe(drawn);
  });
});
