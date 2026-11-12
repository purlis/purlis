// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, render, screen, waitFor } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { CloneToVouch } from "./bindings";
import { PlaneView } from "./PlaneView";
import type { WindowDoing } from "./planeReport";
import { WINDOW_CONTROLS, goToControl } from "./windowControls";

vi.mock("./SessionPane", () => ({
  SessionPane: ({ session }: { session: number }) => <div>session {session}</div>,
}));

afterEach(() => {
  cleanup();
  clearMocks();
});

const PLANE = "/home/dev/plane";
const windowDoes = new Proxy({} as WindowDoing, { get: () => vi.fn() });
const NONE: never[] = [];

const clone = (folder: string, place: string): CloneToVouch => ({
  workspace: "smart-ide",
  folder,
  place,
  places: [place],
});

const TRUST = WINDOW_CONTROLS[0];

/**
 * **Go to, through the window** (R-333): with the Notices view closed, a result's Go to opens
 * it, and the keyboard ends on the control on the Notice the result names, not on the view's
 * first stop, which `PlaneView` gives the keyboard a frame after it shows a view.
 */
describe("Go to a control a result names, through the window", () => {
  it("opens the closed Notices view with the keyboard on the clone's Trust this clone", async () => {
    mockIPC((cmd) => {
      if (cmd === "clones_to_vouch")
        return [
          clone("workspaces/smart-ide/repos/api", "github.com/acme/api"),
          clone("workspaces/smart-ide/repos/web", "github.com/acme/web"),
        ];
      return null;
    });
    render(
      <PlaneView
        plane={PLANE}
        inFront
        kept
        slot={document.body}
        projects={NONE}
        pinnedProjects={NONE}
        window={windowDoes}
        onReport={() => undefined}
      />,
    );
    await waitFor(() =>
      expect(document.querySelector('[data-cause^="clone-vouch:"]')).not.toBeNull(),
    );
    const view = () =>
      document.querySelector<HTMLElement>('[role="tabpanel"][data-view="notices"]');
    // The Notices view is closed: drawn, and hidden.
    expect(view()?.hidden).toBe(true);
    act(() => goToControl(TRUST, PLANE, "web was not saved"));
    await waitFor(() => expect(view()?.hidden).toBe(false));
    // A frame on, where the view's own keyboard step runs.
    await act(() => new Promise((done) => requestAnimationFrame(() => done(undefined))));
    await act(() => new Promise((done) => requestAnimationFrame(() => done(undefined))));
    const trust = screen
      .getAllByRole("button", { name: "Trust this clone" })
      .find((one) => one.closest('[data-cause*="repos/web"]') !== null);
    expect(document.activeElement).toBe(trust);
  });
});
