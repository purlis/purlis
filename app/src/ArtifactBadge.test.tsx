// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { cleanup, render, screen, waitFor } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { afterEach, describe, expect, it } from "vitest";
import type { ArtifactFresh, ArtifactPinShown } from "./bindings";
import { ArtifactGroup } from "./ArtifactGroup";

const HOUR = 60 * 60 * 1000;

/** Updated `hours` ago, stale `staleIn` hours from now. */
const updated = (hours: number, staleIn: number): ArtifactFresh => ({
  at: Date.now() - hours * HOUR,
  stale_at: Date.now() + staleIn * HOUR,
  source: "declared",
});

const pin = (
  folder: string,
  name: string,
  badge: number | null,
  fresh: ArtifactFresh | null = updated(1, 23),
): ArtifactPinShown => ({
  id: `id-${folder}`,
  folder,
  name,
  icon: "chart",
  missing: null,
  live: true,
  too_large: null,
  fresh,
  badge,
});

/** The number drawn on the icon named `name`, or nothing. */
const drawnOn = (name: string) =>
  screen
    .getByRole("button", { name })
    .querySelector('[data-part="count"]')
    ?.getAttribute("data-count");

afterEach(() => {
  cleanup();
  clearMocks();
});

describe("an artifact icon's badge", () => {
  it("draws the number its data declares, told as the icon's description", () => {
    render(
      <ArtifactGroup
        plane="p"
        workspace="alpha"
        pins={[pin("backlog-state", "Backlog state", 12)]}
        onOpen={() => {}}
      />,
    );
    const icon = screen.getByRole("button", { name: "Backlog state" });
    expect(drawnOn("Backlog state")).toBe("12");
    expect(icon).toHaveAccessibleName("Backlog state");
    expect(icon).toHaveAccessibleDescription(/Its data shows 12$/);
  });

  it("draws nothing where the data declares no number, or a zero", () => {
    render(
      <ArtifactGroup
        plane="p"
        workspace="alpha"
        pins={[pin("none", "None", null), pin("zero", "Zero", 0)]}
        onOpen={() => {}}
      />,
    );
    expect(drawnOn("None")).toBeUndefined();
    expect(drawnOn("Zero")).toBeUndefined();
    expect(screen.getByRole("button", { name: "Zero" })).not.toHaveAccessibleDescription(
      /Its data shows/,
    );
  });

  it("draws a number past 99 as 99+, and says the whole of it", () => {
    render(
      <ArtifactGroup
        plane="p"
        workspace="alpha"
        pins={[pin("backlog-state", "Backlog state", 1204)]}
        onOpen={() => {}}
      />,
    );
    expect(drawnOn("Backlog state")).toBe("99+");
    expect(screen.getByRole("button", { name: "Backlog state" })).toHaveAccessibleDescription(
      /Its data shows 1204$/,
    );
  });

  it("dims with its icon when the data is stale, and says it is out of date", () => {
    render(
      <ArtifactGroup
        plane="p"
        workspace="alpha"
        pins={[
          pin("backlog-state", "Backlog state", 12, updated(30, -6)),
          pin("release", "Release", 3),
        ]}
        onOpen={() => {}}
      />,
    );
    const stale = screen.getByRole("button", { name: "Backlog state" });
    expect(stale).toHaveAttribute("data-dimmed");
    expect(stale).toHaveAccessibleDescription(/Its data shows 12, and is out of date$/);
    const fresh = screen.getByRole("button", { name: "Release" });
    expect(fresh).not.toHaveAttribute("data-dimmed");
    expect(fresh).toHaveAccessibleDescription(/Its data shows 3$/);
  });

  it("draws the number the core read with the pins", async () => {
    mockIPC((cmd) => (cmd === "artifact_pins" ? [pin("backlog-state", "Backlog state", 4)] : null));
    render(<ArtifactGroup plane="p" workspace="alpha" onOpen={() => {}} />);
    await waitFor(() => expect(drawnOn("Backlog state")).toBe("4"));
  });
});
