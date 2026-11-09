// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useRef } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { ProjectRail } from "./ProjectRail";
import { forgetTheSlide } from "./railSlide";
import { catalogued, type Offer, type Project } from "./actions";

/**
 * **The project rail's keyboard, focus and names** (#3134): what a person on the keyboard or a
 * screen reader gets from the rail, held across its move onto the tabs family. The projects are
 * one vertical tablist, one Tab stop on the project in front; Up and Down, Home and End walk
 * it, wrapping at its ends; moving switches nothing, Enter switches and Delete closes; the rail's own buttons (open a
 * project, hide) are stops of their own after the tabs, which the arrows pass over; and coming
 * back in lands on the project in front.
 */

const PROJECTS: Project[] = [
  { plane: "/p/alpha", name: "alpha" },
  { plane: "/p/beta-two", name: "beta-two" },
  { plane: "/p/gamma", name: "gamma" },
];

const offer = (id: string): Offer =>
  ({ id, title: id, available: true, reason: "" }) as unknown as Offer;

beforeEach(() => {
  forgetTheSlide();
  vi.stubGlobal(
    "ResizeObserver",
    class {
      observe() {}
      unobserve() {}
      disconnect() {}
    },
  );
});
afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
  forgetTheSlide();
});

function Window({ onPress = () => {} }: { onPress?: (offer: Offer) => void }) {
  const edge = useRef<HTMLDivElement>(null);
  return (
    <>
      <button type="button">before</button>
      <ProjectRail
        projects={PROJECTS}
        inFront="/p/beta-two"
        rows={
          {
            open: offer("project.open"),
            switchTo: PROJECTS.map((one) => offer(`project.switch:${one.plane}`)),
            close: PROJECTS.map((one) => offer(`project.close:${one.plane}`)),
          } as never
        }
        // A menu on every project, so each tab is a context menu's trigger as in the window.
        offers={catalogued(PROJECTS.map((one) => offer(`project.select:${one.plane}`)))}
        onPress={onPress}
        waitingIn={(project) => (project.name === "gamma" ? 2 : 0)}
        unsavedIn={() => undefined}
        onDrop={() => {}}
        lettingGo={0}
        mode="shown"
        onMode={() => {}}
        edge={edge}
        nameOf={(plane) => plane}
      />
      <div ref={edge} />
    </>
  );
}

const rail = () => screen.getByRole("tablist", { name: "Projects" });
const tab = (name: string) => screen.getByRole("tab", { name });

/** Into the rail from the button before it, as Tab does. */
async function comeIn() {
  await userEvent.click(screen.getByRole("button", { name: "before" }));
  await userEvent.tab();
}

describe("the project rail's keyboard (#3134)", () => {
  it("is one vertical tablist of the projects, in order, the one in front selected", () => {
    render(<Window />);
    expect(rail()).toHaveAttribute("aria-orientation", "vertical");
    const tabs = screen.getAllByRole("tab");
    expect(tabs.map((one) => one.getAttribute("aria-label"))).toEqual([
      "alpha",
      "beta-two",
      "gamma",
    ]);
    expect(tab("beta-two")).toHaveAttribute("aria-selected", "true");
    expect(tab("alpha")).toHaveAttribute("aria-selected", "false");
    // What waits in a project is said as its description, beside the drag's instructions.
    expect(tab("gamma")).toHaveAccessibleDescription(/2 waiting/);
  });

  it("owns its tabs by id, in order, and nothing else", () => {
    render(<Window />);
    const owned = (rail().getAttribute("aria-owns") ?? "").split(" ");
    expect(owned).toEqual(screen.getAllByRole("tab").map((one) => one.id));
    expect(within(rail()).queryAllByRole("button")).toEqual([]);
  });

  it("is one Tab stop, on the project in front, then its own buttons", async () => {
    render(<Window />);
    await comeIn();
    expect(tab("beta-two")).toHaveFocus();
    await userEvent.tab();
    expect(screen.getByRole("button", { name: "project.open" })).toHaveFocus();
  });

  it("walks with Up and Down, Home and End, and switches nothing", async () => {
    const onPress = vi.fn();
    render(<Window onPress={onPress} />);
    await comeIn();
    await userEvent.keyboard("{ArrowDown}");
    expect(tab("gamma")).toHaveFocus();
    await userEvent.keyboard("{ArrowUp}{ArrowUp}");
    expect(tab("alpha")).toHaveFocus();
    await userEvent.keyboard("{End}");
    expect(tab("gamma")).toHaveFocus();
    await userEvent.keyboard("{Home}");
    expect(tab("alpha")).toHaveFocus();
    expect(onPress).not.toHaveBeenCalled();
  });

  // Radix's roving focus stopped at either end; the tabs family's (React Aria's) wraps, as the
  // WAI-ARIA tabs pattern does (#3134).
  it("wraps at its ends, as the tabs pattern does", async () => {
    render(<Window />);
    await comeIn();
    await userEvent.keyboard("{End}{ArrowDown}");
    expect(tab("alpha")).toHaveFocus();
    await userEvent.keyboard("{ArrowUp}");
    expect(tab("gamma")).toHaveFocus();
  });

  it("switches with Enter and closes with Delete, on the project the keyboard is on", async () => {
    const onPress = vi.fn();
    render(<Window onPress={onPress} />);
    await comeIn();
    await userEvent.keyboard("{ArrowUp}{Enter}");
    await userEvent.keyboard("{End}{Delete}");
    expect(onPress.mock.calls.map(([one]) => (one as Offer).id)).toEqual([
      "project.switch:/p/alpha",
      "project.close:/p/gamma",
    ]);
  });

  // Radix's roving item chained a project's own keys with its menu's; the tabs family's tab
  // has to hand the menu's on too, or Shift+F10 on a project opens nothing (#3134 review).
  it("opens a project's menu on Shift+F10 and on the menu key, with the keyboard on it", async () => {
    render(<Window />);
    await comeIn();
    fireEvent.keyDown(tab("beta-two"), { key: "F10", shiftKey: true });
    expect(await screen.findByRole("menu")).toBeTruthy();
    await userEvent.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("menu")).toBeNull());
    fireEvent.keyDown(tab("beta-two"), { key: "ContextMenu" });
    expect(await screen.findByRole("menu")).toBeTruthy();
  });

  it("comes back in on the project in front after the keyboard walked and left", async () => {
    render(<Window />);
    await comeIn();
    await userEvent.keyboard("{ArrowUp}");
    await userEvent.tab();
    expect(screen.getByRole("button", { name: "project.open" })).toHaveFocus();
    await userEvent.tab({ shift: true });
    expect(tab("beta-two")).toHaveFocus();
  });
});
