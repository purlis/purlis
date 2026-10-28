// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The popover family's wrapper** (#2909): a popover held up by its owner comes and goes as its
 * owner says, every time, and R165's anatomy is one list.
 */
import { afterEach, describe, expect, it } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { useRef, useState } from "react";
import { InlineMenu, MenuItem } from "../menu/Menu";
import { AnchoredPopover, HungPopover, PopoverFilter, PopoverFooter, PopoverList } from "./Popover";

afterEach(cleanup);

function Refusal({ up }: { up: boolean }) {
  const box = useRef<HTMLSpanElement>(null);
  return (
    <>
      <span ref={box}>web</span>
      <AnchoredPopover triggerRef={box} isOpen={up}>
        <div role="alert">That name is taken.</div>
      </AnchoredPopover>
    </>
  );
}

describe("a popover held up by its owner", () => {
  it("comes back each time its owner holds it up again", () => {
    const { rerender } = render(<Refusal up />);
    expect(screen.getByRole("alert")).toBeInTheDocument();
    for (let again = 0; again < 3; again += 1) {
      rerender(<Refusal up={false} />);
      expect(screen.queryByRole("alert")).toBeNull();
      rerender(<Refusal up />);
      expect(screen.getByRole("alert")).toBeInTheDocument();
    }
  });
});

describe("a popover laid out as a list (R165)", () => {
  it("has one list part, which the rows of a menu in it are laid out in", () => {
    render(
      <div data-scope="popover" data-part="root" data-layout="list">
        <PopoverFilter>
          <span>filter</span>
        </PopoverFilter>
        <PopoverList>
          <InlineMenu aria-label="Workspaces">
            <MenuItem textValue="alpha">alpha</MenuItem>
          </InlineMenu>
        </PopoverList>
        <PopoverFooter />
      </div>,
    );
    const parts = [...document.querySelectorAll("[data-part]")].map((one) =>
      one.getAttribute("data-part"),
    );
    expect(parts.filter((one) => one === "list")).toHaveLength(1);
    expect(parts.filter((one) => ["filter", "rule", "list", "footer"].includes(one ?? ""))).toEqual(
      ["filter", "rule", "list", "footer"],
    );
    expect(screen.getByRole("menu", { name: "Workspaces" })).toHaveAttribute("data-in-list");
  });
});

/** A card hung from a chip (#3648): up until a press elsewhere hides it. */
function Hung() {
  const chip = useRef<HTMLButtonElement>(null);
  const [up, setUp] = useState(true);
  return (
    <>
      <button ref={chip} type="button">
        gamma
      </button>
      <button type="button">elsewhere</button>
      <HungPopover triggerRef={chip} isOpen={up} onHide={() => setUp(false)} aria-label="gamma">
        <div role="region" aria-label="Notices for gamma">
          <button type="button">Add a vision</button>
        </div>
      </HungPopover>
    </>
  );
}

describe("a popover hung from what it is about (#3648)", () => {
  it("is the family's notice surface, centred under it, with its point", () => {
    render(<Hung />);
    const root = screen
      .getByRole("region", { name: "Notices for gamma" })
      .closest('[data-scope="popover"][data-part="root"]');
    expect(root).toHaveAttribute("data-variant", "notice");
    expect(root).toHaveAttribute("data-placement", "bottom");
    expect(root?.querySelector('[data-part="arrow"]')).not.toBeNull();
  });

  it("takes no keyboard as it comes", () => {
    const elsewhere = document.createElement("input");
    document.body.append(elsewhere);
    elsewhere.focus();
    render(<Hung />);
    expect(document.activeElement).toBe(elsewhere);
    elsewhere.remove();
  });

  it("stays up for a press inside it, and is hidden by a press anywhere else", () => {
    render(<Hung />);
    fireEvent.pointerDown(screen.getByRole("button", { name: "Add a vision" }));
    expect(screen.getByRole("region", { name: "Notices for gamma" })).toBeInTheDocument();
    fireEvent.pointerDown(screen.getByRole("button", { name: "elsewhere" }));
    expect(screen.queryByRole("region", { name: "Notices for gamma" })).toBeNull();
  });

  it("is hidden by a press on what it hangs from, too", () => {
    render(<Hung />);
    fireEvent.pointerDown(screen.getByRole("button", { name: "gamma" }));
    expect(screen.queryByRole("region", { name: "Notices for gamma" })).toBeNull();
  });
});
