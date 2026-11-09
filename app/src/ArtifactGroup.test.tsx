// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { act, cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { ArtifactFresh, ArtifactPinShown } from "./bindings";
import { ArtifactGroup, MOST_DRAWN } from "./ArtifactGroup";

const pin = (folder: string, name: string, icon = "chart"): ArtifactPinShown => ({
  id: `id-${folder}`,
  folder,
  name,
  icon,
  missing: null,
  fresh: null,
});

const HOUR = 60 * 60 * 1000;

/** Updated `hours` ago, stale `staleIn` hours from now. */
const updated = (hours: number, staleIn: number): ArtifactFresh => ({
  at: Date.now() - hours * HOUR,
  stale_at: Date.now() + staleIn * HOUR,
  source: "written",
});

const PINS = [pin("backlog-state", "Backlog state"), pin("release", "Release", "rocket")];

afterEach(() => {
  cleanup();
  clearMocks();
  vi.useRealTimers();
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

  it("says in its tooltip when each artifact's data was last updated", async () => {
    const pins = [{ ...pin("backlog-state", "Backlog state"), fresh: updated(3, 21) }];
    render(<ArtifactGroup plane="p" workspace="alpha" pins={pins} onOpen={() => {}} />);
    await userEvent.tab();
    expect(screen.getByRole("button", { name: "Backlog state" })).toHaveFocus();
    expect(await screen.findByRole("tooltip")).toHaveTextContent(
      "Open Backlog state. Updated 3 hours ago",
    );
  });

  it("dims the icon of an artifact whose data is stale, and only that one", () => {
    const pins = [
      { ...pin("backlog-state", "Backlog state"), fresh: updated(30, -6) },
      { ...pin("release", "Release"), fresh: updated(1, 23) },
      pin("unknown", "Unknown"),
    ];
    render(<ArtifactGroup plane="p" workspace="alpha" pins={pins} onOpen={() => {}} />);
    expect(screen.getByRole("button", { name: "Backlog state" })).toHaveAttribute("data-dimmed");
    expect(screen.getByRole("button", { name: "Release" })).not.toHaveAttribute("data-dimmed");
    expect(screen.getByRole("button", { name: "Unknown" })).not.toHaveAttribute("data-dimmed");
  });

  it("dims an icon when its data turns stale, with nothing else changing", () => {
    vi.useFakeTimers();
    const pins = [{ ...pin("backlog-state", "Backlog state"), fresh: updated(23, 1) }];
    render(<ArtifactGroup plane="p" workspace="alpha" pins={pins} onOpen={() => {}} />);
    const icon = screen.getByRole("button", { name: "Backlog state" });
    expect(icon).not.toHaveAttribute("data-dimmed");
    act(() => {
      vi.advanceTimersByTime(HOUR + 1000);
    });
    expect(icon).toHaveAttribute("data-dimmed");
  });

  it("draws nothing, and throws nothing, for a core that answers no list at all", async () => {
    let asked = 0;
    mockIPC((cmd) => {
      if (cmd === "artifact_pins") asked += 1;
      return null;
    });
    render(
      <>
        <p>Beside it</p>
        <ArtifactGroup plane="p" workspace="alpha" onOpen={() => {}} />
      </>,
    );
    await waitFor(() => expect(asked).toBe(1));
    await new Promise((settle) => setTimeout(settle, 20));
    // A throw would have taken the whole tree down with it.
    expect(screen.getByText("Beside it")).toBeInTheDocument();
    expect(screen.queryByRole("toolbar")).toBeNull();
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
