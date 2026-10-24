// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **A strip of tabs on React Aria** (#3042, `TabStrip.tsx`): the WAI-ARIA "Tabs" keyboard with
 * manual activation, the one Tab stop and where it goes back to, and what the strip keeps of its
 * own: the controls beside each tab, and keys a tab has taken.
 */
import { useState } from "react";
import { afterEach, describe, expect, it } from "vitest";
import { act, cleanup, render, screen, waitFor } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { StripTab, TabStripRow, useTabStrip } from "./TabStrip";

afterEach(cleanup);

/** A strip of `names`, the chosen one `chosen` (by name), each with a `×` beside its tab, and a
 *  `+` after the last, drawn as the chat strip draws them. */
function Strip({
  names,
  chosen,
  drawn = names,
  onChoose,
  lifted = false,
}: {
  names: string[];
  chosen: string;
  /** The tabs drawn; the others are collapsed out of the strip. */
  drawn?: string[];
  onChoose?: (name: string) => void;
  /** A tab is up, so its arrows are a drag's. */
  lifted?: boolean;
}) {
  const [inFront, setInFront] = useState(chosen);
  const along = useTabStrip({ keys: drawn, selected: inFront });
  return (
    <TabStripRow
      strip={along}
      name="Tabs"
      ids={drawn.map((name) => `tab-${name}`)}
      data-strip="Tabs"
    >
      {names
        .filter((name) => drawn.includes(name))
        .map((name) => (
          <span key={name}>
            <StripTab
              strip={along}
              tabKey={name}
              selected={name === inFront}
              tip={undefined}
              id={`tab-${name}`}
              onKeyDown={(event) => {
                if (lifted && event.key.startsWith("Arrow")) event.preventDefault();
              }}
              onClick={() => {
                setInFront(name);
                onChoose?.(name);
              }}
            >
              {name}
            </StripTab>
            <button type="button" tabIndex={-1} aria-label={`Close ${name}`} />
          </span>
        ))}
      <button type="button">New tab</button>
    </TabStripRow>
  );
}

const tabs = () => screen.getAllByRole("tab");
const stops = () => tabs().map((tab) => tab.getAttribute("tabindex"));

describe("a strip on React Aria", () => {
  it("is a tablist that owns its tabs by id, named, across", () => {
    render(<Strip names={["one", "two", "three"]} chosen="two" />);
    const list = screen.getByRole("tablist", { name: "Tabs" });
    expect(list).toHaveAttribute("aria-owns", "tab-one tab-two tab-three");
    expect(list).toHaveAttribute("aria-orientation", "horizontal");
    // It holds nothing itself: the cells, their `×` and the `+` are the strip's.
    expect(list.childElementCount).toBe(0);
    expect(tabs().map((tab) => tab.getAttribute("aria-selected"))).toEqual([
      "false",
      "true",
      "false",
    ]);
  });

  it("has one Tab stop, the chosen tab", () => {
    render(<Strip names={["one", "two", "three"]} chosen="two" />);
    expect(stops()).toEqual(["-1", "0", "-1"]);
  });

  it("is stopped at its first tab drawn when the chosen one is not drawn", () => {
    render(<Strip names={["one", "two", "three"]} chosen="three" drawn={["one", "two"]} />);
    expect(stops()).toEqual(["0", "-1"]);
    expect(tabs().every((tab) => tab.getAttribute("aria-selected") === "false")).toBe(true);
  });

  it("moves with the arrows, Home and End, and moving is not choosing", async () => {
    let chosen: string | undefined;
    render(
      <Strip names={["one", "two", "three"]} chosen="two" onChoose={(name) => (chosen = name)} />,
    );
    const [one, two, three] = tabs();
    act(() => two.focus());
    await userEvent.keyboard("{ArrowRight}");
    await waitFor(() => expect(three).toHaveFocus());
    expect(stops()).toEqual(["-1", "-1", "0"]);
    expect(two).toHaveAttribute("aria-selected", "true");
    await userEvent.keyboard("{Home}");
    await waitFor(() => expect(one).toHaveFocus());
    await userEvent.keyboard("{End}");
    await waitFor(() => expect(three).toHaveFocus());
    await userEvent.keyboard("{ArrowLeft}");
    await waitFor(() => expect(two).toHaveFocus());
    expect(chosen).toBeUndefined();
  });

  it("wraps past either end, as the WAI-ARIA pattern has it", async () => {
    render(<Strip names={["one", "two", "three"]} chosen="three" />);
    const [one, , three] = tabs();
    act(() => three.focus());
    await userEvent.keyboard("{ArrowRight}");
    await waitFor(() => expect(one).toHaveFocus());
    await userEvent.keyboard("{ArrowLeft}");
    await waitFor(() => expect(three).toHaveFocus());
  });

  it("chooses with Enter and Space, through the tab's own press", async () => {
    const chosen: string[] = [];
    render(
      <Strip names={["one", "two", "three"]} chosen="two" onChoose={(name) => chosen.push(name)} />,
    );
    const [one, two] = tabs();
    act(() => two.focus());
    await userEvent.keyboard("{ArrowLeft}");
    await waitFor(() => expect(one).toHaveFocus());
    await userEvent.keyboard("{Enter}");
    await waitFor(() => expect(one).toHaveAttribute("aria-selected", "true"));
    await userEvent.keyboard("{ArrowRight}");
    await waitFor(() => expect(two).toHaveFocus());
    await userEvent.keyboard(" ");
    await waitFor(() => expect(two).toHaveAttribute("aria-selected", "true"));
    expect(chosen).toEqual(["one", "two"]);
  });

  it("leaves the arrows of a control beside a tab to it", async () => {
    render(<Strip names={["one", "two", "three"]} chosen="two" />);
    const close = screen.getByRole("button", { name: "Close two" });
    act(() => close.focus());
    await userEvent.keyboard("{ArrowRight}");
    expect(close).toHaveFocus();
    expect(stops()).toEqual(["-1", "0", "-1"]);
  });

  it("leaves a key a tab has taken to the tab: a lifted tab's arrows are a drag's", async () => {
    render(<Strip names={["one", "two", "three"]} chosen="two" lifted />);
    const [, two] = tabs();
    act(() => two.focus());
    await userEvent.keyboard("{ArrowRight}");
    expect(two).toHaveFocus();
  });

  it("goes on to the `+` with Tab, and the stop goes back to the chosen tab", async () => {
    render(<Strip names={["one", "two", "three"]} chosen="two" />);
    const [one, two] = tabs();
    act(() => two.focus());
    await userEvent.keyboard("{ArrowLeft}");
    await waitFor(() => expect(one).toHaveFocus());
    await userEvent.tab();
    expect(screen.getByRole("button", { name: "New tab" })).toHaveFocus();
    expect(stops()).toEqual(["-1", "0", "-1"]);
    await userEvent.tab({ shift: true });
    expect(two).toHaveFocus();
  });

  it("keeps a stop when the tab with the keyboard goes", async () => {
    const { rerender } = render(<Strip names={["one", "two", "three"]} chosen="two" />);
    const [, , three] = tabs();
    act(() => three.focus());
    rerender(<Strip names={["one", "two", "three"]} chosen="two" drawn={["one", "two"]} />);
    await waitFor(() => expect(stops()).toEqual(["-1", "0"]));
  });
});
