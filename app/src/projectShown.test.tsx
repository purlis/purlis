// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useState } from "react";
import { createPortal } from "react-dom";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, fireEvent, render, renderHook, screen } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { SETTINGS, usePlaneChanged } from "./planeChanged";
import {
  AsLastShown,
  drawnOverTheWindow,
  InTheWindow,
  KEPT_DRAWN,
  keptOf,
  OnlyInFront,
  ProjectShownProvider,
  SCROLLED_MOST,
  scrolledPlacesHeld,
  useProjectHost,
} from "./projectShown";
import { RemovePersona } from "./RemovePersona";
import { useWhileShown } from "./whileShown";
import { useWindowFocused } from "./windowFocused";

/**
 * **A project that is not in front stays drawn, off the window** (`projectShown.tsx`): the
 * parts it is made of, each on its own. `Projects.test.tsx` holds them together in the real
 * window, and `projectSwitch.cost.test.tsx` holds what a switch costs.
 */

afterEach(() => {
  cleanup();
  clearMocks();
  vi.useRealTimers();
});

/** A project's view the way `PlaneView` draws one: into a host, in the slot while shown. */
function View({ shown, slot, name }: { shown: boolean; slot: HTMLElement; name: string }) {
  const host = useProjectHost();
  return createPortal(
    <ProjectShownProvider value={shown}>
      <InTheWindow host={host} slot={slot} shown={shown} onLeftCovered={() => {}} />
      <AsLastShown>
        <Drawn name={name} />
      </AsLastShown>
    </ProjectShownProvider>,
    host,
  );
}

const draws = new Map<string, number>();
const mounts = new Map<string, number>();

function Drawn({ name }: { name: string }) {
  // By the project, whatever it says now: `two3` is the project `two`, drawn once more.
  const project = name.replace(/\d+$/, "");
  draws.set(project, (draws.get(project) ?? 0) + 1);
  useState(() => mounts.set(project, (mounts.get(project) ?? 0) + 1));
  return <p data-testid="view">{name}</p>;
}

beforeEach(() => {
  draws.clear();
  mounts.clear();
});

describe("a project's view and the window", () => {
  function two(front: "one" | "two", slot: HTMLElement, tick = 0) {
    return (
      <>
        <View shown={front === "one"} slot={slot} name="one" key="one" />
        <View shown={front === "two"} slot={slot} name={`two${tick || ""}`} key="two" />
      </>
    );
  }

  it("is in the window only while its project is in front", () => {
    const slot = document.body.appendChild(document.createElement("div"));
    const { rerender } = render(two("one", slot));
    expect(screen.getAllByTestId("view").map((one) => one.textContent)).toEqual(["one"]);

    rerender(two("two", slot));

    // One view in the document, so no query of the window finds two tab strips or two of an id.
    expect(screen.getAllByTestId("view").map((one) => one.textContent)).toEqual(["two"]);
    slot.remove();
  });

  it("keeps what it drew, so coming back mounts nothing", () => {
    const slot = document.body.appendChild(document.createElement("div"));
    const { rerender } = render(two("one", slot));
    const first = screen.getByTestId("view");

    rerender(two("two", slot));
    rerender(two("one", slot));

    // The very element it drew before, put back.
    expect(screen.getByTestId("view")).toBe(first);
    expect(mounts.get("one")).toBe(1);
    slot.remove();
  });

  it("comes back scrolled to where it was left", () => {
    const slot = document.body.appendChild(document.createElement("div"));
    const { rerender } = render(two("one", slot));
    const list = screen.getByTestId("view");
    list.scrollTop = 240;
    list.scrollLeft = 12;
    fireEvent.scroll(list);

    rerender(two("two", slot));
    // What an engine does to an element taken out of the document; jsdom, which lays nothing
    // out, leaves the numbers where they were.
    list.scrollTop = 0;
    list.scrollLeft = 0;

    rerender(two("one", slot));
    expect([list.scrollTop, list.scrollLeft]).toEqual([240, 12]);
    slot.remove();
  });

  it("lets go of the places of what has since gone, once they pile up, while it is still in front", () => {
    // A view in front for a day scrolls rows that are long gone. Each place holds its element,
    // so they are let go past so many, and not only when the project next leaves the front.
    const slot = document.body.appendChild(document.createElement("div"));
    render(two("one", slot));
    const host = screen.getByTestId("view").parentElement as HTMLElement;
    const scrolled = (still: boolean) => {
      const row = host.appendChild(document.createElement("div"));
      row.scrollTop = 10;
      fireEvent.scroll(row);
      if (!still) row.remove();
      return row;
    };

    // Rows scrolled and then gone, up to the most held: every one is still held.
    for (let n = 0; n < SCROLLED_MOST; n++) scrolled(false);
    expect(scrolledPlacesHeld(host)).toBe(SCROLLED_MOST);

    // One more, still drawn: the ones that went are let go, and it is kept.
    const here = scrolled(true);
    expect(scrolledPlacesHeld(host)).toBe(1);

    // And what is still drawn is never let go, however many there are.
    for (let n = 0; n < SCROLLED_MOST + 5; n++) scrolled(true);
    expect(scrolledPlacesHeld(host)).toBe(SCROLLED_MOST + 6);
    expect(host.contains(here)).toBe(true);
    slot.remove();
  });

  it("is not drawn again while it is off the window, and takes what changed as it comes back", () => {
    const slot = document.body.appendChild(document.createElement("div"));
    const { rerender } = render(two("one", slot));
    const before = draws.get("two");

    // The project behind hears something three times over.
    rerender(two("one", slot, 1));
    rerender(two("one", slot, 2));
    rerender(two("one", slot, 3));
    expect(draws.get("two")).toBe(before);

    rerender(two("two", slot, 3));
    expect(screen.getByTestId("view")).toHaveTextContent("two3");
    slot.remove();
  });
});

describe("what is asked over the whole window", () => {
  it("is put away with its project, and is there again when the project is", () => {
    const asked = (shown: boolean) => (
      <ProjectShownProvider value={shown}>
        <OnlyInFront>
          <div role="dialog">Delete workspace alpha?</div>
        </OnlyInFront>
      </ProjectShownProvider>
    );
    const { rerender } = render(asked(true));
    expect(screen.getByRole("dialog")).toBeInTheDocument();

    rerender(asked(false));
    expect(screen.queryByRole("dialog")).toBeNull();

    rerender(asked(true));
    expect(screen.getByRole("dialog")).toBeInTheDocument();
  });
});

describe("the projects that may stay drawn", () => {
  it("are the ones the person was in last, among those the window still holds", () => {
    // Three closed projects between two open ones took the room of one.
    expect([...keptOf(["a", "x", "y", "z", "b"], ["a", "b"])]).toEqual(["a", "b"]);
  });

  it("are no more than so many, the last ones first", () => {
    const open = ["a", "b", "c", "d", "e", "f"];
    expect([...keptOf(["f", "e", "d", "c", "b", "a"], open)]).toEqual(["f", "e", "d", "c"]);
    expect(KEPT_DRAWN).toBe(4);
  });
});

describe("what is drawn over the whole window, as a project leaves the front", () => {
  /** A window's own element with a place for the project in front, as `App` draws one. */
  function aWindow() {
    const own = document.body.appendChild(document.createElement("main"));
    const slot = own.appendChild(document.createElement("div"));
    return { own, slot };
  }

  it("is the confirm a part of a view opened, drawn outside the window's element", () => {
    const { own, slot } = aWindow();
    render(
      <RemovePersona persona="steward" deleting={false} onDelete={() => {}} onCancel={() => {}} />,
    );
    expect(screen.getByRole("alertdialog", { name: "Delete persona steward?" })).toBeTruthy();
    expect(drawnOverTheWindow(slot)).toBe(true);
    own.remove();
  });

  it("is not what is drawn inside the window, nor a tooltip, nor the palette a switch is made in", () => {
    const { own, slot } = aWindow();
    // A list inside the view, as the Chats list is.
    own.appendChild(document.createElement("ul")).setAttribute("role", "listbox");
    const tip = document.body.appendChild(document.createElement("div"));
    tip.setAttribute("role", "tooltip");
    // The palette as the dialog family draws it: its scrim, and its root with its own hook.
    const palette = document.body.appendChild(document.createElement("div"));
    Object.assign(palette.dataset, { scope: "dialog", part: "scrim" });
    const root = palette.appendChild(document.createElement("section"));
    Object.assign(root.dataset, { scope: "dialog", part: "root" });
    root.className = "palette";
    root.setAttribute("role", "dialog");
    root.appendChild(document.createElement("ul")).setAttribute("role", "listbox");
    expect(drawnOverTheWindow(slot)).toBe(false);

    // And a menu left open is.
    const menu = document.body.appendChild(document.createElement("div"));
    menu.setAttribute("role", "menu");
    expect(drawnOverTheWindow(slot)).toBe(true);
    for (const one of [own, tip, palette, menu]) one.remove();
  });

  it("is said by the view once the press that made it leave is over, once, and only then", async () => {
    const { own, slot } = aWindow();
    const covered = vi.fn();
    const drawn = (shown: boolean) => <Leaving shown={shown} slot={slot} onLeftCovered={covered} />;
    const { rerender } = render(drawn(true));
    const asked = document.body.appendChild(document.createElement("div"));
    asked.setAttribute("role", "alertdialog");
    expect(covered).not.toHaveBeenCalled();

    rerender(drawn(false));
    // Not in the middle of the press: a menu closes after the row that switched.
    expect(covered).not.toHaveBeenCalled();
    await act(async () => {});
    expect(covered).toHaveBeenCalledTimes(1);

    // Coming back, and leaving with nothing asked, says nothing.
    asked.remove();
    rerender(drawn(true));
    rerender(drawn(false));
    await act(async () => {});
    expect(covered).toHaveBeenCalledTimes(1);
    own.remove();
  });
});

/** A view's place in the window alone, for what it says as its project leaves the front. */
function Leaving(props: { shown: boolean; slot: HTMLElement; onLeftCovered: () => void }) {
  const host = useProjectHost();
  return <InTheWindow host={host} {...props} />;
}

/** A hook run inside a project that is, or is not, the one in front. */
function inAProject<T>(hook: () => T, shown: boolean) {
  let now = shown;
  const drawn = renderHook(hook, {
    wrapper: ({ children }) => <ProjectShownProvider value={now}>{children}</ProjectShownProvider>,
  });
  return {
    ...drawn,
    show(next: boolean) {
      now = next;
      drawn.rerender();
    },
  };
}

describe("a read made when the window is focused again", () => {
  beforeEach(() => vi.useFakeTimers());
  const focus = () => act(() => void window.dispatchEvent(new Event("focus")));
  /** The frame the project came back in is painted, and what waited for that has run. */
  const painted = () => act(() => void vi.advanceTimersByTime(50));

  it("is made for the project in front, and for the window's own", () => {
    const front = inAProject(useWindowFocused, true);
    const windows = renderHook(useWindowFocused);
    focus();
    expect(front.result.current).toBe(1);
    expect(windows.result.current).toBe(1);
  });

  it("is not made for a project behind, however often the window is focused", () => {
    const behind = inAProject(useWindowFocused, true);
    behind.show(false);
    focus();
    focus();
    expect(behind.result.current).toBe(0);
  });

  it("is made once as the project comes back, after it is painted and not before", () => {
    const behind = inAProject(useWindowFocused, true);
    behind.show(false);
    focus();

    behind.show(true);
    // Nothing a switch waits on: the project is on screen first.
    expect(behind.result.current).toBe(0);
    painted();
    expect(behind.result.current).toBe(1);
  });

  it("is not made at the mount, which reads for itself", () => {
    const front = inAProject(useWindowFocused, true);
    painted();
    expect(front.result.current).toBe(0);
  });
});

describe("a read made on a beat", () => {
  beforeEach(() => vi.useFakeTimers());

  it("stops for a project behind, and is made once as it comes back, after it is painted", () => {
    const read = vi.fn();
    const behind = inAProject(() => useWhileShown(1000, read), true);
    vi.advanceTimersByTime(1000);
    expect(read).toHaveBeenCalledTimes(1);

    behind.show(false);
    vi.advanceTimersByTime(5000);
    expect(read).toHaveBeenCalledTimes(1);

    behind.show(true);
    expect(read).toHaveBeenCalledTimes(1);
    vi.advanceTimersByTime(50);
    expect(read).toHaveBeenCalledTimes(2);
    vi.advanceTimersByTime(1000);
    expect(read).toHaveBeenCalledTimes(3);
  });

  it("is not made as it comes back while it is off", () => {
    const read = vi.fn();
    const behind = inAProject(() => useWhileShown(1000, read, false), true);
    behind.show(false);
    behind.show(true);
    vi.advanceTimersByTime(50);
    expect(read).not.toHaveBeenCalled();
  });
});

declare global {
  interface Window {
    __TAURI_INTERNALS__: { runCallback: (id: number, payload: unknown) => void };
  }
}

describe("a read made when the project changed on disk", () => {
  const PLANE = "/home/dev/plane";

  /** A core that says `plane-changed`, as it would for a settings file written. */
  function core() {
    const handlers: number[] = [];
    mockIPC((cmd, args) => {
      if (cmd !== "plugin:event|listen") return null;
      const { event, handler } = args as unknown as { event: string; handler: number };
      if (event === "plane-changed") handlers.push(handler);
      return 1;
    });
    return async () => {
      await act(async () => {
        for (const handler of handlers)
          window.__TAURI_INTERNALS__.runCallback(handler, {
            event: "plane-changed",
            id: 1,
            payload: { plane: PLANE, changes: null, answers: null },
          });
      });
    };
  }

  it("is counted at once for the project in front", async () => {
    const changed = core();
    const front = inAProject(() => usePlaneChanged([PLANE], SETTINGS), true);
    await act(async () => {});
    await changed();
    expect(front.result.current).toBe(1);
  });

  it("is not counted for a project behind, and is counted once as it comes back", async () => {
    const changed = core();
    const behind = inAProject(() => usePlaneChanged([PLANE], SETTINGS), true);
    await act(async () => {});
    behind.show(false);

    await changed();
    await changed();
    expect(behind.result.current).toBe(0);

    behind.show(true);
    // After it is painted, and once however often it changed.
    expect(behind.result.current).toBe(0);
    await act(async () => {
      await new Promise((done) => setTimeout(done, 60));
    });
    expect(behind.result.current).toBe(1);
  });

  it("is counted as a project comes back with nothing heard, for what no watcher told", async () => {
    core();
    const behind = inAProject(() => usePlaneChanged([PLANE], SETTINGS), true);
    await act(async () => {});
    behind.show(false);
    behind.show(true);
    await act(async () => {
      await new Promise((done) => setTimeout(done, 60));
    });
    expect(behind.result.current).toBe(1);
  });
});
