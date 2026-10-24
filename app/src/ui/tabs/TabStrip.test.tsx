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
import { Button, Dialog, DialogTrigger, Modal } from "react-aria-components";
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
  // A tab chosen from outside the strip (the palette, a menu) is handed in as `chosen`.
  const [handed, setHanded] = useState(chosen);
  if (handed !== chosen) {
    setHanded(chosen);
    setInFront(chosen);
  }
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

/** The strip, and a box outside it the keyboard can be in (the palette's, a terminal's). */
function Window(props: Parameters<typeof Strip>[0]) {
  return (
    <>
      <Strip {...props} />
      <input aria-label="Palette" />
    </>
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

  it("never takes the keyboard back when the tab it was on goes and it went elsewhere", async () => {
    const names = ["one", "two", "three"];
    const { rerender } = render(<Window names={names} chosen="two" />);
    act(() => tabs()[2].focus());
    // The tab with the keyboard goes: no blur reaches the strip as it leaves the document.
    rerender(<Window names={names} chosen="two" drawn={["one", "two"]} />);
    await waitFor(() => expect(stops()).toEqual(["-1", "0"]));
    expect(tabs().some((tab) => tab === document.activeElement)).toBe(false);
    // The keyboard goes to the palette, and the strip changes under it: a tab is chosen, one
    // comes and goes. It stays in the palette.
    const palette = screen.getByRole("textbox", { name: "Palette" });
    act(() => palette.focus());
    rerender(<Window names={names} chosen="one" drawn={["one", "two"]} />);
    rerender(<Window names={names} chosen="one" drawn={["one"]} />);
    rerender(<Window names={names} chosen="one" />);
    await waitFor(() => expect(stops()).toEqual(["0", "-1", "-1"]));
    expect(palette).toHaveFocus();
  });

  it("makes a tab chosen from outside the strip its stop, where Shift+Tab comes back in", async () => {
    const names = ["one", "two", "three"];
    const { rerender } = render(<Window names={names} chosen="two" />);
    const plus = screen.getByRole("button", { name: "New tab" });
    act(() => plus.focus());
    // Chosen from the palette or a menu, while the keyboard is not on the tabs.
    rerender(<Window names={names} chosen="three" />);
    await waitFor(() => expect(stops()).toEqual(["-1", "-1", "0"]));
    expect(plus).toHaveFocus();
    await userEvent.tab({ shift: true });
    expect(tabs()[2]).toHaveFocus();
  });

  it("leaves the keyboard on the `+` when the dialog it opened closes on a new chosen tab", async () => {
    // `use-skill.e2e`'s steps: the `+` opens Start a chat, Start starts a chat whose tab is
    // chosen and closes the dialog, and the keyboard is handed back. F2 then has to reach the
    // palette, not rename the new chat's tab.
    function Starting() {
      const [names, setNames] = useState(["one", "two"]);
      const [chosen, setChosen] = useState("two");
      const along = useTabStrip({ keys: names, selected: chosen });
      return (
        <TabStripRow strip={along} name="Tabs" ids={names.map((name) => `tab-${name}`)}>
          {names.map((name) => (
            <StripTab
              key={name}
              strip={along}
              tabKey={name}
              selected={name === chosen}
              tip={undefined}
              id={`tab-${name}`}
            >
              {name}
            </StripTab>
          ))}
          <DialogTrigger>
            <Button aria-label="New tab">+</Button>
            <Modal>
              <Dialog aria-label="Start a chat">
                {({ close }) => (
                  <Button
                    onPress={() => {
                      setNames((was) => [...was, "steward 42"]);
                      setChosen("steward 42");
                      close();
                    }}
                  >
                    Start
                  </Button>
                )}
              </Dialog>
            </Modal>
          </DialogTrigger>
        </TabStripRow>
      );
    }
    render(<Starting />);
    const plus = screen.getByRole("button", { name: "New tab" });
    await userEvent.click(plus);
    await userEvent.click(await screen.findByRole("button", { name: "Start" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    await waitFor(() => expect(stops()).toEqual(["-1", "-1", "0"]));
    await waitFor(() => expect(plus).toHaveFocus());
    expect(tabs().some((tab) => tab === document.activeElement)).toBe(false);
  });
});
