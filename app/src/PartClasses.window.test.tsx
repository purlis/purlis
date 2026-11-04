// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, render, screen } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import App from "./App";
import { partsWithoutTheirClass } from "./ui/partClasses";

/**
 * **Every part the window draws carries its classes** (#4351; ADR 0103, amended 2026-11-04): the
 * window's CSS is keyed on `ds-<scope>` and `dp-<part>`, so a part drawn without them is drawn
 * unstyled, and one carrying another part's is styled as that part. The stories hold each family
 * to the same in Chromium (`.storybook/parts.setup.ts`); this holds the window they make.
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
        return {
          shared: { which: "shared", parsed: true, entries: [] },
          local: { which: "local", parsed: true, entries },
        };
      if (cmd === "about_charter") return new Promise(() => {});
      if (cmd === "chat_states" || cmd === "chats_that_would_not_start") return [];
      if (cmd === "running_sessions" || cmd === "chats_plane_updated") return [];
      return null;
    },
    { shouldMockEvents: true },
  );
  return { asked };
}

const settle = () =>
  act(async () => {
    await new Promise((done) => setTimeout(done, 0));
  });

describe("the window's parts (#4351)", () => {
  it("each carry the classes of their scope and part, and no other part's", async () => {
    core();
    render(<App />);
    await screen.findByTestId("pane");
    await settle();
    expect(partsWithoutTheirClass(document.body)).toEqual([]);

    await userEvent.keyboard("{F2}");
    await screen.findByRole("dialog", { name: "Command palette" });
    await settle();
    expect(partsWithoutTheirClass(document.body)).toEqual([]);
  });
});
