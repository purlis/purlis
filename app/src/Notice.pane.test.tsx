// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { readFileSync } from "node:fs";
import { join } from "node:path";
import { afterEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useContext, useLayoutEffect, useState } from "react";
import { Notice, NoticePaneRow } from "./Notice";
import {
  NOTICE_MOVED_JUST_NOW,
  NoticeWayGuard,
  carryPaneBand,
  forgetPaneBand,
  paneBand,
  useNoticeSettle,
} from "./noticeSettle";
import { drawnWith } from "./cascade.testkit";

/**
 * **A Notice in a pane's corner fits its pane, whatever it says** (#1481).
 *
 * The operator, from the dev build with two panes side by side: *"ui is broken for questions
 * modals"*. A Notice's sentence was one word wide and fourteen lines tall, its buttons each
 * wrapped to five lines beside it, and the block a button opened was drawn to the right of the
 * Notice and off the window.
 *
 * jsdom lays nothing out, so two things are held here and the measuring is `pane-notices.e2e.ts`:
 *
 * - **the shape `Notice` draws for a pane**: one box, the line, and under it what a way out
 *   opened. The stylesheet's rules are written against exactly this shape;
 * - **the rules as written**: each one is a property the fix needs, and a rule that went
 *   missing fails here before it is seen in a window.
 */

afterEach(cleanup);

/** What an element of a Notice is, by the notice family's attributes (#2912): a way out by its
 *  `data-way`, a part by its `data-part`, the line by its `data-scope` (the shared button row). */
const partOf = (one: Element) =>
  one.getAttribute("data-way") ??
  (one.getAttribute("data-scope") === "notice" ? one.getAttribute("data-part") : null) ??
  one.getAttribute("data-scope") ??
  one.tagName;

const opened = { id: "what-it-opens", open: true };

const aPaneNotice = (under?: React.ReactNode) =>
  render(
    <Notice
      cause="vault-refused:7:devops"
      at="pane"
      tone="trouble"
      label="Vault"
      fixes={[
        { label: "Allow steward to use this vault", onPress: () => undefined, opens: opened },
        { label: "Keep blocked", onPress: () => undefined },
      ]}
      onDismiss={() => undefined}
      under={under}
    >
      This chat runs as steward, so purlis did not open it.
    </Notice>,
  );

describe("a Notice in a pane's corner", () => {
  it("is one box: the line, and under it what a way out opened", () => {
    const { container } = aPaneNotice(<div id="what-it-opens">Allow lets every chat…</div>);

    const box = container.firstElementChild;
    expect(container.children).toHaveLength(1);
    expect(box).toHaveAttribute("data-scope", "notice");
    expect(box).toHaveAttribute("data-placement", "pane");
    expect(box).toHaveAttribute("data-variant", "trouble");
    expect(box).toHaveAttribute("data-opened");
    expect([...(box?.children ?? [])].map(partOf)).toEqual(["button-row", "under"]);
    // What it opened is outside the line: the line is a live region, and a form in one would
    // be read again on every keystroke.
    const line = screen.getByRole("status", { name: "Vault" });
    expect(line).not.toContainElement(screen.getByText("Allow lets every chat…"));
  });

  it("draws the sentence first on its line, then every way out, in order", () => {
    aPaneNotice();

    const line = screen.getByRole("status", { name: "Vault" });
    expect(line).toHaveAttribute("data-cause", "vault-refused:7:devops");
    expect([...line.children].map(partOf)).toEqual(["message", "fix", "fix", "dismiss"]);
    // Each a Tab stop (`docs/ui-primitives.md`), and the one that opens something says so.
    for (const button of screen.getAllByRole("button"))
      expect(button).toHaveAttribute("tabindex", "0");
    const allow = screen.getByRole("button", { name: "Allow steward to use this vault" });
    expect(allow).toHaveAttribute("aria-expanded", "true");
    expect(allow).toHaveAttribute("aria-controls", "what-it-opens");
  });

  it("is a box with nothing under the line when nothing is opened", () => {
    const { container } = aPaneNotice();

    const box = container.firstElementChild;
    expect(box).toHaveAttribute("data-placement", "pane");
    expect(box).not.toHaveAttribute("data-opened");
    expect(box?.children).toHaveLength(1);
  });

  it("draws the persona's mark as the sentence's first word, never as an item of its own", () => {
    // An item of its own is left alone on a row when the sentence takes the next one whole.
    render(
      <Notice cause="persona-grants:7" at="pane" persona="devops" onDismiss={() => undefined}>
        devops may use two vaults.
      </Notice>,
    );

    const line = screen.getByRole("status");
    const mark = line.querySelector(".persona-mark");
    expect(mark?.parentElement).toHaveAttribute("data-part", "message");
    expect(mark?.parentElement?.firstElementChild).toBe(mark);
  });
});

/**
 * **A pane's Notices stand in a row of their own, above the terminal, two at a time** (#1647).
 *
 * The operator's screenshot (2026-10-10): three cards stacked over a chat's terminal and hid the
 * conversation. The row takes height from the terminal instead, and like the band under the
 * strip (V91i) it shows at most two and keeps the rest behind "+N more".
 */
describe("a pane's row of Notices", () => {
  /** A pane Notice with one answer and a Dismiss, as `ChatNotices` draws them. */
  const one = (cause: string, says: string) => (
    <Notice
      key={cause}
      cause={cause}
      at="pane"
      label={says}
      fixes={[{ label: `Answer ${says}`, onPress: () => undefined }]}
      onDismiss={() => undefined}
    >
      {says}
    </Notice>
  );
  /** The Notices a person can see, top to bottom, by their names. */
  const seen = () => screen.queryAllByRole("status").map((line) => line.getAttribute("aria-label"));

  it("draws two and keeps the rest behind +N more, in the order the pane wrote them", async () => {
    render(
      <NoticePaneRow>
        {[one("a:1", "first"), one("b:1", "second"), one("c:1", "third")]}
      </NoticePaneRow>,
    );

    await waitFor(() => expect(seen()).toEqual(["first", "second"]));
    const more = screen.getByRole("button", { name: "+1 more" });
    expect(more).toHaveAttribute("aria-expanded", "false");
    expect(more).toHaveAttribute("tabindex", "0");
    // Behind it, not gone: every way out is still in the row.
    expect(document.querySelector('[data-cause="c:1"]')).not.toBeNull();
  });

  it("opens the rest in the row with every way out, and closes on Escape back on +N more", async () => {
    const user = userEvent.setup();
    render(
      <NoticePaneRow>
        {[one("a:1", "first"), one("b:1", "second"), one("c:1", "third")]}
      </NoticePaneRow>,
    );
    const more = await screen.findByRole("button", { name: "+1 more" });

    await user.click(more);
    expect(seen()).toEqual(["first", "second", "third"]);
    expect(more).toHaveAttribute("aria-expanded", "true");
    const third = screen.getByRole("status", { name: "third" });
    expect(within(third).getByRole("button", { name: "Answer third" })).toBeVisible();
    expect(within(third).getByRole("button", { name: "Dismiss" })).toBeVisible();

    within(third).getByRole("button", { name: "Answer third" }).focus();
    await user.keyboard("{Escape}");
    expect(seen()).toEqual(["first", "second"]);
    expect(more).toHaveFocus();
  });

  it("draws no +N more for two, and no row at all for none", async () => {
    const { container, rerender } = render(
      <NoticePaneRow>{[one("a:1", "first"), one("b:1", "second")]}</NoticePaneRow>,
    );
    await waitFor(() => expect(seen()).toEqual(["first", "second"]));
    expect(screen.queryByRole("button", { name: /more/ })).toBeNull();

    rerender(<NoticePaneRow>{null}</NoticePaneRow>);
    await waitFor(() =>
      expect(container.querySelector('[data-scope="notice-row"]')).toHaveAttribute("data-empty"),
    );
  });

  it("stands the ones that ask before one that only says, keeping the pane's order on screen", async () => {
    // The session's own "Allowed." came first in the pane, and the question two of its tasks
    // wait on came third: the question stands, and what can be read later is behind +1 more.
    render(
      <NoticePaneRow>
        <Notice cause="said:1" at="pane" label="allowed" onDismiss={() => undefined}>
          Allowed for this chat.
        </Notice>
        {one("b:1", "second")}
        {one("c:1", "third")}
      </NoticePaneRow>,
    );

    await waitFor(() => expect(seen()).toEqual(["second", "third"]));
    expect(screen.getByRole("button", { name: "+1 more" })).toBeTruthy();
    await userEvent.click(screen.getByRole("button", { name: "+1 more" }));
    expect(seen()).toEqual(["allowed", "second", "third"]);
  });

  it("stands a Notice that comes to ask something where it stood behind +N more", async () => {
    // The same Notice, the same box: it only said something, and now it asks. It is the box's
    // `data-asks` that changes, not the stack's children.
    let ask: () => void = () => undefined;
    function Pane() {
      const [asking, setAsking] = useState(false);
      ask = () => setAsking(true);
      return (
        <NoticePaneRow>
          <Notice cause="said:1" at="pane" label="first said" onDismiss={() => undefined}>
            Said first.
          </Notice>
          <Notice cause="said:2" at="pane" label="second said" onDismiss={() => undefined}>
            Said second.
          </Notice>
          <Notice
            cause="later:1"
            at="pane"
            label="later"
            fixes={asking ? [{ label: "Answer later", onPress: () => undefined }] : undefined}
            onDismiss={() => undefined}
          >
            Later.
          </Notice>
        </NoticePaneRow>
      );
    }
    render(<Pane />);
    await waitFor(() => expect(seen()).toEqual(["first said", "second said"]));

    act(() => ask());
    await waitFor(() => expect(seen()).toEqual(["first said", "later"]));
  });

  it("follows Notices that come and go, and never hides the one the keyboard is on", async () => {
    const user = userEvent.setup();
    let raise: (cause: string) => void = () => undefined;
    function Pane() {
      const [first, setFirst] = useState<string[]>([]);
      raise = (cause) => setFirst((was) => [cause, ...was]);
      return (
        <NoticePaneRow>
          {first.map((cause) => one(cause, cause))}
          {one("b:1", "second")}
          {one("c:1", "third")}
        </NoticePaneRow>
      );
    }
    render(<Pane />);
    await waitFor(() => expect(seen()).toEqual(["second", "third"]));

    // The person is on the second Notice's answer when one arrives above it.
    await user.click(screen.getByRole("button", { name: "Answer third" }));
    expect(screen.getByRole("button", { name: "Answer third" })).toHaveFocus();
    raise("new:1");
    await waitFor(() => expect(screen.getByRole("button", { name: "+1 more" })).toBeTruthy());
    expect(seen()).toEqual(["new:1", "second", "third"]);
    expect(screen.getByRole("button", { name: "Answer third" })).toHaveFocus();

    // Once the keyboard leaves it, it goes behind +N more like the rest.
    screen.getByRole("button", { name: "Answer new:1" }).focus();
    await waitFor(() => expect(seen()).toEqual(["new:1", "second"]));
  });
});

describe("the dispatch question's extras, in a pane", () => {
  it("draws what is under the answers in the Notice's own box: the choice, then the brief", () => {
    // The shape the dispatch question hands the Notice (#1505): its workspace choice and the
    // brief are both under the line, in that order, and neither is beside the answers.
    aPaneNotice(
      <>
        <div className="dispatch-choice" role="radiogroup">
          <p>Where an Allow for you or for the project holds</p>
        </div>
        <pre>the brief</pre>
      </>,
    );
    const box = document.querySelector('[data-scope="notice"][data-placement="pane"]');
    const line = box?.querySelector(':scope > [data-scope="button-row"]');
    const under = box?.querySelector('[data-scope="notice"][data-part="under"]');
    expect(under?.children.length).toBe(2);
    expect(under?.children[0]).toHaveClass("dispatch-choice");
    expect(under?.children[1].tagName).toBe("PRE");
    expect(line?.querySelector(".dispatch-choice")).toBeNull();
    expect(line?.nextElementSibling).toBe(under);
  });
});

describe("a Notice in the Inbox", () => {
  it.each(["inbox"] as const)(
    "stands alone at %s outside a list: one surface, the line and what it opened",
    (at) => {
      const { container } = render(
        <Notice
          cause="doctor-finding:git-identity"
          at={at}
          persona="devops"
          fixes={[{ label: "Set it", onPress: () => undefined, opens: opened }]}
          under={<div id="what-it-opens">a form</div>}
        >
          git has no name for you.
        </Notice>,
      );

      expect(container.querySelector('[data-placement="pane"]')).toBeNull();
      // Standing alone, its box is the inline surface (#1855), the line and what it opened in it.
      expect(container.children).toHaveLength(1);
      const box = container.firstElementChild;
      expect(box).toHaveAttribute("data-placement", "alone");
      expect([...(box?.children ?? [])].map(partOf)).toEqual(["button-row", "under"]);
      // The mark stays before the sentence, where the Inbox's rules place it.
      const line = screen.getByRole("status");
      expect(line.firstElementChild).toHaveClass("persona-mark");
    },
  );
});

/**
 * **The rules the layout is made of, held over the stylesheet.** Each is named for what breaks
 * without it.
 */
describe("the pane Notice's rules", () => {
  const css = readFileSync(join(process.cwd(), "src/App.css"), "utf8").replace(
    /\/\*[\s\S]*?\*\//g,
    "",
  );
  const rule = (selector: string) =>
    new RegExp(`(^|[},])\\s*${selector}\\s*\\{([^}]*)\\}`).exec(css)?.[2] ?? "";
  /** The notice family's stylesheet (#2912), each rule's selector on one line. */
  const family = readFileSync(join(process.cwd(), "src/ui/notice/notice.css"), "utf8")
    .replace(/\/\*[\s\S]*?\*\//g, "")
    .replace(/\s+/g, " ");
  /** The declarations of the family's rule written for exactly `selector`. */
  const of = (selector: string) =>
    [...family.matchAll(/([^{}]+)\{([^}]*)\}/g)].find(
      (hit) => hit[1].trim() === selector.replace(/\s+/g, " "),
    )?.[2] ?? "";
  const ROOT = '[data-scope="notice"][data-part="root"]';
  const PANE = `${ROOT}[data-placement="pane"]`;
  const UNDER = '[data-placement="pane"] > [data-scope="notice"][data-part="under"]';
  const ROW = '[data-scope="notice-row"][data-part="root"]';
  const STACK = '[data-scope="notice-row"][data-part="stack"]';

  it("draws nothing a pane holds outside it", () => {
    // By a box that cannot be scrolled.
    expect(rule("\\.pane-frame")).toMatch(/overflow:\s*clip/);
  });

  it("gives the Notices a row of their own above the terminal, never over it (#1647)", () => {
    // The pane is a column: the row, then the body the terminal and its corners are in, which
    // has what the row leaves.
    const frame = rule("\\.pane-frame");
    expect(frame).toMatch(/display:\s*flex/);
    expect(frame).toMatch(/flex-direction:\s*column/);
    const body = rule("\\.pane-body");
    expect(body).toMatch(/position:\s*relative/);
    expect(body).toMatch(/flex:\s*1 1 0/);
    expect(body).toMatch(/min-height:\s*0/);
    // The row is in the column, not positioned over anything, and leaves the terminal most of
    // the pane whatever is opened.
    const row = of(ROW);
    expect(row).not.toMatch(/position|z-index/);
    expect(row).toMatch(/max-block-size:\s*60%/);
    expect(row).toMatch(/min-block-size:\s*0/);
    expect(of(`${ROW}[data-empty]`)).toMatch(/display:\s*none/);
    // What the row keeps behind "+N more" is not drawn, whatever its box's own display says.
    expect(of(`${STACK} > [hidden]`)).toMatch(/display:\s*none/);
  });

  it("stacks a pane's Notices and scrolls them when they are taller than the pane", () => {
    const stack = of(STACK);
    expect(stack).toMatch(/flex-direction:\s*column/);
    expect(stack).toMatch(/min-block-size:\s*0/);
    expect(stack).toMatch(/overflow-y:\s*auto/);
    // Never wider than the pane, and no wider than a sentence reads well.
    expect(stack).toMatch(/max-inline-size:\s*min\(100%, 40rem\)/);
    expect(of(PANE)).toMatch(/max-inline-size:\s*100%/);
  });

  it("wraps the ways out under the sentence, and never breaks one that fits", () => {
    // The line is the shared row of buttons that wraps (`ButtonRow`, #1855), read through
    // the window's cascade.
    aPaneNotice();
    const line = screen.getByRole("status", { name: "Vault" });
    expect(drawnWith(line, "flex-wrap")).toBe("wrap");
    // The sentence is as wide as it reads, so it shares a row only when everything fits.
    expect(
      of(
        '[data-placement="pane"] > [data-scope="button-row"] > [data-scope="notice"][data-part="message"]',
      ),
    ).toMatch(/flex:\s*0 1 auto/);
    // A way out never shrinks, so a row with no room sends it to the next; only its cap at the
    // row's width can make it narrower than its words, and only then do they wrap.
    for (const way of within(line).getAllByRole("button")) {
      expect(drawnWith(way, "flex")).toBe("none");
      expect(drawnWith(way, "max-inline-size")).toBe("100%");
    }
  });

  it("keeps what a way out opened at the Notice's width, its draft scrolling in its own box", () => {
    expect(of(UNDER)).toMatch(/min-inline-size:\s*0/);
    const draft = of(`${UNDER} pre`);
    expect(draft).toMatch(/max-inline-size:\s*100%/);
    expect(draft).toMatch(/overflow:\s*auto/);
  });

  it("keeps the dispatch question's workspace choice inside a narrow pane", () => {
    // #1505. Since train 67 the choice is the set's `Choice` with no row, in a group that may
    // shrink to the pane and breaks a workspace's long name rather than widen the Notice; the
    // App.css rules this test read went with the hand-built radios.
    const source = readFileSync(join(process.cwd(), "src", "DispatchGrantNotice.tsx"), "utf8");
    expect(source).toMatch(/className="[^"]*\bmin-w-0\b[^"]*\bmax-w-full\b[^"]*wrap-anywhere/);
    expect(rule("\\.dispatch-within")).toBe("");
  });

  it("keeps a dispatch's boxes and access line at the Notice's width, breaking a host anywhere", () => {
    // #1502: a persona's hosts are long unbroken words. Without these the list of boxes, or
    // the sentence of what the target works with, widens the Notice past a narrow pane.
    const also = rule("\\.dispatch-also");
    expect(also).toMatch(/min-width:\s*0/);
    expect(also).toMatch(/overflow-wrap:\s*anywhere/);
    // A box's name and what it works with shrink with the Notice, in the grid the checkbox
    // family draws (#2907).
    const family = readFileSync(join(process.cwd(), "src", "ui", "choice", "choice.css"), "utf8");
    expect(family).toMatch(
      /\[data-scope="checkbox"\]\[data-part="root"\] \{[^}]*grid-template-columns: auto minmax\(0, 1fr\)/,
    );
    expect(family).toMatch(
      /\[data-scope="checkbox"\]\[data-part="label"\] \{[^}]*min-inline-size: 0/,
    );
    // It is drawn under the line, so the rules for what a way out opened hold it too.
    expect(of(`${UNDER} > *`)).toMatch(/max-inline-size:\s*100%/);
  });

  it("is never taller than its pane: what it opened gives way, the answers never do", () => {
    // The dispatch question in a short pane (#1483's train): the box shrinks in the stack,
    // its line and answers keep their height, and what is under them scrolls, the brief
    // first.
    expect(of(ROOT)).toMatch(/flex-direction:\s*column/);
    const box = of(PANE);
    expect(box).toMatch(/min-block-size:\s*0/);
    expect(box).toMatch(/flex:\s*0 1 auto/);
    expect(of(`${PANE} > [data-scope="button-row"]`)).toMatch(/flex:\s*none/);
    const under = of(UNDER);
    expect(under).toMatch(/min-block-size:\s*0/);
    expect(under).toMatch(/overflow-y:\s*auto/);
    expect(
      rule(
        '\\[data-placement="pane"\\] > \\[data-scope="notice"\\]\\[data-part="under"\\] > \\.block-report',
      ),
    ).toMatch(/min-height:\s*0/);
    // The stack still scrolls as a whole when the Notices together are too tall.
    expect(of(STACK)).toMatch(/overflow-y:\s*auto/);
  });

  it("is opaque", () => {
    const { container } = aPaneNotice();
    // Its trouble is the danger ground (#2912), as opaque as the raised one news stands on.
    expect(drawnWith(container.firstElementChild as Element, "background-color")).toBe(
      "var(--danger-surface)",
    );
  });

  it("is under whatever is opened over the window: a dialog, a menu", () => {
    // A pane's corner has a z-index, and a dialog portaled to the body has none.
    expect(rule("#root")).toMatch(/isolation:\s*isolate/);
  });

  it("leaves the window's lines and the Inbox's rows as they were", () => {
    const message = '> [data-scope="button-row"] > [data-scope="notice"][data-part="message"]';
    expect(of(`[data-placement="alone"] ${message}`)).toMatch(/display:\s*inline/);
    // The Inbox's row puts its ways out under the words, in the shared row that wraps.
    expect(of(`[data-placement="inbox"] ${message}`)).toMatch(/flex:\s*1 1 100%/);
    // No rule of the pane's reaches a Notice that is not in one.
    const reach = [...family.matchAll(/([^{}]+)\{[^{}]*\}/g)]
      .map((hit) => hit[1].trim())
      .filter((selector) => /data-placement="pane"|notice-row/.test(selector));
    for (const selector of reach) expect(selector).not.toMatch(/"(inbox|alone)"|notice-list/);
  });
});

describe("Ask {persona}'s rules", () => {
  const css = readFileSync(join(process.cwd(), "src/App.css"), "utf8").replace(
    /\/\*[\s\S]*?\*\//g,
    "",
  );

  it("draws the box you write in as every list field: the row's width, growing with its lines", () => {
    // The shared list field (DS-10, `ListField.test.tsx`) is the whole of it: no rule of the
    // dialog's own sizes it.
    expect(css).not.toMatch(/\.ask-persona[^{]*textarea/);
  });

  it("fits the window and keeps its answers at its bottom edge", () => {
    // Every dialog's, Ask {persona}'s among them (#2910): at most the window's height, its body
    // the part that scrolls, and its answers in the footer under it, always in view.
    const family = readFileSync(join(process.cwd(), "src/ui/dialog/dialog.css"), "utf8");
    const of = (part: string) =>
      new RegExp(`\\[data-scope="dialog"\\]\\[data-part="${part}"\\] \\{([^}]*)\\}`).exec(
        family,
      )?.[1] ?? "";
    expect(of("root")).toMatch(/max-block-size:\s*100%/);
    expect(of("body")).toMatch(/overflow:\s*auto/);
    expect(of("footer")).not.toMatch(/overflow/);
    // Nothing of its own places or sizes it any more.
    expect(css).not.toMatch(/\.warning\.ask-persona/);
    expect(css).not.toMatch(/\.ask-persona \.ui-setting-actions/);
  });
});

describe("a granting Notice on a pane that just moved (#1695, train 54's review)", () => {
  it("holds any answer that grants, whatever its Notice's cause (#3142)", async () => {
    // A vault refusal's Allow is the grants variant, and its cause is in no list: the look is
    // the rule.
    const rect = vi.spyOn(Element.prototype, "getBoundingClientRect").mockImplementation(function (
      this: Element,
    ) {
      const box = this.closest('[data-scope="notice-row"][data-part="stack"] > *');
      const top = box === null ? 0 : [...(box.parentElement?.children ?? [])].indexOf(box) * 40;
      return DOMRect.fromRect({ x: 0, y: top, width: 100, height: 30 });
    });
    let clock = 1_000_000;
    const now = vi.spyOn(performance, "now").mockImplementation(() => clock);
    try {
      const allowed: string[] = [];
      let away: () => void = () => undefined;
      function Pane() {
        const [first, setFirst] = useState(true);
        away = () => setFirst(false);
        return (
          <NoticePaneRow>
            {first && (
              <Notice cause="skills-missing:7" at="pane" label="Skills" onDismiss={() => undefined}>
                A skill is missing.
              </Notice>
            )}
            <Notice
              cause="vault-refused:7:forge"
              at="pane"
              tone="trouble"
              label="Vault"
              fixes={[
                { label: "Allow forge", onPress: () => allowed.push("forge"), allows: true },
                { label: "Keep blocked", onPress: () => undefined },
              ]}
            >
              purlis did not open vault forge.
            </Notice>
          </NoticePaneRow>
        );
      }
      render(<Pane />);
      act(() => away());
      await userEvent.click(screen.getByRole("button", { name: "Allow forge" }));
      expect(allowed).toEqual([]);
      expect(screen.getByText(NOTICE_MOVED_JUST_NOW)).toBeTruthy();

      clock += 2000;
      await userEvent.click(screen.getByRole("button", { name: "Allow forge" }));
      expect(allowed).toEqual(["forge"]);
    } finally {
      now.mockRestore();
      rect.mockRestore();
    }
  });

  it("does nothing for a press on its way out until it settles, and says so", async () => {
    // jsdom lays nothing out: each Notice's box stands at its place among the stack's, and the
    // row at the top. Its `offsetTop` says nothing, as a positioned box makes it say nothing in
    // a real engine (#3142): the guard reads where each one is drawn.
    const rect = vi.spyOn(Element.prototype, "getBoundingClientRect").mockImplementation(function (
      this: Element,
    ) {
      const box = this.closest('[data-scope="notice-row"][data-part="stack"] > *');
      const top = box === null ? 0 : [...(box.parentElement?.children ?? [])].indexOf(box) * 40;
      return DOMRect.fromRect({ x: 0, y: top, width: 100, height: 30 });
    });
    const offset = vi.spyOn(HTMLElement.prototype, "offsetTop", "get").mockReturnValue(4);
    let clock = 1_000_000;
    const now = vi.spyOn(performance, "now").mockImplementation(() => clock);
    try {
      const allowed: string[] = [];
      let keep: () => void = () => undefined;
      function Pane() {
        const [first, setFirst] = useState(true);
        keep = () => setFirst(false);
        const block = (cause: string, name: string) => (
          <Notice
            cause={cause}
            at="pane"
            label="Sandbox block"
            fixes={[{ label: `Allow ${name}`, onPress: () => allowed.push(name) }]}
          >
            {name} was blocked.
          </Notice>
        );
        return (
          <NoticePaneRow>
            {first && block("sandbox-blocked:7:network:connect:chat", "first")}
            {block("sandbox-blocked:8:network:connect:chat", "second")}
          </NoticePaneRow>
        );
      }
      render(<Pane />);
      // Drawn where the person looks, it is pressed at once: arriving is not a move here.
      await userEvent.click(screen.getByRole("button", { name: "Allow second" }));
      expect(allowed).toEqual(["second"]);

      // The one above it is answered and goes: the second moves up under the pointer.
      act(() => keep());
      await userEvent.click(screen.getByRole("button", { name: "Allow second" }));
      expect(allowed).toEqual(["second"]);
      expect(screen.getByText(NOTICE_MOVED_JUST_NOW)).toBeTruthy();
      // The keyboard's press waits as the pointer's does.
      screen.getByRole("button", { name: "Allow second" }).focus();
      await userEvent.keyboard("{Enter}");
      await userEvent.keyboard(" ");
      expect(allowed).toEqual(["second"]);

      clock += 2000;
      await userEvent.click(screen.getByRole("button", { name: "Allow second" }));
      expect(allowed).toEqual(["second", "second"]);
      expect(screen.queryByText(NOTICE_MOVED_JUST_NOW)).toBeNull();
    } finally {
      now.mockRestore();
      rect.mockRestore();
      offset.mockRestore();
    }
  });
});

describe("a granting answer on a pane that moved while its line stood still (#3167)", () => {
  /** The band's scroll, as `band` says it: every other box scrolls nothing. */
  function scrolling(band: { top: number; height: number }) {
    const of = (key: "top" | "height" | "client") =>
      function (this: Element) {
        if (!this.matches('[data-scope="notice-row"][data-part="stack"]')) return 0;
        return key === "client" ? 200 : band[key as "top" | "height"];
      };
    const spies = [
      vi.spyOn(Element.prototype, "scrollTop", "get").mockImplementation(of("top")),
      vi.spyOn(Element.prototype, "scrollHeight", "get").mockImplementation(of("height")),
      vi.spyOn(Element.prototype, "clientHeight", "get").mockImplementation(of("client")),
    ];
    return () => spies.forEach((spy) => spy.mockRestore());
  }

  /** One vault refusal alone in a pane's row, its Allow drawn where `allowAt` says, its line
   *  at the top of the band whatever happens: the guard reads the Allow's own box. */
  function aConsent(allowAt: () => DOMRect, lineTop: () => number = () => 0) {
    const rect = vi.spyOn(Element.prototype, "getBoundingClientRect").mockImplementation(function (
      this: Element,
    ) {
      if (this.closest('[data-variant="grants"]') !== null) return allowAt();
      // What the band holds slides with its scroll; the row and the band themselves do not.
      const inBand = this.closest('[data-scope="notice-row"][data-part="stack"] > *') !== null;
      return DOMRect.fromRect({ x: 0, y: inBand ? lineTop() : 0, width: 300, height: 60 });
    });
    const allowed: string[] = [];
    render(
      <NoticePaneRow>
        <Notice
          cause="vault-refused:7:forge"
          at="pane"
          tone="trouble"
          label="Vault"
          fixes={[
            { label: "Allow forge", onPress: () => allowed.push("forge"), allows: true },
            { label: "Keep blocked", onPress: () => undefined },
          ]}
        >
          purlis did not open vault forge.
        </Notice>
      </NoticePaneRow>,
    );
    const line = screen.getByRole("status", { name: "Vault" });
    return { rect, allowed, line };
  }

  it("holds a press on an Allow its words pushed elsewhere at a new width", async () => {
    // Wide, the Allow sits after the words on the line's second row; narrower, the words take
    // that row and the Allow goes down and to the start. The line's top never moves.
    let wide = true;
    const { rect, allowed, line } = aConsent(() =>
      DOMRect.fromRect(
        wide ? { x: 180, y: 24, width: 90, height: 24 } : { x: 12, y: 48, width: 90, height: 24 },
      ),
    );
    let clock = 1_000_000;
    const now = vi.spyOn(performance, "now").mockImplementation(() => clock);
    try {
      const top = line.getBoundingClientRect().top;
      wide = false;
      expect(line.getBoundingClientRect().top).toBe(top);
      await userEvent.click(screen.getByRole("button", { name: "Allow forge" }));
      expect(allowed).toEqual([]);
      expect(screen.getByText(NOTICE_MOVED_JUST_NOW)).toBeTruthy();

      clock += 2000;
      await userEvent.click(screen.getByRole("button", { name: "Allow forge" }));
      expect(allowed).toEqual(["forge"]);
    } finally {
      now.mockRestore();
      rect.mockRestore();
    }
  });

  it("holds a press after the band was made to scroll back, its place in the band unchanged", async () => {
    // The band is scrolled to its end, 100 down. What is below the consent goes: the band holds
    // less, the browser clamps its scroll to 20, and the consent slides 80 down on screen while
    // its place inside the band stays where it was.
    const band = { top: 100, height: 300 };
    const unscroll = scrolling(band);
    const { rect, allowed } = aConsent(
      () => DOMRect.fromRect({ x: 12, y: 140 - band.top, width: 90, height: 24 }),
      () => 116 - band.top,
    );
    let clock = 1_000_000;
    const now = vi.spyOn(performance, "now").mockImplementation(() => clock);
    try {
      await userEvent.click(screen.getByRole("button", { name: "Allow forge" }));
      expect(allowed).toEqual(["forge"]);

      band.height = 220;
      band.top = 20;
      await userEvent.click(screen.getByRole("button", { name: "Allow forge" }));
      expect(allowed).toEqual(["forge"]);
      expect(screen.getByText(NOTICE_MOVED_JUST_NOW)).toBeTruthy();

      clock += 2000;
      await userEvent.click(screen.getByRole("button", { name: "Allow forge" }));
      expect(allowed).toEqual(["forge", "forge"]);
    } finally {
      now.mockRestore();
      rect.mockRestore();
      unscroll();
    }
  });

  /** The person's own ways to scroll the band (#3217): each is what the browser hears first. */
  const byThePerson: Record<string, (band: Element) => void> = {
    wheel: (band) => band.dispatchEvent(new WheelEvent("wheel", { bubbles: true, deltaY: -80 })),
    "a scroll key": (band) =>
      band.dispatchEvent(new KeyboardEvent("keydown", { bubbles: true, key: "PageUp" })),
    touch: (band) => band.dispatchEvent(new Event("touchstart", { bubbles: true })),
    "its scrollbar": (band) =>
      band.dispatchEvent(new PointerEvent("pointerdown", { bubbles: true, button: 0 })),
  };

  for (const [how, scroll] of Object.entries(byThePerson)) {
    it(`lets a press through after the person scrolled the band themselves (${how})`, async () => {
      // The same 80 on screen, with nothing in the band gone: a scroll the band could still take.
      const band = { top: 100, height: 300 };
      const unscroll = scrolling(band);
      const { rect, allowed } = aConsent(
        () => DOMRect.fromRect({ x: 12, y: 140 - band.top, width: 90, height: 24 }),
        () => 116 - band.top,
      );
      try {
        const stack = document.querySelector(
          '[data-scope="notice-row"][data-part="stack"]',
        ) as HTMLElement;
        scroll(stack);
        band.top = 20;
        await userEvent.click(screen.getByRole("button", { name: "Allow forge" }));
        expect(allowed).toEqual(["forge"]);
        expect(screen.queryByText(NOTICE_MOVED_JUST_NOW)).toBeNull();
      } finally {
        rect.mockRestore();
        unscroll();
      }
    });
  }

  it("holds a press after a scroll of the band the person did not make (#3217)", async () => {
    // The same 80 on screen, the band holding as much as before, and nothing from the person:
    // the window scrolled it (a box brought into view, a focus that scrolled).
    const band = { top: 100, height: 300 };
    const unscroll = scrolling(band);
    const { rect, allowed } = aConsent(
      () => DOMRect.fromRect({ x: 12, y: 140 - band.top, width: 90, height: 24 }),
      () => 116 - band.top,
    );
    let clock = 1_000_000;
    const now = vi.spyOn(performance, "now").mockImplementation(() => clock);
    try {
      band.top = 20;
      await userEvent.click(screen.getByRole("button", { name: "Allow forge" }));
      expect(allowed).toEqual([]);
      expect(screen.getByText(NOTICE_MOVED_JUST_NOW)).toBeTruthy();

      clock += 2000;
      await userEvent.click(screen.getByRole("button", { name: "Allow forge" }));
      expect(allowed).toEqual(["forge"]);
    } finally {
      now.mockRestore();
      rect.mockRestore();
      unscroll();
    }
  });

  it("holds a press after a scroll that came long after the person's last scroll (#3217)", async () => {
    const band = { top: 100, height: 300 };
    const unscroll = scrolling(band);
    const { rect, allowed } = aConsent(
      () => DOMRect.fromRect({ x: 12, y: 140 - band.top, width: 90, height: 24 }),
      () => 116 - band.top,
    );
    let clock = 1_000_000;
    const now = vi.spyOn(performance, "now").mockImplementation(() => clock);
    try {
      const stack = document.querySelector(
        '[data-scope="notice-row"][data-part="stack"]',
      ) as HTMLElement;
      byThePerson.wheel(stack);
      clock += 5000;
      band.top = 20;
      await userEvent.click(screen.getByRole("button", { name: "Allow forge" }));
      expect(allowed).toEqual([]);
    } finally {
      now.mockRestore();
      rect.mockRestore();
      unscroll();
    }
  });

  it("holds a press after a scroll that follows a key that scrolls nothing (#3217)", async () => {
    const band = { top: 100, height: 300 };
    const unscroll = scrolling(band);
    const { rect, allowed } = aConsent(
      () => DOMRect.fromRect({ x: 12, y: 140 - band.top, width: 90, height: 24 }),
      () => 116 - band.top,
    );
    const clock = 1_000_000;
    const now = vi.spyOn(performance, "now").mockImplementation(() => clock);
    try {
      const stack = document.querySelector(
        '[data-scope="notice-row"][data-part="stack"]',
      ) as HTMLElement;
      stack.dispatchEvent(new KeyboardEvent("keydown", { bubbles: true, key: "Escape" }));
      // A press on a Notice's way out is no scroll of the band either.
      screen
        .getByRole("button", { name: "Keep blocked" })
        .dispatchEvent(new PointerEvent("pointerdown", { bubbles: true, button: 0 }));
      band.top = 20;
      await userEvent.click(screen.getByRole("button", { name: "Allow forge" }));
      expect(allowed).toEqual([]);
    } finally {
      now.mockRestore();
      rect.mockRestore();
      unscroll();
    }
  });

  it("dates a move from the frame after it, not from the press: a settled Allow is not held", async () => {
    // The window is narrowed and the Allow moves; its press comes well after. The frame after
    // the resize read the move then, so the press is not held for a move it did not follow.
    let wide = true;
    const { rect, allowed } = aConsent(() =>
      DOMRect.fromRect(
        wide ? { x: 180, y: 24, width: 90, height: 24 } : { x: 12, y: 48, width: 90, height: 24 },
      ),
    );
    let clock = 1_000_000;
    const now = vi.spyOn(performance, "now").mockImplementation(() => clock);
    const aFrame = () => act(() => new Promise((done) => setTimeout(done, 50)));
    try {
      // Whatever the first drawing asked to read again is read first.
      await aFrame();
      wide = false;
      window.dispatchEvent(new Event("resize"));
      await aFrame();
      clock += 2000;
      await userEvent.click(screen.getByRole("button", { name: "Allow forge" }));
      expect(allowed).toEqual(["forge"]);
    } finally {
      now.mockRestore();
      rect.mockRestore();
    }
  });

  it("holds an Allow drawn into a Notice that already stood, where another way out was", async () => {
    const allowed: string[] = [];
    let ask: () => void = () => undefined;
    function Pane() {
      const [asks, setAsks] = useState(false);
      ask = () => setAsks(true);
      return (
        <NoticePaneRow>
          <Notice
            cause="vault-refused:7:forge"
            at="pane"
            label="Vault"
            fixes={
              asks
                ? [
                    { label: "Allow forge", onPress: () => allowed.push("forge"), allows: true },
                    { label: "Keep blocked", onPress: () => undefined },
                  ]
                : [{ label: "Keep blocked", onPress: () => undefined }]
            }
          >
            purlis did not open vault forge.
          </Notice>
        </NoticePaneRow>
      );
    }
    let clock = 1_000_000;
    const now = vi.spyOn(performance, "now").mockImplementation(() => clock);
    try {
      render(<Pane />);
      act(() => ask());
      await userEvent.click(screen.getByRole("button", { name: "Allow forge" }));
      expect(allowed).toEqual([]);

      clock += 2000;
      await userEvent.click(screen.getByRole("button", { name: "Allow forge" }));
      expect(allowed).toEqual(["forge"]);
    } finally {
      now.mockRestore();
    }
  });

  it("holds a press on an Allow that drifted in steps each too small to count", async () => {
    // Each read sees it 0.4 px from the last, under the tolerance; together they carry it 40 px.
    let x = 12;
    const { rect, allowed } = aConsent(() => DOMRect.fromRect({ x, y: 24, width: 90, height: 24 }));
    const aFrame = () => act(() => new Promise((done) => setTimeout(done, 20)));
    try {
      await aFrame();
      for (let i = 0; i < 100; i++) {
        x += 0.4;
        window.dispatchEvent(new Event("resize"));
        await aFrame();
      }
      await userEvent.click(screen.getByRole("button", { name: "Allow forge" }));
      expect(allowed).toEqual([]);
      expect(screen.getByText(NOTICE_MOVED_JUST_NOW)).toBeTruthy();
    } finally {
      rect.mockRestore();
    }
  });

  it("holds a press on an Allow that grew under the pointer", async () => {
    let wide = false;
    const { rect, allowed } = aConsent(() =>
      DOMRect.fromRect({ x: 12, y: 24, width: wide ? 160 : 90, height: 24 }),
    );
    try {
      wide = true;
      await userEvent.click(screen.getByRole("button", { name: "Allow forge" }));
      expect(allowed).toEqual([]);
    } finally {
      rect.mockRestore();
    }
  });
});

describe("a granting Notice that arrives in a pane's band (#3167, D-3167-5)", () => {
  type Shown = "none" | "warning" | "both" | "consent";
  function Band({ shown, allowed }: { shown: Shown; allowed: string[] }) {
    return (
      <NoticePaneRow>
        {(shown === "warning" || shown === "both") && (
          <Notice cause="skills-missing:7" at="pane" label="Skills" onDismiss={() => undefined}>
            A skill is missing.
          </Notice>
        )}
        {(shown === "both" || shown === "consent") && (
          <Notice
            cause="vault-refused:7:forge"
            at="pane"
            tone="trouble"
            label="Vault"
            fixes={[
              { label: "Allow forge", onPress: () => allowed.push("forge"), allows: true },
              { label: "Keep blocked", onPress: () => undefined },
            ]}
          >
            purlis did not open vault forge.
          </Notice>
        )}
      </NoticePaneRow>
    );
  }

  it("stands as it arrives in an empty band: nothing else there is aimed at", async () => {
    const allowed: string[] = [];
    const { rerender } = render(<Band shown="none" allowed={allowed} />);
    rerender(<Band shown="consent" allowed={allowed} />);
    await userEvent.click(screen.getByRole("button", { name: "Allow forge" }));
    expect(allowed).toEqual(["forge"]);
  });

  it("holds a consent drawn in the same render that took away a warning that stood long", async () => {
    let clock = 1_000_000;
    const now = vi.spyOn(performance, "now").mockImplementation(() => clock);
    try {
      const allowed: string[] = [];
      const { rerender } = render(<Band shown="warning" allowed={allowed} />);
      // The warning stands still for 10 s, and nothing reads the band in between.
      clock += 10_000;
      rerender(<Band shown="consent" allowed={allowed} />);
      await userEvent.click(screen.getByRole("button", { name: "Allow forge" }));
      expect(allowed).toEqual([]);
      expect(screen.getByText(NOTICE_MOVED_JUST_NOW)).toBeTruthy();
    } finally {
      now.mockRestore();
    }
  });

  it("holds a consent drawn just after a warning that stood long was put away", async () => {
    let clock = 1_000_000;
    const now = vi.spyOn(performance, "now").mockImplementation(() => clock);
    try {
      const allowed: string[] = [];
      const { rerender } = render(<Band shown="warning" allowed={allowed} />);
      clock += 10_000;
      rerender(<Band shown="none" allowed={allowed} />);
      clock += 200;
      rerender(<Band shown="consent" allowed={allowed} />);
      await userEvent.click(screen.getByRole("button", { name: "Allow forge" }));
      expect(allowed).toEqual([]);

      // Once it has stood for the settle time, the same press allows.
      clock += 2000;
      await userEvent.click(screen.getByRole("button", { name: "Allow forge" }));
      expect(allowed).toEqual(["forge"]);
    } finally {
      now.mockRestore();
    }
  });

  it("holds its Allow when it arrives beside another Notice, or where one just left", async () => {
    let clock = 1_000_000;
    const now = vi.spyOn(performance, "now").mockImplementation(() => clock);
    try {
      const allowed: string[] = [];
      const { rerender } = render(<Band shown="warning" allowed={allowed} />);
      rerender(<Band shown="both" allowed={allowed} />);
      await userEvent.click(screen.getByRole("button", { name: "Allow forge" }));
      expect(allowed).toEqual([]);
      expect(screen.getByText(NOTICE_MOVED_JUST_NOW)).toBeTruthy();
      clock += 2000;
      await userEvent.click(screen.getByRole("button", { name: "Allow forge" }));
      expect(allowed).toEqual(["forge"]);

      // The band empties, and a consent arrives at once where the warning was.
      rerender(<Band shown="warning" allowed={allowed} />);
      rerender(<Band shown="none" allowed={allowed} />);
      clock += 200;
      rerender(<Band shown="consent" allowed={allowed} />);
      expect(screen.queryByText(NOTICE_MOVED_JUST_NOW)).toBeNull();
      await userEvent.click(screen.getByRole("button", { name: "Allow forge" }));
      expect(allowed).toEqual(["forge"]);
      expect(screen.getByText(NOTICE_MOVED_JUST_NOW)).toBeTruthy();
    } finally {
      now.mockRestore();
    }
  });
});

describe("a pane's band drawn anew, by another component (#3167, D-3167-6)", () => {
  /** How the band's pane is drawn around it: each a different tree, so the band is drawn anew. */
  type Around = "live" | "ended" | "split" | "away";
  type Shown = "warning" | "consent" | "both" | "first";
  let named = 0;
  /** A pane no test has drawn before. */
  const aPane = () => paneBand("test-plane", 10_000 + named++);

  function Band({ shown, allowed }: { shown: Shown; allowed: string[] }) {
    return (
      <>
        {(shown === "warning" || shown === "both") && (
          <Notice cause="skills-missing:9" at="pane" label="Skills" onDismiss={() => undefined}>
            A skill is missing.
          </Notice>
        )}
        {shown !== "warning" && (
          <Notice
            cause="vault-refused:7:forge"
            at="pane"
            tone="trouble"
            label="Vault"
            fixes={[
              { label: "Allow forge", onPress: () => allowed.push("forge"), allows: true },
              { label: "Keep blocked", onPress: () => undefined },
            ]}
          >
            purlis did not open vault forge.
          </Notice>
        )}
      </>
    );
  }

  /** As `LayoutPanes` draws a pane: by `PaneFrame` while its task is live, by a plain frame once
   *  it ended or moved, inside a split's group once the tab is split, and not at all while
   *  another tab is in front. */
  function Pane({
    band,
    around,
    shown,
    allowed,
  }: {
    band: string;
    around: Around;
    shown: Shown;
    allowed: string[];
  }) {
    const row = (
      <NoticePaneRow band={band}>
        <Band shown={shown} allowed={allowed} />
      </NoticePaneRow>
    );
    if (around === "away") return <p>Another tab is in front.</p>;
    if (around === "ended") return <div className="pane-frame pane-headed">{row}</div>;
    if (around === "split") {
      return (
        <div data-group="">
          <div data-panel="">
            <section className="pane-frame pane-headed">{row}</section>
          </div>
        </div>
      );
    }
    return <section className="pane-frame pane-headed">{row}</section>;
  }

  /** The pane drawn `from`, its warning standing 10 s, then drawn `to` with a consent where the
   *  warning stood. */
  async function drawnAnew(from: Around, to: Around, shown: Shown = "consent") {
    const band = aPane();
    const allowed: string[] = [];
    const { rerender } = render(
      <Pane band={band} around={from} shown="warning" allowed={allowed} />,
    );
    clock.now += 10_000;
    rerender(<Pane band={band} around={to} shown={shown} allowed={allowed} />);
    return { band, allowed, rerender };
  }

  const clock = { now: 1_000_000 };
  let spy: { mockRestore: () => void } | undefined;
  const timed = () => {
    clock.now = 1_000_000;
    spy = vi.spyOn(performance, "now").mockImplementation(() => clock.now);
  };
  afterEach(() => spy?.mockRestore());

  it("holds a consent drawn where a warning stood once the pane's task ended", async () => {
    timed();
    const { allowed } = await drawnAnew("live", "ended");
    await userEvent.click(screen.getByRole("button", { name: "Allow forge" }));
    expect(allowed).toEqual([]);
    expect(screen.getByText(NOTICE_MOVED_JUST_NOW)).toBeTruthy();
    clock.now += 2000;
    await userEvent.click(screen.getByRole("button", { name: "Allow forge" }));
    expect(allowed).toEqual(["forge"]);
  });

  it("holds it from the keyboard as from the pointer", async () => {
    timed();
    const { allowed } = await drawnAnew("live", "ended");
    screen.getByRole("button", { name: "Allow forge" }).focus();
    await userEvent.keyboard("{Enter}");
    await userEvent.keyboard(" ");
    expect(allowed).toEqual([]);
  });

  it("holds it when the pane's task moved, and when it came back", async () => {
    timed();
    const { band, allowed, rerender } = await drawnAnew("live", "ended");
    await userEvent.click(screen.getByRole("button", { name: "Allow forge" }));
    expect(allowed).toEqual([]);
    // Drawn by its live frame again, with the warning back in the consent's place.
    clock.now += 10_000;
    rerender(<Pane band={band} around="live" shown="warning" allowed={allowed} />);
    clock.now += 10_000;
    rerender(<Pane band={band} around="ended" shown="consent" allowed={allowed} />);
    await userEvent.click(screen.getByRole("button", { name: "Allow forge" }));
    expect(allowed).toEqual([]);
  });

  it("holds it when a split formed, and when the split went", async () => {
    timed();
    const formed = await drawnAnew("live", "split");
    await userEvent.click(screen.getByRole("button", { name: "Allow forge" }));
    expect(formed.allowed).toEqual([]);
    cleanup();
    const went = await drawnAnew("split", "live");
    await userEvent.click(screen.getByRole("button", { name: "Allow forge" }));
    expect(went.allowed).toEqual([]);
  });

  it("holds it when its tab comes back to the front with a consent where the warning was", async () => {
    timed();
    const band = aPane();
    const allowed: string[] = [];
    const { rerender } = render(
      <Pane band={band} around="live" shown="warning" allowed={allowed} />,
    );
    clock.now += 10_000;
    rerender(<Pane band={band} around="away" shown="warning" allowed={allowed} />);
    clock.now += 10_000;
    rerender(<Pane band={band} around="live" shown="consent" allowed={allowed} />);
    await userEvent.click(screen.getByRole("button", { name: "Allow forge" }));
    expect(allowed).toEqual([]);
  });

  it("holds a consent that stood below the warning, read against where it was", async () => {
    // The consent stood under the warning; the warning goes as the pane is drawn anew, and the
    // consent is drawn 40 px higher, where the warning's way out was.
    let up = false;
    const rect = vi.spyOn(Element.prototype, "getBoundingClientRect").mockImplementation(function (
      this: Element,
    ) {
      if (this.closest('[data-variant="grants"]') === null) {
        return DOMRect.fromRect({ x: 0, y: 0, width: 300, height: 120 });
      }
      return DOMRect.fromRect({ x: 12, y: up ? 24 : 64, width: 90, height: 24 });
    });
    timed();
    try {
      const band = aPane();
      const allowed: string[] = [];
      const { rerender } = render(
        <Pane band={band} around="live" shown="both" allowed={allowed} />,
      );
      clock.now += 10_000;
      up = true;
      rerender(<Pane band={band} around="ended" shown="consent" allowed={allowed} />);
      await userEvent.click(screen.getByRole("button", { name: "Allow forge" }));
      expect(allowed).toEqual([]);
    } finally {
      rect.mockRestore();
    }
  });

  it("lets a consent that stood still through the redrawing be pressed: nothing moved", async () => {
    const rect = vi
      .spyOn(Element.prototype, "getBoundingClientRect")
      .mockImplementation(() => DOMRect.fromRect({ x: 12, y: 24, width: 90, height: 24 }));
    timed();
    try {
      const band = aPane();
      const allowed: string[] = [];
      const { rerender } = render(
        <Pane band={band} around="live" shown="consent" allowed={allowed} />,
      );
      clock.now += 10_000;
      rerender(<Pane band={band} around="split" shown="consent" allowed={allowed} />);
      await userEvent.click(screen.getByRole("button", { name: "Allow forge" }));
      expect(allowed).toEqual(["forge"]);
    } finally {
      rect.mockRestore();
    }
  });

  it("lets the first Notice of a pane never drawn before be pressed at once", async () => {
    timed();
    const allowed: string[] = [];
    render(<Pane band={aPane()} around="live" shown="consent" allowed={allowed} />);
    await userEvent.click(screen.getByRole("button", { name: "Allow forge" }));
    expect(allowed).toEqual(["forge"]);
  });

  it("forgets a pane closed for good: drawn again, it is a pane never drawn before", async () => {
    timed();
    const band = aPane();
    const allowed: string[] = [];
    const { unmount } = render(
      <Pane band={band} around="live" shown="warning" allowed={allowed} />,
    );
    unmount();
    forgetPaneBand(band);
    clock.now += 10_000;
    render(<Pane band={band} around="live" shown="consent" allowed={allowed} />);
    await userEvent.click(screen.getByRole("button", { name: "Allow forge" }));
    expect(allowed).toEqual(["forge"]);
  });
});

describe("a look-alike grant drawn in the place of one that stood (#3167, D-3167-10)", () => {
  // Every box in one place, as two look-alike Notices drawn in the same slot are.
  const clock = { now: 1_000_000 };
  let spies: { mockRestore: () => void }[] = [];
  const timedAndStill = () => {
    clock.now = 1_000_000;
    spies = [
      vi.spyOn(performance, "now").mockImplementation(() => clock.now),
      vi
        .spyOn(Element.prototype, "getBoundingClientRect")
        .mockImplementation(() => DOMRect.fromRect({ x: 12, y: 24, width: 90, height: 24 })),
    ];
  };
  afterEach(() => {
    for (const spy of spies) spy.mockRestore();
    spies = [];
  });

  /** A block of the chat's, as the sandbox's Notice draws one: one cause for the chat's
   *  writes, whatever path each names, so two of them differ only in what they say. */
  function Block({ target, allowed }: { target: string; allowed: string[] }) {
    return (
      <Notice
        cause="sandbox-blocked:7:write:path:chat"
        at="pane"
        tone="trouble"
        label="Sandbox block"
        fixes={[
          { label: "Allow for this chat", onPress: () => allowed.push(target), allows: true },
          { label: "Keep blocked", onPress: () => undefined },
        ]}
      >
        The sandbox blocked writing {target}.
      </Notice>
    );
  }

  function Band({ target, allowed, band }: { target: string; allowed: string[]; band?: string }) {
    return (
      <NoticePaneRow band={band}>
        <Notice cause="skills-missing:9" at="pane" label="Skills" onDismiss={() => undefined}>
          A skill is missing.
        </Notice>
        <Block key={target} target={target} allowed={allowed} />
      </NoticePaneRow>
    );
  }

  let named = 0;
  for (const naming of ["unnamed", "named"] as const) {
    it(`holds one that replaces it in one render, the band drawn on (${naming} band)`, async () => {
      timedAndStill();
      const band = naming === "named" ? paneBand("look-alike", named++) : undefined;
      const allowed: string[] = [];
      const { rerender } = render(<Band target="~/proj/out" allowed={allowed} band={band} />);
      clock.now += 10_000;
      rerender(<Band target="~/.ssh" allowed={allowed} band={band} />);
      await userEvent.click(screen.getByRole("button", { name: "Allow for this chat" }));
      expect(allowed).toEqual([]);
      expect(screen.getByText(NOTICE_MOVED_JUST_NOW)).toBeTruthy();
    });
  }

  it("holds one that replaces it as the pane's band is drawn anew", async () => {
    timedAndStill();
    const band = paneBand("look-alike", named++);
    const allowed: string[] = [];
    const { rerender } = render(
      <section className="pane-frame pane-headed">
        <Band target="~/proj/out" allowed={allowed} band={band} />
      </section>,
    );
    clock.now += 10_000;
    rerender(
      <div className="pane-frame pane-headed">
        <Band target="~/.ssh" allowed={allowed} band={band} />
      </div>,
    );
    await userEvent.click(screen.getByRole("button", { name: "Allow for this chat" }));
    expect(allowed).toEqual([]);
  });

  it("holds one that replaces it in a list whose every first drawing waits (the Inbox's)", async () => {
    timedAndStill();
    const allowed: string[] = [];
    function Listed({ target }: { target: string }) {
      const { at, allows, said } = useNoticeSettle<HTMLDivElement>("moves");
      return (
        <NoticeWayGuard value={allows}>
          <div ref={at}>
            <Block key={target} target={target} allowed={allowed} />
          </div>
          {said !== undefined && <p>{said}</p>}
        </NoticeWayGuard>
      );
    }
    const { rerender } = render(<Listed target="~/proj/out" />);
    clock.now += 10_000;
    rerender(<Listed target="~/.ssh" />);
    await userEvent.click(screen.getByRole("button", { name: "Allow for this chat" }));
    expect(allowed).toEqual([]);
    expect(screen.getByText(NOTICE_MOVED_JUST_NOW)).toBeTruthy();
  });

  it("lets the same grant that stood still through a redrawing be pressed", async () => {
    timedAndStill();
    const band = paneBand("look-alike", named++);
    const allowed: string[] = [];
    const { rerender } = render(
      <section className="pane-frame pane-headed">
        <Band target="~/proj/out" allowed={allowed} band={band} />
      </section>,
    );
    clock.now += 10_000;
    rerender(
      <div className="pane-frame pane-headed">
        <Band target="~/proj/out" allowed={allowed} band={band} />
      </div>,
    );
    await userEvent.click(screen.getByRole("button", { name: "Allow for this chat" }));
    expect(allowed).toEqual(["~/proj/out"]);
  });
});

describe("a hold measured on a clock that only goes forward (#3227)", () => {
  const HOUR = 3_600_000;
  /** A consent that moves up under the pointer, as the warning above it goes; the wall clock and
   *  the monotonic one each driven by the test. */
  function aMovedConsent() {
    const rect = vi.spyOn(Element.prototype, "getBoundingClientRect").mockImplementation(function (
      this: Element,
    ) {
      const box = this.closest('[data-scope="notice-row"][data-part="stack"] > *');
      const top = box === null ? 0 : [...(box.parentElement?.children ?? [])].indexOf(box) * 40;
      return DOMRect.fromRect({ x: 0, y: top, width: 100, height: 30 });
    });
    const clocks = { wall: 1_800_000_000_000, mono: 50_000 };
    const wall = vi.spyOn(Date, "now").mockImplementation(() => clocks.wall);
    const mono = vi.spyOn(performance, "now").mockImplementation(() => clocks.mono);
    const allowed: string[] = [];
    let away: () => void = () => undefined;
    function Pane() {
      const [first, setFirst] = useState(true);
      away = () => setFirst(false);
      return (
        <NoticePaneRow>
          {first && (
            <Notice cause="skills-missing:7" at="pane" label="Skills" onDismiss={() => undefined}>
              A skill is missing.
            </Notice>
          )}
          <Notice
            cause="vault-refused:7:forge"
            at="pane"
            tone="trouble"
            label="Vault"
            fixes={[{ label: "Allow forge", onPress: () => allowed.push("forge"), allows: true }]}
          >
            purlis did not open vault forge.
          </Notice>
        </NoticePaneRow>
      );
    }
    render(<Pane />);
    // Stood long, then moved up under the pointer.
    clocks.wall += 10_000;
    clocks.mono += 10_000;
    act(() => away());
    const allow = () => screen.getByRole("button", { name: "Allow forge" });
    const restore = () => [rect, wall, mono].forEach((spy) => spy.mockRestore());
    return { clocks, allowed, allow, restore };
  }

  it("lets the press through once the settle time has passed, though the wall clock went back", async () => {
    const { clocks, allowed, allow, restore } = aMovedConsent();
    try {
      clocks.wall -= HOUR;
      await userEvent.click(allow());
      expect(allowed).toEqual([]);
      clocks.wall += 2000;
      clocks.mono += 2000;
      await userEvent.click(allow());
      expect(allowed).toEqual(["forge"]);
    } finally {
      restore();
    }
  });

  it("still holds the press when the wall clock jumped forward", async () => {
    const { clocks, allowed, allow, restore } = aMovedConsent();
    try {
      clocks.wall += HOUR;
      clocks.mono += 100;
      await userEvent.click(allow());
      expect(allowed).toEqual([]);
      expect(screen.getByText(NOTICE_MOVED_JUST_NOW)).toBeTruthy();
    } finally {
      restore();
    }
  });
});

describe("a pane's band read only once its boxes are drawn (#3302)", () => {
  // As a real engine lays the band out: the row is not drawn while it is empty
  // (the row's `data-empty`, display: none), so every box in it is 0x0 then; once drawn,
  // every answer stands in one place.
  const clock = { now: 1_000_000 };
  let spies: { mockRestore: () => void }[] = [];
  const laidOut = () => {
    clock.now = 1_000_000;
    spies = [
      vi.spyOn(performance, "now").mockImplementation(() => clock.now),
      vi.spyOn(Element.prototype, "getBoundingClientRect").mockImplementation(function (
        this: Element,
      ) {
        if (this.closest('[data-scope="notice-row"][data-empty]') !== null)
          return DOMRect.fromRect();
        return DOMRect.fromRect({ x: 12, y: 24, width: 90, height: 24 });
      }),
    ];
  };
  afterEach(() => {
    for (const spy of spies) spy.mockRestore();
    spies = [];
  });

  function Block({ target, allowed }: { target: string; allowed: string[] }) {
    return (
      <Notice
        cause="sandbox-blocked:7:write:path:chat"
        at="pane"
        tone="trouble"
        label="Sandbox block"
        fixes={[
          { label: "Allow for this chat", onPress: () => allowed.push(target), allows: true },
          { label: "Keep blocked", onPress: () => undefined },
        ]}
      >
        The sandbox blocked writing {target}.
      </Notice>
    );
  }
  const Warning = () => (
    <Notice cause="skills-missing:9" at="pane" label="Skills" onDismiss={() => undefined}>
      A skill is missing.
    </Notice>
  );
  function Pane({
    shape,
    band,
    children,
  }: {
    shape: "frame" | "div";
    band?: string;
    children?: React.ReactNode;
  }) {
    const row = <NoticePaneRow band={band}>{children}</NoticePaneRow>;
    return shape === "frame" ? (
      <section className="pane-frame pane-headed">{row}</section>
    ) : (
      <div className="pane-frame pane-headed">{row}</div>
    );
  }
  const allow = () => screen.getByRole("button", { name: "Allow for this chat" });
  let named = 0;
  const aPane = () => paneBand("drawn-boxes", named++);

  it("lets the first Notice into an empty band be pressed at once", async () => {
    laidOut();
    const allowed: string[] = [];
    const { rerender } = render(<Pane shape="frame" />);
    clock.now += 10_000;
    rerender(
      <Pane shape="frame">
        <Block target="~/a" allowed={allowed} />
      </Pane>,
    );
    await userEvent.click(allow());
    expect(allowed).toEqual(["~/a"]);
  });

  it("lets the first Notice of a pane never drawn before be pressed at once", async () => {
    laidOut();
    const allowed: string[] = [];
    render(
      <Pane shape="frame" band={aPane()}>
        <Block target="~/a" allowed={allowed} />
      </Pane>,
    );
    await userEvent.click(allow());
    expect(allowed).toEqual(["~/a"]);
  });

  it("lets a grant that stood still through a redrawing be pressed at once", async () => {
    laidOut();
    const band = aPane();
    const allowed: string[] = [];
    const { rerender } = render(
      <Pane shape="frame" band={band}>
        <Block target="~/a" allowed={allowed} />
      </Pane>,
    );
    clock.now += 10_000;
    rerender(
      <Pane shape="div" band={band}>
        <Block target="~/a" allowed={allowed} />
      </Pane>,
    );
    await userEvent.click(allow());
    expect(allowed).toEqual(["~/a"]);
  });

  it("still holds a grant drawn where a warning stood as the band is drawn anew", async () => {
    laidOut();
    const band = aPane();
    const allowed: string[] = [];
    const { rerender } = render(
      <Pane shape="frame" band={band}>
        <Warning />
      </Pane>,
    );
    clock.now += 10_000;
    rerender(
      <Pane shape="div" band={band}>
        <Block target="~/a" allowed={allowed} />
      </Pane>,
    );
    await userEvent.click(allow());
    expect(allowed).toEqual([]);
    expect(screen.getByText(NOTICE_MOVED_JUST_NOW)).toBeTruthy();
    clock.now += 2000;
    await userEvent.click(allow());
    expect(allowed).toEqual(["~/a"]);
  });

  it("still holds a grant that arrives just after the band's last warning went", async () => {
    laidOut();
    const allowed: string[] = [];
    const { rerender } = render(
      <Pane shape="frame">
        <Warning />
      </Pane>,
    );
    clock.now += 10_000;
    rerender(<Pane shape="frame" />);
    await act(async () => {
      await new Promise((done) => setTimeout(done, 30));
    });
    clock.now += 200;
    rerender(
      <Pane shape="frame">
        <Block target="~/a" allowed={allowed} />
      </Pane>,
    );
    await userEvent.click(allow());
    expect(allowed).toEqual([]);
  });

  it("holds a press made before the band was ever read", async () => {
    laidOut();
    const allowed: string[] = [];
    const seen: boolean[] = [];
    function Peek() {
      const allows = useContext(NoticeWayGuard);
      useLayoutEffect(() => {
        const way = document.querySelector('[data-variant="grants"]');
        if (way !== null) seen.push(allows(way));
      }, [allows]);
      return null;
    }
    render(
      <Pane shape="frame" band={aPane()}>
        <Block target="~/a" allowed={allowed} />
        <Peek />
      </Pane>,
    );
    expect(seen[0]).toBe(false);
  });

  // Only the redrawing's own first read recalls a place (D-3167-10): a new grant that says the
  // very same words, drawn in a later read where one stood, is new.
  for (const naming of ["unnamed", "named"] as const) {
    it(`holds a grant with the same words that replaces one in a later read (${naming} band)`, async () => {
      laidOut();
      const band = naming === "named" ? aPane() : undefined;
      const allowed: string[] = [];
      const { rerender } = render(
        <Pane shape="frame" band={band}>
          <Block key="one" target="~/a" allowed={allowed} />
        </Pane>,
      );
      clock.now += 10_000;
      rerender(
        <Pane shape="frame" band={band}>
          <Block key="two" target="~/a" allowed={allowed} />
        </Pane>,
      );
      await userEvent.click(allow());
      expect(allowed).toEqual([]);
    });
  }

  it("holds a grant with the same words that replaces one in the Inbox's list", async () => {
    laidOut();
    const allowed: string[] = [];
    function Listed({ which }: { which: string }) {
      const { at, allows } = useNoticeSettle<HTMLDivElement>("moves");
      return (
        <NoticeWayGuard value={allows}>
          <div ref={at}>
            <Block key={which} target="~/a" allowed={allowed} />
          </div>
        </NoticeWayGuard>
      );
    }
    const { rerender } = render(<Listed which="one" />);
    clock.now += 10_000;
    rerender(<Listed which="two" />);
    await userEvent.click(allow());
    expect(allowed).toEqual([]);
  });
});

describe("a task's band memory carried to the pane it moved to (#3271)", () => {
  const clock = { now: 1_000_000 };
  let spies: { mockRestore: () => void }[] = [];
  const laidOut = () => {
    clock.now = 1_000_000;
    spies = [
      vi.spyOn(performance, "now").mockImplementation(() => clock.now),
      vi.spyOn(Element.prototype, "getBoundingClientRect").mockImplementation(function (
        this: Element,
      ) {
        if (this.closest('[data-scope="notice-row"][data-empty]') !== null)
          return DOMRect.fromRect();
        return DOMRect.fromRect({ x: 12, y: 24, width: 90, height: 24 });
      }),
    ];
  };
  afterEach(() => {
    for (const spy of spies) spy.mockRestore();
    spies = [];
  });

  function Block({ target, allowed }: { target: string; allowed: string[] }) {
    return (
      <Notice
        cause="sandbox-blocked:4:write:path:chat"
        at="pane"
        tone="trouble"
        label="Sandbox block"
        fixes={[
          { label: "Allow for this chat", onPress: () => allowed.push(target), allows: true },
        ]}
      >
        The sandbox blocked writing {target}.
      </Notice>
    );
  }
  const Warning = () => (
    <Notice cause="mcp-failed:4:figma" at="pane" label="MCP server" onDismiss={() => undefined}>
      MCP server figma did not start.
    </Notice>
  );
  let named = 0;
  const allow = () => screen.getByRole("button", { name: "Allow for this chat" });

  /** The task's warning stood in its session's pane `from`; the task is moved to pane `to`,
   *  where a consent is drawn in the warning's place. */
  function moved(carried: boolean) {
    const from = paneBand("moved", named++);
    const to = paneBand("moved", named++);
    const allowed: string[] = [];
    const { unmount } = render(
      <NoticePaneRow band={from}>
        <Warning />
      </NoticePaneRow>,
    );
    clock.now += 10_000;
    if (carried) carryPaneBand(from, to);
    unmount();
    render(
      <NoticePaneRow band={to}>
        <Block target="~/a" allowed={allowed} />
      </NoticePaneRow>,
    );
    return allowed;
  }

  it("holds a consent drawn where the task's warning stood, in the pane it moved to", async () => {
    laidOut();
    const allowed = moved(true);
    await userEvent.click(allow());
    expect(allowed).toEqual([]);
    expect(screen.getByText(NOTICE_MOVED_JUST_NOW)).toBeTruthy();
    clock.now += 2000;
    await userEvent.click(allow());
    expect(allowed).toEqual(["~/a"]);
  });

  it("lets a pane that is new, with nothing carried to it, stand its first Notice", async () => {
    laidOut();
    const allowed = moved(false);
    await userEvent.click(allow());
    expect(allowed).toEqual(["~/a"]);
  });

  it("lets the same grant, where it stood, be pressed at once in the pane it moved to", async () => {
    laidOut();
    const from = paneBand("moved", named++);
    const to = paneBand("moved", named++);
    const allowed: string[] = [];
    const { unmount } = render(
      <NoticePaneRow band={from}>
        <Block target="~/a" allowed={allowed} />
      </NoticePaneRow>,
    );
    clock.now += 10_000;
    carryPaneBand(from, to);
    unmount();
    render(
      <NoticePaneRow band={to}>
        <Block target="~/a" allowed={allowed} />
      </NoticePaneRow>,
    );
    await userEvent.click(allow());
    expect(allowed).toEqual(["~/a"]);
  });

  it("only adds to a band that has read already: what it held still holds", async () => {
    laidOut();
    const from = paneBand("moved", named++);
    const to = paneBand("moved", named++);
    const allowed: string[] = [];
    // The pane it left: its warning went long ago, so its band remembers nothing standing.
    const left = render(
      <NoticePaneRow band={from}>
        <Warning />
      </NoticePaneRow>,
    );
    left.rerender(<NoticePaneRow band={from}>{null}</NoticePaneRow>);
    const { unmount } = left;
    const into = render(
      <NoticePaneRow band={to}>
        <Warning />
      </NoticePaneRow>,
    );
    clock.now += 10_000;
    carryPaneBand(from, to);
    unmount();
    into.rerender(
      <NoticePaneRow band={to}>
        <Block target="~/a" allowed={allowed} />
      </NoticePaneRow>,
    );
    await userEvent.click(allow());
    expect(allowed).toEqual([]);
  });
});
