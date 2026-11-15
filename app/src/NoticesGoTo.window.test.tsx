// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useRef } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, render, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { Said } from "./actions";
import type { CloneToVouch } from "./bindings";
import { ChipCard, useChipNotices } from "./ChipNotices";
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
  made: null,
});

const TRUST = WINDOW_CONTROLS[0];

/** The project's clones: two, each with its Notice and its Trust this clone. */
const twoClones = () =>
  mockIPC((cmd) => {
    if (cmd === "clones_to_vouch")
      return [
        clone("workspaces/smart-ide/repos/api", "github.com/acme/api"),
        clone("workspaces/smart-ide/repos/web", "github.com/acme/web"),
      ];
    return null;
  });

/** The project's window, its Notices view closed. */
const theView = () => (
  <PlaneView
    plane={PLANE}
    inFront
    kept
    slot={document.body}
    projects={NONE}
    pinnedProjects={NONE}
    window={windowDoes}
    onReport={() => undefined}
  />
);

/** Two frames on, where the view's own keyboard step runs. */
const framesOn = async () => {
  await act(() => new Promise((done) => requestAnimationFrame(() => done(undefined))));
  await act(() => new Promise((done) => requestAnimationFrame(() => done(undefined))));
};

/** The Trust this clone on web's Notice. */
const websTrust = () =>
  screen
    .getAllByRole("button", { name: "Trust this clone" })
    .find((one) => one.closest('[data-cause*="repos/web"]') !== null);

/** A result about smart-ide that names the control, as a refusal to save web does. */
const NOT_SAVED: Said = {
  from: "repo.save",
  refused: true,
  title: "repos/web was not saved",
  words: "Press Trust this clone on web's Notice, then save it again.",
  workspace: "smart-ide",
};

/** smart-ide's chip with its card up, as the title bar draws it, from the window's own hook. */
function ChipWithItsCard() {
  const does = useChipNotices({
    said: NOT_SAVED,
    kept: [{ id: 1, said: NOT_SAVED, plane: PLANE }],
    plane: PLANE,
    chips: [{ workspace: "smart-ide", look: "pinned" }],
    drop: () => undefined,
    onSettings: () => undefined,
  });
  const chip = useRef<HTMLButtonElement>(null);
  return (
    <>
      <button ref={chip}>smart-ide</button>
      <ChipCard
        workspace="smart-ide"
        named="smart-ide"
        notices={does.on.get("smart-ide") ?? []}
        anchor={chip}
        does={does}
      />
    </>
  );
}

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
    act(() => goToControl(TRUST, PLANE, "repos/web was not saved"));
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

/**
 * **Go to on a result's first surface** (R-333): a result is first seen on its workspace chip's
 * card (or the status bar's), not in the Notices view, so the card offers its Go to too, and it
 * reaches the control.
 */
describe("Go to from a workspace chip's card", () => {
  it("offers Go to Trust this clone, which opens the Notices view on web's", async () => {
    twoClones();
    render(
      <>
        {theView()}
        <ChipWithItsCard />
      </>,
    );
    await waitFor(() =>
      expect(document.querySelector('[data-cause^="clone-vouch:"]')).not.toBeNull(),
    );
    const card = screen.getByRole("region", { name: "Notices for smart-ide" });
    await userEvent.click(within(card).getByRole("button", { name: "Go to Trust this clone" }));
    await waitFor(() =>
      expect(
        document.querySelector<HTMLElement>('[role="tabpanel"][data-view="notices"]')?.hidden,
      ).toBe(false),
    );
    await framesOn();
    expect(document.activeElement).toBe(websTrust());
  });
});
