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
  live: true,
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

  it(`draws at most ${MOST_DRAWN}, and every one past them in show-more, none dropped (#5953)`, async () => {
    const many = Array.from({ length: MOST_DRAWN + 3 }, (_, n) => pin(`f${n}`, `Page ${n}`));
    const onOpen = vi.fn();
    render(<ArtifactGroup plane="p" workspace="alpha" pins={many} onOpen={onOpen} />);
    const group = screen.getByRole("toolbar", { name: "Artifacts" });
    const drawn = within(group).getAllByRole("button");
    expect(drawn.map((one) => one.getAttribute("aria-label"))).toEqual([
      ...many.slice(0, MOST_DRAWN).map((one) => one.name),
      "3 more artifacts",
    ]);
    await userEvent.click(screen.getByRole("button", { name: "3 more artifacts" }));
    const menu = await screen.findByRole("menu");
    expect(
      within(menu)
        .getAllByRole("menuitem")
        .map((row) => row.textContent),
    ).toEqual(["Page 8", "Page 9", "Page 10"]);
    await userEvent.click(within(menu).getByRole("menuitem", { name: "Page 9" }));
    expect(onOpen).toHaveBeenCalledWith(many[9]);
  });

  it("draws no show-more at eight or fewer", () => {
    render(<ArtifactGroup plane="p" workspace="alpha" pins={PINS} onOpen={() => {}} />);
    expect(screen.queryByRole("button", { name: /more artifacts/ })).toBeNull();
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

  it("moves a pin from its menu, for everyone, by writing the whole order through the core (#5953)", async () => {
    const asked: [string, unknown][] = [];
    mockIPC((cmd, args) => {
      asked.push([cmd, args]);
      return null;
    });
    const three = [...PINS, pin("notes", "Notes")];
    render(<ArtifactGroup plane="p" workspace="alpha" pins={three} onOpen={() => {}} />);
    const backlog = screen.getByRole("button", { name: "Backlog state" });
    backlog.focus();
    await userEvent.keyboard("{Shift>}{F10}{/Shift}");
    const menu = await screen.findByRole("menu");
    const rows = within(menu).getAllByRole("menuitem");
    expect(rows.map((row) => row.textContent)).toEqual([
      "Open",
      "Move up",
      "Move down",
      "Unpin from workspace…",
    ]);
    // The first cannot move up: said, not hidden.
    expect(within(menu).getByRole("menuitem", { name: "Move up" })).toHaveAttribute(
      "aria-disabled",
      "true",
    );
    await userEvent.click(within(menu).getByRole("menuitem", { name: "Move down" }));
    await waitFor(() =>
      expect(asked).toContainEqual([
        "artifact_reorder",
        { plane: "p", workspace: "alpha", ids: ["id-release", "id-backlog-state", "id-notes"] },
      ]),
    );
  });

  it("unpins a pin only once the person answers Unpin, saying who loses it (#5953)", async () => {
    const asked: [string, unknown][] = [];
    mockIPC((cmd, args) => {
      asked.push([cmd, args]);
      return null;
    });
    render(<ArtifactGroup plane="/x/acme" workspace="alpha" pins={PINS} onOpen={() => {}} />);
    screen.getByRole("button", { name: "Release" }).focus();
    await userEvent.keyboard("{Shift>}{F10}{/Shift}");
    await userEvent.click(
      within(await screen.findByRole("menu")).getByRole("menuitem", {
        name: "Unpin from workspace…",
      }),
    );
    const dialog = await screen.findByRole("alertdialog");
    expect(dialog).toHaveTextContent("Unpin Release from alpha");
    expect(dialog).toHaveTextContent("for everyone in acme");
    expect(dialog).toHaveTextContent("artifacts/release/");
    expect(asked.filter(([cmd]) => cmd === "artifact_unpin")).toEqual([]);
    await userEvent.click(within(dialog).getByRole("button", { name: "Unpin" }));
    await waitFor(() =>
      expect(asked).toContainEqual([
        "artifact_unpin",
        { plane: "/x/acme", workspace: "alpha", id: "id-release" },
      ]),
    );
    await waitFor(() => expect(screen.queryByRole("alertdialog")).toBeNull());
  });
});
