// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **A view tab's top bar** (#3667): back and forward, unavailable with a reason at their ends;
 * ⌘[ and ⌘] (Alt+← and Alt+→ off a Mac) and the mouse's own buttons anywhere in the view; the
 * path, a list named "Path" whose last name is where the person is.
 */
import { afterEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { viewHistoryKeyOf } from "../../keys/viewKeys";
import { ViewBar, type Crumb } from "./ViewBar";
import { ViewFrame, ViewListHead } from "./ViewFrame";
import { forgetViewHistory, useViewHistory } from "./viewHistory";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
  forgetViewHistory("studio");
});

/** Each place is a path of page names. */
type Place = string[];

function Studio() {
  const history = useViewHistory<Place>("studio", ["Library"]);
  const path: Crumb[] = history.at.map((label) => ({ id: label, label }));
  return (
    <ViewFrame
      label="Skill studio pages"
      pages={[{ id: "library", label: "Library" }]}
      page="library"
      onPage={() => undefined}
      history={history}
      bar={
        <ViewBar
          history={history}
          path={path}
          onGo={(_, at) => history.go(history.at.slice(0, at + 1))}
        />
      }
      list={
        <>
          <ViewListHead title={history.at.at(-1)} />
          {["deploy", "Grades", "New grade"].map((next) => (
            <button key={next} type="button" onClick={() => history.go([...history.at, next])}>
              {`Open ${next}`}
            </button>
          ))}
        </>
      }
    />
  );
}

const back = () => screen.getByRole("button", { name: "Back" });
const forward = () => screen.getByRole("button", { name: "Forward" });
const path = () => screen.getByRole("list", { name: "Path" });
const here = () => path().querySelector('[aria-current="page"]')?.textContent;

describe("back and forward", () => {
  it("are unavailable at their ends, and say why", () => {
    render(<Studio />);
    expect(back()).toHaveAttribute("aria-disabled", "true");
    expect(forward()).toHaveAttribute("aria-disabled", "true");
    act(() => fireEvent.click(screen.getByRole("button", { name: "Open deploy" })));
    expect(back()).not.toHaveAttribute("aria-disabled");
    act(() => fireEvent.click(back()));
    expect(here()).toBe("Library");
    expect(back()).toHaveAttribute("aria-disabled", "true");
    expect(forward()).not.toHaveAttribute("aria-disabled");
  });

  it("are pressed by ⌘[ and ⌘] anywhere in the view, and by the mouse's own buttons", () => {
    vi.spyOn(navigator, "platform", "get").mockReturnValue("MacIntel");
    render(<Studio />);
    const open = (name: string) =>
      act(() => fireEvent.click(screen.getByRole("button", { name: `Open ${name}` })));
    open("deploy");
    open("Grades");
    expect(here()).toBe("Grades");
    const inside = screen.getByRole("button", { name: "Open deploy" });
    act(() => fireEvent.keyDown(inside, { key: "[", metaKey: true }));
    expect(here()).toBe("deploy");
    act(() => fireEvent.keyDown(inside, { key: "]", metaKey: true }));
    expect(here()).toBe("Grades");
    act(() => fireEvent.mouseUp(inside, { button: 3 }));
    expect(here()).toBe("deploy");
    act(() => fireEvent.mouseUp(inside, { button: 4 }));
    expect(here()).toBe("Grades");
  });

  it("are pressed by Alt+← and Alt+→ off a Mac", () => {
    vi.spyOn(navigator, "platform", "get").mockReturnValue("Linux x86_64");
    render(<Studio />);
    act(() => fireEvent.click(screen.getByRole("button", { name: "Open deploy" })));
    const inside = screen.getByRole("button", { name: "Open Grades" });
    act(() => fireEvent.keyDown(inside, { key: "ArrowLeft", altKey: true }));
    expect(here()).toBe("Library");
    act(() => fireEvent.keyDown(inside, { key: "ArrowRight", altKey: true }));
    expect(here()).toBe("deploy");
    // ⌘[ is a Mac's alone.
    act(() => fireEvent.keyDown(inside, { key: "[", ctrlKey: true }));
    expect(here()).toBe("deploy");
  });
});

describe("the keys", () => {
  const key = (init: KeyboardEventInit) => new KeyboardEvent("keydown", init);
  it("are ⌘[ and ⌘] on a Mac, and nothing with another modifier", () => {
    expect(viewHistoryKeyOf(key({ key: "[", metaKey: true }), true)).toBe("back");
    expect(viewHistoryKeyOf(key({ key: "]", metaKey: true }), true)).toBe("forward");
    expect(viewHistoryKeyOf(key({ key: "[", metaKey: true, shiftKey: true }), true)).toBe(
      undefined,
    );
    expect(viewHistoryKeyOf(key({ key: "[", ctrlKey: true }), true)).toBe(undefined);
  });

  it("never take another of the window's keys on a layout with the brackets elsewhere", () => {
    // Dvorak: the keys in the brackets' places type `/` and `=`, the shortcuts' and the size's.
    expect(viewHistoryKeyOf(key({ key: "/", code: "BracketLeft", metaKey: true }), true)).toBe(
      undefined,
    );
    expect(viewHistoryKeyOf(key({ key: "=", code: "BracketRight", metaKey: true }), true)).toBe(
      undefined,
    );
    // A layout whose `[` needs Option: the key in its place, typing something else.
    expect(viewHistoryKeyOf(key({ key: "ü", code: "BracketLeft", metaKey: true }), true)).toBe(
      undefined,
    );
    expect(viewHistoryKeyOf(key({ key: "*", code: "BracketRight", metaKey: true }), true)).toBe(
      "forward",
    );
  });

  it("are Alt+← and Alt+→ off a Mac, never Ctrl+[, which is a terminal's Escape", () => {
    expect(viewHistoryKeyOf(key({ key: "ArrowLeft", altKey: true }), false)).toBe("back");
    expect(viewHistoryKeyOf(key({ key: "ArrowRight", altKey: true }), false)).toBe("forward");
    expect(viewHistoryKeyOf(key({ key: "[", ctrlKey: true }), false)).toBe(undefined);
    expect(viewHistoryKeyOf(key({ key: "ArrowLeft" }), false)).toBe(undefined);
  });
});

describe("the path", () => {
  it("is a list named Path: each name before the last goes there, the last is where they are", () => {
    render(<Studio />);
    for (const name of ["deploy", "Grades", "New grade"])
      act(() => fireEvent.click(screen.getByRole("button", { name: `Open ${name}` })));
    expect(path().querySelectorAll("li")).toHaveLength(4);
    expect(here()).toBe("New grade");
    // The last is a name, never a button.
    expect(
      [...path().querySelectorAll("button")].map((one) => one.textContent),
    ).toEqual(["Library", "deploy", "Grades"]);
    act(() => fireEvent.click(screen.getByRole("button", { name: "deploy" })));
    expect(here()).toBe("deploy");
    // A crumb is a step of its own: back returns to where the person was.
    act(() => fireEvent.click(back()));
    expect(here()).toBe("New grade");
  });
});
