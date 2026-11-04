// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { StrictMode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render as renderBare, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import App from "./App";
import { aTabForEachChat } from "./laidOut.testkit";
import { OPEN_SKILL_STUDIO } from "./skillStudio";

/**
 * **The Skill studio is always on** (#2786): no flag is asked of the core, so a window whose
 * core answers nothing about the studio still offers it on the palette and draws the Skills view
 * on the left side.
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

/** The core: one chat in alpha, and no answer of its own to anything about the studio. */
function core() {
  mockIPC(
    (cmd) => {
      if (cmd === "plane_at_launch") return { plane: PLANE, from: PLANE, why: null };
      if (cmd === "opened_chats") return [CHAT];
      // The chat's tab, as the stored layout keeps it (#4758).
      if (cmd === "reopened_layout") return aTabForEachChat([CHAT], () => "alpha");
      if (cmd === "plane_sidebar")
        return {
          root: PLANE,
          personas: [],
          persona: null,
          unfiled: [],
          workspaces: [{ name: "alpha", path: ALPHA, vision: "", todos: [], chats: [{ ...CHAT }] }],
        };
      if (cmd === "about_charter") return new Promise(() => {});
      if (cmd === "chat_states" || cmd === "chats_that_would_not_start") return [];
      if (cmd === "running_sessions" || cmd === "chats_plane_updated") return [];
      return null;
    },
    { shouldMockEvents: true },
  );
}

describe("the Skill studio with no flag to ask (#2786)", () => {
  it("is offered on the palette", async () => {
    core();
    render(<App />);
    await screen.findByTestId("pane");

    await userEvent.keyboard("{F2}");
    await screen.findByRole("dialog", { name: "Command palette" });
    await userEvent.type(screen.getByRole("combobox"), OPEN_SKILL_STUDIO);

    expect(
      await screen.findAllByRole("option", { name: new RegExp(`^${OPEN_SKILL_STUDIO}`) }),
    ).not.toHaveLength(0);
  });

  it("draws the Skills view on the left side's bar", async () => {
    core();
    render(<App />);
    await screen.findByTestId("pane");

    await waitFor(() =>
      expect(
        within(screen.getByRole("tablist", { name: "Navigation" })).getByRole("tab", {
          name: "Skills",
        }),
      ).toBeInTheDocument(),
    );
  });
});
