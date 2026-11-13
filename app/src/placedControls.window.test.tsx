// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, render, screen, waitFor } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { PlaneSaving } from "./bindings";
import { PlaneView } from "./PlaneView";
import type { WindowDoing } from "./planeReport";
import { PLACED_CONTROLS, goToPlaced, type PlacedControl } from "./placedControls";
import { need, skill, studio } from "./skillsListing.testkit";

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

/** A save that stopped on a conflict: the Saving view draws Resolve. */
const BLOCKED = {
  stage: "blocked",
  changed: [],
  ahead: 1,
  pr: null,
  request: "pull request",
  blocked: null,
  branch: "main",
  pushes: true,
  behind: 3,
  pushFailed: null,
  live: [],
  conflicts: [],
  conflictKeys: {},
  notice: null,
  mode: null,
  modeFrom: "default",
  journal: [],
  toResolve: "Press Resolve.",
} as PlaneSaving;

/** A few frames on, past the reach's own hold. */
const framesOn = async (frames: number) => {
  for (let at = 0; at < frames; at++)
    await act(() => new Promise((done) => requestAnimationFrame(() => done(undefined))));
};

/**
 * **Go to a control in a view, through the window** (R-357): the project's own window opens the
 * view the message names, and the keyboard ends on the control and stays there.
 *
 * `giveViewTheKeyboard` runs only for a side view (Search, Explorer, Notices), not on these
 * paths, and nothing else in jsdom takes the keyboard back as a view tab opens: these pass with
 * `reach` focusing once, so they hold the path, not the reach's hold against a view's own step.
 */
describe("Go to a control a message names in a view, through the window", () => {
  it("opens the Saving view with the keyboard on Resolve, and it stays there", async () => {
    mockIPC((cmd) => (cmd === "plane_saving" ? BLOCKED : null));
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
    const resolve = PLACED_CONTROLS.find((one) => one.words === "Resolve") as PlacedControl;

    act(() =>
      goToPlaced(resolve, PLANE, "press Resolve in the Saving view to settle it", () => undefined),
    );

    await waitFor(() => expect(screen.getByRole("button", { name: "Resolve" })).toHaveFocus());
    await framesOn(6);
    expect(screen.getByRole("button", { name: "Resolve" })).toHaveFocus();
  });

  it("opens the Skill studio's Needs you with the keyboard on Accept the folders, and it stays there", async () => {
    const needs = [
      need("grilling"),
      need("review:project-folders", {
        title: "The project's folders",
        tag: null,
        reason: "Purlis can't confirm which of this project's folders you accepted before.",
        fix: { kind: "accept-folders", folders: ["workspaces/ops"] },
        fix_label: "Accept the folders",
        fix_does: "Their skills reach chats from then on.",
      }),
    ];
    mockIPC((cmd) => {
      if (cmd === "skill_studio") return studio([skill("deploy")], { needs });
      if (cmd === "skill_updates") return { offers: [], unconfirmed: [], unasked: [] };
      if (cmd === "skill_drafts" || cmd === "skills_deleted") return [];
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
    const accept = PLACED_CONTROLS.find(
      (one) => one.words === "Accept the folders",
    ) as PlacedControl;

    act(() =>
      goToPlaced(
        accept,
        PLANE,
        "press Accept the folders in the Skill studio's Needs you",
        () => undefined,
      ),
    );

    await waitFor(() =>
      expect(screen.getByRole("button", { name: "Accept the folders" })).toHaveFocus(),
    );
    await framesOn(6);
    expect(screen.getByRole("button", { name: "Accept the folders" })).toHaveFocus();
  });
});
