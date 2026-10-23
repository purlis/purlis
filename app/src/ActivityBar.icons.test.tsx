// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it } from "vitest";
import { act, cleanup, render, screen } from "@testing-library/react";
import { ActivityBar, VIEW_MARKS, type Tabbed } from "./ActivityBar";
import { symbolOf } from "./theme/railIcons";
import { CLASSIC_THEME, drawIn, type Theme } from "./theme/theme";

/**
 * **A theme's icon set on the rails** (#2337): the view a theme draws is its symbol, stroked as
 * the window's own marks are; a view it does not draw keeps its mark; Purlis Classic, which has
 * no set, draws every mark as before.
 */

const VIEWS: Tabbed[] = [
  { view: "chats", name: "Chats", mark: VIEW_MARKS.chats },
  { view: "search", name: "Search", mark: VIEW_MARKS.search },
];

function bar() {
  return render(
    <ActivityBar
      side="left"
      name="Navigation"
      views={VIEWS}
      open="chats"
      panelOf={(view) => `panel-${view}`}
      tabOf={(view) => `tab-${view}`}
      onPick={() => {}}
    />,
  );
}

const iconOf = (name: string) => screen.getByRole("tab", { name }).querySelector("svg");

const DUSK: Theme = {
  ...CLASSIC_THEME,
  name: "Dusk",
  icons: { chats: { paths: ["M3 3h18v18H3z", "M8 8h8"] } },
};

afterEach(() => {
  act(() => drawIn(CLASSIC_THEME));
  cleanup();
});

describe("the rails' icons", () => {
  it("are the window's own marks under Purlis Classic", () => {
    act(() => drawIn(CLASSIC_THEME));
    bar();
    for (const name of ["Chats", "Search"]) {
      expect(iconOf(name)?.classList.contains("lucide")).toBe(true);
      expect(iconOf(name)?.getAttribute("data-icon")).toBeNull();
    }
  });

  it("draw a theme's symbol for each view it draws, and the mark for the rest", () => {
    act(() => drawIn(DUSK));
    bar();
    const chats = iconOf("Chats");
    expect(chats?.getAttribute("data-icon")).toBe("chats");
    expect([...(chats?.querySelectorAll("path") ?? [])].map((p) => p.getAttribute("d"))).toEqual([
      "M3 3h18v18H3z",
      "M8 8h8",
    ]);
    expect(chats?.getAttribute("viewBox")).toBe("0 0 24 24");
    expect(chats?.getAttribute("fill")).toBe("none");
    expect(chats?.getAttribute("stroke")).toBe("currentColor");
    expect(chats?.getAttribute("stroke-width")).toBe("2");
    expect(chats?.getAttribute("stroke-linecap")).toBe("round");
    expect(chats?.getAttribute("stroke-linejoin")).toBe("round");
    expect(chats?.getAttribute("aria-hidden")).toBe("true");
    expect(iconOf("Search")?.classList.contains("lucide")).toBe(true);
  });

  it("follow the theme in force as it changes", () => {
    act(() => drawIn(CLASSIC_THEME));
    bar();
    expect(iconOf("Chats")?.classList.contains("lucide")).toBe(true);
    act(() => drawIn(DUSK));
    expect(iconOf("Chats")?.getAttribute("data-icon")).toBe("chats");
    act(() => drawIn(CLASSIC_THEME));
    expect(iconOf("Chats")?.classList.contains("lucide")).toBe(true);
  });

  it("draw no path outside the path grammar, whatever reached the window", () => {
    act(() =>
      drawIn({
        ...CLASSIC_THEME,
        name: "Odd",
        icons: { chats: { paths: ["M1 1h2", 'M1 1"/><g onload="x', "L1 1"] } },
      }),
    );
    bar();
    const drawn = [...(iconOf("Chats")?.querySelectorAll("path") ?? [])];
    expect(drawn.map((p) => p.getAttribute("d"))).toEqual(["M1 1h2"]);
  });

  it("never take a name an object inherits as a view the set draws", () => {
    expect(symbolOf({}, "toString")).toBeUndefined();
    expect(symbolOf({}, "constructor")).toBeUndefined();
    expect(symbolOf({ chats: { paths: ["M1 1"] } }, "chats")).toEqual({ paths: ["M1 1"] });
  });
});
