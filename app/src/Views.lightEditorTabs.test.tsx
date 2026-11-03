// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { StrictMode, act } from "react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import App from "./App";
import type { ViewTab } from "./bindings";
import { jumpTo } from "./fileJump";
import { stripNamed } from "./test-strips";
import { themeInForce } from "./theme/inForce.testkit";

/**
 * **A light editor tab after another one opens at once** (#4799): the editor's chunk is loaded
 * by the first file tab of any kind, and a later tab of another kind draws in the same commit
 * that brings it forward. Each kind used to be a lazy component of its own, so the first tab of
 * the second kind waited for React to retry it, while the tab before it stayed in the page,
 * hidden, under the new tab's name. Only a render for some other reason (a sidebar read after
 * every tab change, before #4799) made that retry come soon.
 */

vi.mock("./SessionPane", () => ({
  SessionPane: ({ session }: { session: number }) => (
    <div data-testid="pane" tabIndex={-1}>
      session {session}
    </div>
  ),
}));

beforeEach(() => globalThis.localStorage.clear());
afterEach(() => {
  cleanup();
  clearMocks();
});

const PLANE = "/home/dev/plane";
const ALPHA = `${PLANE}/workspaces/alpha`;
const CHAT = {
  session: 1,
  name: "1",
  cwd: ALPHA,
  harness: "claude",
  in_front: false,
  resumed: null,
  fresh: null,
  profile: "claude",
  persona: "steward",
};
const SIDEBAR = {
  root: PLANE,
  workspaces: [{ name: "alpha", path: ALPHA, vision: "", todos: [], chats: [CHAT] }],
  personas: ["steward"],
  persona: "steward",
  unfiled: [],
};
/** A file's own tab, put back in front at launch: the first light editor tab drawn. */
const LOGIN_FORM: ViewTab = {
  from: null,
  view: "piece-file",
  key: "alpha/svc/fix-login/login-form.ts",
  title: "login-form.ts · fix-login",
  workspace: "alpha",
  at: 0,
  active: true,
  pinned: false,
};
const FILES: Record<string, string> = {
  "login-form.ts": "export const form = 1;\n",
  "notes.md": "# Notes\n\nnothing here\n\nthe quokka-sentinel lives on line five\n",
};

it("brings a search hit's file tab forward with its own content, after a file's tab", async () => {
  mockIPC(
    (cmd, args) => {
      const a = (args ?? {}) as Record<string, string>;
      if (cmd === "plane_at_launch") return { plane: PLANE, from: PLANE, why: null };
      if (cmd === "plane_sidebar") return structuredClone(SIDEBAR);
      if (cmd === "opened_chats") return [CHAT];
      if (cmd === "reopened_views") return [LOGIN_FORM];
      if (cmd === "chat_states") return [];
      if (cmd === "chats_that_would_not_start") return [];
      if (cmd === "running_sessions") return [];
      if (cmd === "extension_views") return [];
      if (cmd === "extension_commands") return [];
      if (cmd === "extension_panels") return [];
      if (cmd === "extensions_on") return [];
      if (cmd === "theme_in_force") return themeInForce(args);
      if (cmd === "project_theme_drawn") return null;
      if (cmd === "branch_tree")
        return a.folder === ""
          ? {
              entries: Object.keys(FILES).map((name) => ({ name, kind: "file" })),
              more: 0,
            }
          : { entries: [], more: 0 };
      if (cmd === "branch_status") return { changes: [], folders: [], more: 0, base: "main" };
      if (cmd === "piece_file") return { kind: "text", text: FILES[a.path] };
      return null;
    },
    { shouldMockEvents: true },
  );
  render(
    <StrictMode>
      <App />
    </StrictMode>,
  );
  const first = await screen.findByTestId("light-editor", {}, { timeout: 5000 });
  expect(first).toHaveTextContent(/export const form/);

  act(() =>
    jumpTo({
      plane: PLANE,
      place: { workspace: "alpha", repo: "svc", piece: "fix-login" },
      path: "notes.md",
      line: 5,
    }),
  );

  // In the commit that brings the tab forward: no wait for the editor's chunk, and nothing of
  // the tab it replaced left in the page.
  expect(within(stripNamed("Tabs")).getByRole("tab", { selected: true })).toHaveTextContent(
    /Files · fix-login/,
  );
  expect(screen.queryByText("Opening the light editor…")).toBeNull();
  expect(first).not.toBeInTheDocument();

  await waitFor(() =>
    expect(screen.getByTestId("light-editor")).toHaveTextContent(/quokka-sentinel/),
  );
});
