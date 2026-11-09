// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { ArtifactPinShown } from "./bindings";
import { ArtifactGroup, MOST_DRAWN } from "./ArtifactGroup";

const pin = (folder: string, name: string, icon = "chart"): ArtifactPinShown => ({
  id: `id-${folder}`,
  folder,
  name,
  icon,
  missing: null,
  too_large: null,
});

const PINS = [pin("backlog-state", "Backlog state"), pin("release", "Release", "rocket")];

afterEach(() => {
  cleanup();
  clearMocks();
});

describe("the activity bar's artifacts", () => {
  it("draws each pin as a named icon in a group of its own, after a divider", () => {
    const { container } = render(
      <ArtifactGroup plane="p" workspace="alpha" pins={PINS} onOpen={() => {}} />,
    );
    const group = screen.getByRole("toolbar", { name: "Artifacts" });
    expect(
      within(group)
        .getAllByRole("button")
        .map((one) => one.getAttribute("aria-label")),
    ).toEqual(["Backlog state", "Release"]);
    const divider = container.querySelector('[data-scope="separator"]');
    expect(divider).not.toBeNull();
    // The divider comes first, then the group.
    expect(
      (divider?.compareDocumentPosition(group) ?? 0) & Node.DOCUMENT_POSITION_FOLLOWING,
    ).toBeTruthy();
  });

  it("opens the pressed artifact, and marks the one whose tab is in front", async () => {
    const onOpen = vi.fn();
    render(
      <ArtifactGroup plane="p" workspace="alpha" pins={PINS} current="release" onOpen={onOpen} />,
    );
    const release = screen.getByRole("button", { name: "Release" });
    expect(release).toHaveAttribute("aria-current", "page");
    expect(screen.getByRole("button", { name: "Backlog state" })).not.toHaveAttribute(
      "aria-current",
    );
    await userEvent.click(screen.getByRole("button", { name: "Backlog state" }));
    expect(onOpen).toHaveBeenCalledWith(PINS[0]);
  });

  it("is walked by the arrows with one Tab stop", async () => {
    render(<ArtifactGroup plane="p" workspace="alpha" pins={PINS} onOpen={() => {}} />);
    const [first, second] = screen.getAllByRole("button");
    await userEvent.tab();
    expect(first).toHaveFocus();
    expect(second).toHaveAttribute("tabindex", "-1");
    await userEvent.keyboard("{ArrowDown}");
    expect(second).toHaveFocus();
  });

  it("greys a pin that is missing or too large, and its tip says why", () => {
    const over = "'release' is 9.0 MB, over the 8.0 MB an artifact may be, so none of it is shown";
    const gone = "there is no folder 'backlog-state' in artifacts/";
    const { container } = render(
      <ArtifactGroup
        plane="p"
        workspace="alpha"
        pins={[
          { ...pin("backlog-state", "Backlog state"), missing: gone },
          { ...pin("release", "Release"), too_large: over },
          pin("fine", "Fine"),
        ]}
        onOpen={() => {}}
      />,
    );
    const marked = (folder: string) =>
      container.querySelector(`[data-artifact="${folder}"]`)?.getAttribute("data-missing");
    expect(marked("backlog-state")).toBe("true");
    expect(marked("release")).toBe("true");
    expect(marked("fine")).toBeNull();
  });

  it(`draws at most ${MOST_DRAWN}`, () => {
    const many = Array.from({ length: MOST_DRAWN + 3 }, (_, n) => pin(`f${n}`, `Page ${n}`));
    render(<ArtifactGroup plane="p" workspace="alpha" pins={many} onOpen={() => {}} />);
    expect(screen.getAllByRole("button")).toHaveLength(MOST_DRAWN);
  });

  it("draws nothing, not even its divider, with no pins or outside every workspace", () => {
    const none = render(<ArtifactGroup plane="p" workspace="alpha" pins={[]} onOpen={() => {}} />);
    expect(none.container).toBeEmptyDOMElement();
    none.unmount();
    const outside = render(
      <ArtifactGroup plane="p" workspace={undefined} pins={PINS} onOpen={() => {}} />,
    );
    expect(outside.container).toBeEmptyDOMElement();
  });

  it("asks the core for the workspace's pins", async () => {
    const asked: unknown[] = [];
    mockIPC((cmd, args) => {
      if (cmd !== "artifact_pins") return null;
      asked.push(args);
      return PINS;
    });
    render(<ArtifactGroup plane="p" workspace="alpha" onOpen={() => {}} />);
    await waitFor(() => expect(screen.getAllByRole("button")).toHaveLength(2));
    expect(asked).toEqual([{ plane: "p", workspace: "alpha" }]);
  });
});
