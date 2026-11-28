// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { StrictMode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, render as renderBare, screen, waitFor } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import App from "./App";
import { aTabForEachChat } from "./laidOut.testkit";
import { forgetStudioReadAhead } from "./studioReadAhead";

/**
 * **The studio's skills are asked for as it is opened, not once it is drawn** (#4351, L10): the
 * read is most of an opening, so it runs while the tab is being drawn, and the tab draws the
 * answer to that one read.
 */

vi.mock("./SessionPane", () => ({
  SessionPane: ({ session }: { session: number }) => (
    <div data-testid="pane">session {session}</div>
  ),
}));

const render = (ui: React.ReactElement) => renderBare(<StrictMode>{ui}</StrictMode>);

afterEach(() => {
  cleanup();
  // Before the mocks go: it stops listening through them.
  forgetStudioReadAhead();
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

/** Each `skill_studio` the window asked, with whether the studio's tab was drawn by then. */
type StudioAsked = { workspace: unknown; tabDrawn: boolean };

function core() {
  const studioAsked: StudioAsked[] = [];
  mockIPC(
    (cmd, args) => {
      if (cmd === "skill_studio") {
        studioAsked.push({
          workspace: (args as { workspace?: unknown }).workspace,
          tabDrawn: document.querySelector('[data-testid="skill-studio"]') !== null,
        });
        // Held, as a read of two hundred skills is while the tab is drawn.
        return new Promise(() => undefined);
      }
      if (cmd === "plane_at_launch") return { plane: PLANE, from: PLANE, why: null };
      if (cmd === "opened_chats") return [CHAT];
      // Its tab on alpha's strip, from the stored layout (#4758).
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
  return { studioAsked };
}

const settle = () =>
  act(async () => {
    await new Promise((done) => setTimeout(done, 0));
  });

/** Runs alpha's Open Skill studio from the palette, as a person does, until its tab is drawn. */
async function openFromThePalette() {
  await userEvent.keyboard("{F2}");
  await userEvent.keyboard("Open Skill studio");
  const row = await waitFor(() => {
    const found = document.getElementById("palette-row-skills.studio:alpha");
    expect(found).not.toBeNull();
    return found as HTMLElement;
  });
  await userEvent.click(row);
  await waitFor(() => {
    expect(screen.getByTestId("skill-studio")).toBeTruthy();
    expect(document.getElementById("palette-query")).toBeNull();
  });
  await settle();
}

describe("opening the Skill studio from the palette (#4351)", () => {
  it("asks for its skills before its tab is drawn, and only once", async () => {
    const { studioAsked } = core();
    render(<App />);
    await screen.findByTestId("pane");
    await settle();

    await openFromThePalette();

    expect(studioAsked).toEqual([{ workspace: "alpha", tabDrawn: false }]);
  });

  it("asks nothing more when its tab is already open on those skills", async () => {
    const { studioAsked } = core();
    render(<App />);
    await screen.findByTestId("pane");
    await settle();
    await openFromThePalette();
    await openFromThePalette();

    expect(studioAsked).toHaveLength(1);
  });
});
