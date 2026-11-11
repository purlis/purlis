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
  live: true,
  too_large: null,
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
    render(
      <>
        <ArtifactGroup plane="p" workspace="alpha" pins={PINS} onOpen={() => {}} />
        <button type="button">After</button>
      </>,
    );
    const [first, second] = screen.getAllByRole("button");
    await userEvent.tab();
    expect(first).toHaveFocus();
    await userEvent.keyboard("{ArrowDown}");
    expect(second).toHaveFocus();
    // Tab leaves the group from any icon, and Shift+Tab comes back to the one it left.
    await userEvent.tab();
    expect(screen.getByRole("button", { name: "After" })).toHaveFocus();
    await userEvent.tab({ shift: true });
    expect(second).toHaveFocus();
  });

  it("takes the keyboard in on the artifact in front, from either side", async () => {
    const three = [...PINS, pin("oncall", "On call", "zap")];
    render(
      <>
        <button type="button">Before</button>
        <ArtifactGroup
          plane="p"
          workspace="alpha"
          pins={three}
          current="release"
          onOpen={() => {}}
        />
        <button type="button">After</button>
      </>,
    );
    const release = screen.getByRole("button", { name: "Release" });
    await userEvent.tab();
    await userEvent.tab();
    expect(release).toHaveFocus();
    await userEvent.keyboard("{ArrowDown}");
    await userEvent.tab();
    expect(screen.getByRole("button", { name: "After" })).toHaveFocus();
    await userEvent.tab({ shift: true });
    expect(release).toHaveFocus();
  });

  it("leaves the focus on the artifact clicked, not the one in front (#6120)", async () => {
    // An engine that focuses a button on its press (Chromium, Firefox; not WebKit) brings the
    // focus in from outside the group: that is a click, never the keyboard coming back in.
    render(
      <>
        <button type="button">Before</button>
        <ArtifactGroup
          plane="p"
          workspace="alpha"
          pins={PINS}
          current="release"
          onOpen={() => {}}
        />
      </>,
    );
    screen.getByRole("button", { name: "Before" }).focus();
    const backlog = screen.getByRole("button", { name: "Backlog state" });
    await userEvent.click(backlog);
    expect(backlog).toHaveFocus();
  });

  it("reaches its first and last icon with Home and End", async () => {
    const three = [...PINS, pin("oncall", "On call", "zap")];
    render(<ArtifactGroup plane="p" workspace="alpha" pins={three} onOpen={() => {}} />);
    await userEvent.tab();
    await userEvent.keyboard("{End}");
    expect(screen.getByRole("button", { name: "On call" })).toHaveFocus();
    await userEvent.keyboard("{Home}");
    expect(screen.getByRole("button", { name: "Backlog state" })).toHaveFocus();
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
        { plane: "/x/acme", workspace: "alpha", folder: "release" },
      ]),
    );
    await waitFor(() => expect(screen.queryByRole("alertdialog")).toBeNull());
  });

  it("draws nothing when the core answers no pins at all", async () => {
    const asked: unknown[] = [];
    mockIPC((cmd, args) => {
      if (cmd === "artifact_pins") asked.push(args);
      return null;
    });
    render(
      <>
        <p>Beside it</p>
        <ArtifactGroup plane="p" workspace="alpha" onOpen={() => {}} />
      </>,
    );
    await waitFor(() => expect(asked).toHaveLength(1));
    // Let the answer land: a throw while drawing it takes the whole tree down with it.
    await act(async () => {});
    expect(screen.getByText("Beside it")).toBeInTheDocument();
    expect(screen.queryByRole("toolbar")).toBeNull();
  });
});
