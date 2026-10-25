// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/// <reference types="node" />
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { Closer, Doer, MARKS, Pin, ShowMore } from "./StripMarks";
import { PaneDoing } from "./LayoutPanes";
import { drawnWith } from "./cascade.testkit";
import type { Offer } from "./actions";

/**
 * The strips and the bar, drawn as tabs with icons (M6.6).
 *
 * **What an icon must never do is rename a control.** Every scenario spec reaches the bar by
 * words — `pressOnly("New tab")`, `button[aria-pressed="true"]=Explorer` — and a screen reader
 * announces it by the same words. So these tests hold, for each control that gained a mark,
 * that the mark is there AND that the name is exactly what it was.
 */

afterEach(cleanup);

function offer(id: string, title: string, on: Partial<Offer> = {}): Offer {
  return { id, title, available: true, reason: "", does: { verb: "nothing" }, ...on };
}

describe("the bar's buttons carry a mark beside their words", () => {
  it.each([
    ["chat.new", "New tab", "lucide-plus"],
    ["pane.split.right", "Split right", "lucide-square-split-horizontal"],
    ["pane.split.down", "Split down", "lucide-square-split-vertical"],
    ["pane.close", "End this pane's chat", "lucide-x"],
    // The project strip's two, and they are deliberately not the same glyph: `FolderPlus` is
    // what every file manager puts on *New folder*, `FolderOpen` on *Open* (charter-app#178).
    // Two icon-only buttons an inch apart carrying one icon is a strip aimed at by memory.
    ["project.create", "New project…", "lucide-folder-plus"],
    ["project.open", "Open a project…", "lucide-folder-open"],
    // And the workspace strip's own `+` (charter-app#193), which shares `chat.new`'s glyph
    // rather than taking a third folder icon: each is the one control at the end of its own
    // strip, a whole row apart from the other, and both mean *make one more of what this
    // strip lists*. #178's rule above is about two controls drawn side by side.
    ["workspace.create", "New workspace…", "lucide-plus"],
  ])("%s is named %s and draws its mark", (id, title, mark) => {
    render(<Doer offer={offer(id, title)} onPress={() => {}} />);

    const button = screen.getByRole("button", { name: title });
    expect(button.textContent).toBe(title);
    expect(button.querySelector(`svg.${mark}`)).not.toBeNull();
  });

  it("draws words alone for a row it has no mark for, rather than dropping the button", () => {
    render(<Doer offer={offer("charter.quit", "Quit charter")} onPress={() => {}} />);

    expect(screen.getByRole("button", { name: "Quit charter" }).querySelector("svg")).toBeNull();
  });

  it("marks the button that ends a pane's chat as one that ends something", () => {
    render(<Doer offer={offer("pane.close", "End this pane's chat")} onPress={() => {}} />);

    expect(screen.getByRole("button")).toHaveClass("ends-a-chat");
  });

  it("has a mark for exactly the rows the bar and the project strip draw", () => {
    // A row added to the bar without a mark is not an error — it draws its words — but a
    // mark for a row nothing draws is dead weight that looks like coverage.
    expect(Object.keys(MARKS).sort()).toEqual(
      [
        "chat.new",
        "pane.close",
        "pane.split.down",
        "pane.split.right",
        "project.create",
        "project.open",
        // The project switcher's button, at the start of the strip's controls (FR-27).
        "project.switch",
        "workspace.create",
      ].sort(),
    );
  });

  /**
   * **Icon-only is only legible while the icons drawn TOGETHER differ** (charter-app#178,
   * narrowed in charter-app#193 to what its own argument supports).
   *
   * The argument was about the project strip: its controls carry no words at all —
   * `aria-label` is their whole name — so two of them an inch apart drawing one glyph leaves
   * a pointer with nothing to tell them apart, and the operator pressing *New project…* when
   * they meant *Open a project…* gets a folder-picker they did not ask for at best. That is a
   * claim about **adjacency**, and it was written down as a claim about the whole set because
   * at the time the whole set was one group.
   *
   * It is not any more. `workspace.create` is the one control at the end of the workspace
   * strip and `chat.new` is the one control on the bar, a whole row apart, and they share
   * `Plus` on purpose: the `+` at the end of a strip makes one more of what the strip lists,
   * which is the thing an operator learns once and then reads on every strip in the window.
   * Holding the whole set to be distinct would forbid exactly that, so the groups are written
   * down instead and each is held to the original rule.
   */
  const SIDE_BY_SIDE: Record<string, string[]> = {
    // `App.tsx`'s `.strip-doing`, the only place two icon-only rows are drawn together.
    "the project strip": ["project.switch", "project.open", "project.create"],
    // One each, so these cannot collide with anything — listed so that a second control
    // arriving on either strip has somewhere to be added and something to fail against.
    "the workspace strip": ["workspace.create"],
    "the chat strip": ["chat.new"],
  };

  it.each(Object.keys(SIDE_BY_SIDE))("never gives %s two rows with the same mark", (strip) => {
    const marks = SIDE_BY_SIDE[strip].map((id) => MARKS[id]);
    expect(marks.filter(Boolean)).toHaveLength(SIDE_BY_SIDE[strip].length);
    expect(new Set(marks).size).toBe(marks.length);
  });
});

describe("a tab's close", () => {
  it("is named for what it does, with the X drawn and unread", async () => {
    const onPress = vi.fn();
    const ends = offer("tab.close:3", "End chat 3 steward", { note: "Ends the program." });
    render(<Closer offer={ends} onPress={onPress} />);

    const button = screen.getByRole("button", { name: "End chat 3 steward" });
    expect(button.querySelector("svg.lucide-x")?.getAttribute("aria-hidden")).toBe("true");
    await userEvent.click(button);
    expect(onPress).toHaveBeenCalledWith(ends);
  });

  /**
   * **It may not look lighter than it is** (charter-app#130). The `×` ends a chat with no undo,
   * and the one thing the styling must not do is agree with a glyph that reads as "hide".
   * jsdom computes no hover, so this reads the rule.
   */
  it("goes to the danger colours when a pointer is on it, in the stylesheet", () => {
    // The session tabs' `×` is the tabs family's (`ui/tabs/tabs.css`, #2914).
    const css = readFileSync(join(process.cwd(), "src/ui/tabs/tabs.css"), "utf8");
    const hover =
      /\[data-part="closer"\]\[data-scope="tabs"\]:hover\s*\{([^}]*)\}/.exec(css)?.[1] ?? "";
    expect(hover).toMatch(/background-color:\s*var\(--danger-surface\)/);
    expect(hover).toMatch(/color:\s*var\(--danger-text\)/);
  });
});

describe("a pinned mark", () => {
  it("is still one image named for what is pinned, with the pin drawn inside it", () => {
    render(<Pin held what="chat" />);

    const mark = screen.getByRole("img", { name: "pinned chat" });
    // The name is the span's, and the drawing inside it is hidden. `role="img"` already makes
    // its children presentational in the spec, but that is a rule every engine has to get
    // right; `aria-hidden` on the drawing is the one nothing can misread.
    expect(mark.querySelector("svg.lucide-pin")?.getAttribute("aria-hidden")).toBe("true");
  });
});

/**
 * **A strip's tabs are cells, and the selected one is a step lighter — nothing more.** jsdom
 * lays nothing out, so what can be held here is the stylesheet's claim: all three strips draw
 * the selected tab from one token, and hovering never repaints the tab that is selected.
 */
describe("the three strips share one tab shape", () => {
  // Comments out first: a selector read with the prose above it splits on the prose's commas.
  const css = readFileSync(join(process.cwd(), "src/App.css"), "utf8").replace(
    /\/\*[\s\S]*?\*\//g,
    "",
  );

  it("draws the selected tab a step lighter, from one token, on every strip", () => {
    // The operator: *"little lighter for selected tab"* (charter-app#193). The projects are the
    // rail since #2915, which rings the current one instead (`ui/rail`), and the workspaces are
    // the title bar's chips since #2916 (`ui/titlebar`). A chat's tab is the tabs family's
    // (#2914): docked, it takes the panel's fill and joins it; Purlis Classic keeps the step
    // lighter through its parts.
    const classic = JSON.parse(
      readFileSync(
        join(process.cwd(), "../crates/purlis-core/src/theme/built-in/purlis-classic/theme.json"),
        "utf8",
      ),
    ) as { parts: Record<string, unknown> };
    expect(classic.parts["tabs.tab.selected.fill"]).toBe("layer.selected");
  });

  it("lights no edge under a tab, because a lighter background is the whole signal", () => {
    // #171 lit the selected tab's bottom edge in the accent colour. The operator turned down
    // the coloured lines — *"borders are not feeling well"* — and the edge was one of them. A
    // pseudo-element on a strip's tab is how it was drawn, so none may come back quietly.
    const edges = [...css.matchAll(/([^{}]*::after[^{}]*)\{/g)]
      .flatMap((hit) => hit[1].split(","))
      .map((one) => one.trim())
      .filter((one) => /\.projects|\.workspaces-strip|\.tabs/.test(one));
    expect(edges).toEqual([]);
  });

  it("never lets hover repaint the selected tab", () => {
    // Whatever of the window's own stylesheet still draws a tab under the pointer.
    const hovers = [...css.matchAll(/([^{}]*\[role="tab"\]:hover[^{}]*)\{/g)].map((hit) => hit[1]);
    for (const selector of hovers) {
      for (const one of selector.split(",")) {
        expect(one).toContain(':not([aria-selected="true"])');
      }
    }
  });

  it("centres a tab's label in its cell", () => {
    // The operator's *"lets make tabs labels center aligned"* (charter-app#193). A chat's tab is the tabs family's (#2914), and centres its label in its cell too.
    const family = readFileSync(join(process.cwd(), "src/ui/tabs/tabs.css"), "utf8");
    expect(
      /\[data-part="tab"\] > \[role="tab"\] \{[^}]*justify-content: center;/.test(family),
    ).toBe(true);
  });
});

/**
 * **`N more` is one button, not one per strip** (charter-app#193).
 *
 * The operator: *"lets make 'N more' button looks like that buttons, to have all buttons in
 * same style."* It was drawn three ways — a bordered box on the chat strip from `.bar button`,
 * a borderless one on the project strip from `.projects button`, and a rule of its own on the
 * workspace strip with a third padding — and none of those differences was about the control.
 * Each was whichever ancestor's rule happened to reach it, which is a look that changes when
 * somebody moves the markup.
 *
 * jsdom computes no cascade, so the property is held over the stylesheet's text: nothing
 * dresses this per strip, and what does dress it is the family the `+` beside it is in.
 */
describe("the show-more button", () => {
  const css = readFileSync(join(process.cwd(), "src/App.css"), "utf8").replace(
    /\/\*[\s\S]*?\*\//g,
    "",
  );

  /** Every selector in the stylesheet that opens a rule and mentions `.show-more`. */
  const selectors = [...css.matchAll(/([^{}]*)\{/g)]
    .flatMap((hit) => hit[1].split(","))
    .map((one) => one.trim())
    .filter((one) => one.includes(".show-more"));

  it("is dressed by nothing that names a strip", () => {
    expect(selectors.length).toBeGreaterThan(0);
    const perStrip = selectors.filter((one) =>
      [".projects", ".workspaces", ".bar", ".tabs"].some((strip) => one.includes(strip)),
    );
    expect(perStrip, "one control, one rule — a strip may not redress it").toEqual([]);
  });

  // The quiet ghost of the button family (#3214), as the `+` beside it is: no box, quiet words.
  it("is the same quiet ghost as the `+` beside it", () => {
    render(
      <>
        <Doer offer={offer("chat.new", "New tab")} onPress={() => {}} iconOnly />
        <ShowMore
          noun="tab"
          hidden={[{ key: "a", offer: offer("tab.select:a", "a"), needs: 0, children: "a" }]}
          onPress={() => {}}
        />
      </>,
    );
    const plus = screen.getByRole("button", { name: "New tab" });
    const more = screen.getByRole("button", { name: /^Show 1 tab/ });
    for (const one of [plus, more]) {
      expect(one).toHaveAttribute("data-scope", "button");
      expect(one).toHaveAttribute("data-variant", "quiet");
      expect(one).toHaveAttribute("data-size", "sm");
    }
    expect(plus).toHaveClass("bare");
    expect(more).toHaveClass("show-more");
    expect(more.textContent).toContain("1 more");
  });
});

/**
 * **What says which of the three strips you are looking at** (charter-app#193).
 *
 * charter-app#171 drew the nesting with three signals — height, inset and surface — and the
 * operator read the inset back off the running app as stray padding: *"workspaces tabs and
 * sessions tabs have some padding from left, they should be like project tabs without
 * padding."* The inset is gone, and so are the coloured rules that first replaced it — *"this is
 * not looks professional, it should be minimalistic, and i prefer to change little bit
 * backgrounds of tabs"*. A quiet shade per strip carries the depth, one token each.
 *
 * jsdom lays nothing out and computes no stylesheet, so what can be held here is the rule as it
 * is written — which is enough for the claim: each strip names its own shade, no strip draws a
 * line under itself, and nothing indents a strip any more.
 */
describe("each strip says which layer it is, by shade rather than by indent or line", () => {
  const raw = readFileSync(join(process.cwd(), "src/App.css"), "utf8");
  const css = raw.replace(/\/\*[\s\S]*?\*\//g, "");

  /** One rule's declarations, by the selector that opens it. */
  const block = (selector: string): string => {
    const found = new RegExp(
      `(^|[},])\\s*${selector.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}\\s*\\{([^}]*)\\}`,
    ).exec(css);
    expect(found, `no ${selector} rule in App.css`).not.toBeNull();
    return found?.[2] ?? "";
  };

  /** What a rule sets as its inline-start padding, in whichever of the three spellings. */
  const beginsAt = (declarations: string): string => {
    const start = /padding-inline-start:\s*([^;]+)/.exec(declarations)?.[1];
    if (start) return start.trim();
    const inline = /padding-inline:\s*([^;]+)/.exec(declarations)?.[1];
    if (inline) return inline.trim().split(/\s+/)[0];
    const all = /(?:^|[;{\s])padding:\s*([^;]+)/.exec(declarations)?.[1];
    if (all === undefined) return "0";
    // top | top right | top right bottom | top right bottom left
    const parts = all.trim().split(/\s+/);
    return parts.length === 4 ? parts[3] : parts.length === 1 ? parts[0] : parts[1];
  };

  it("draws the chat strip as its theme says: on the panel with no shade, or on its own", () => {
    // The chat strip is the tabs family's strip (#2914): docked on the content panel (Purlis)
    // it has no ground of its own, and on a strip of its own (Purlis Classic's parts) it takes
    // the third shade, `--layer-chat`, which the engine writes for `tabs.layout.attach: strip`.
    const rule = block(".bar");
    expect(rule, ".bar takes its ground from the family").not.toMatch(/background|border/);
    const family = readFileSync(join(process.cwd(), "src/ui/tabs/tabs.css"), "utf8");
    expect(family).toMatch(
      /\[data-scope="tabs"\]\[data-part="strip"\] \{[^}]*background-color: transparent;/,
    );
    const classic = JSON.parse(
      readFileSync(
        join(process.cwd(), "../crates/purlis-core/src/theme/built-in/purlis-classic/theme.json"),
        "utf8",
      ),
    ) as { parts: Record<string, unknown> };
    expect(classic.parts["tabs.layout.attach"]).toBe("strip");
  });

  it("indents nothing, so a strip begins at the edge of the column it is drawn in", () => {
    // `--nested` was #171's one value used twice. A strip that starts further in than the one
    // above it is the thing the operator asked to have taken away, and the property is that
    // there is no such number left to reach for. The chat strip is the editor's tab bar since
    // DS-6 (#1836), so its edge is the centre column's and not the window's; it still adds no
    // inset of its own.
    expect(raw).not.toContain("--nested");
    for (const selector of [".bar"]) {
      expect(beginsAt(block(selector)), `${selector} begins further in than the strip above`).toBe(
        "0",
      );
    }
  });
});

/**
 * **A pane's close that ends a chat says so under the pointer** (#3214): the quiet ghost's hover
 * would draw it as light as a split, so `paneFrame.css` gives it the danger colour, as a tab's
 * `×` has. A close that ends nothing (a view's, a task's minimise) keeps the quiet hover.
 */
describe("a pane's split and close", () => {
  function drawn(ends: boolean) {
    const close = offer("pane.close", ends ? "End this pane's chat" : "Close this view", {
      does: { verb: "closePane", ends },
    });
    render(
      <PaneDoing
        pane={0}
        offerFor={(id) => (id === "pane.split.right" ? offer(id, "Split right") : undefined)}
        closeOf={() => close}
        onPaneDoes={() => {}}
      />,
    );
    return {
      split: screen.getByRole("button", { name: "Split right" }),
      close: screen.getByRole("button", { name: close.title }),
    };
  }

  it("draws the close that ends a chat in the danger colour under the pointer", () => {
    const { split, close } = drawn(true);
    expect(close).toHaveAttribute("data-variant", "quiet");
    expect(close).toHaveClass("ends-a-chat");
    expect(drawnWith(close, "background-color", { hover: true })).toBe("var(--danger-surface)");
    expect(drawnWith(close, "color", { hover: true })).toBe("var(--danger-text)");
    // At rest it is as quiet as the split beside it, and the split keeps the quiet hover.
    expect(drawnWith(close, "color")).toBe("var(--text-quiet)");
    expect(drawnWith(split, "background-color", { hover: true })).toBe("var(--surface-hover)");
  });

  it("keeps the quiet hover on a close that ends nothing", () => {
    const { close } = drawn(false);
    expect(close).not.toHaveClass("ends-a-chat");
    expect(drawnWith(close, "background-color", { hover: true })).toBe("var(--surface-hover)");
  });
});
