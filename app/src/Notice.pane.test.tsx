// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { readFileSync } from "node:fs";
import { join } from "node:path";
import { afterEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import { Notice, NoticePaneRow } from "./Notice";
import { NOTICE_MOVED_JUST_NOW } from "./noticeSettle";
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
    expect(box).toHaveClass("notice-pane-box", "notice-opened");
    expect([...(box?.children ?? [])].map((one) => one.className)).toEqual([
      "notice notice-pane notice-trouble",
      "notice-under notice-under-pane",
    ]);
    // What it opened is outside the line: the line is a live region, and a form in one would
    // be read again on every keystroke.
    const line = screen.getByRole("status", { name: "Vault" });
    expect(line).not.toContainElement(screen.getByText("Allow lets every chat…"));
  });

  it("draws the sentence first on its line, then every way out, in order", () => {
    aPaneNotice();

    const line = screen.getByRole("status", { name: "Vault" });
    expect(line).toHaveAttribute("data-cause", "vault-refused:7:devops");
    expect([...line.children].map((one) => one.className.split(" ")[0] || one.tagName)).toEqual([
      "notice-says",
      "notice-fix",
      "notice-fix",
      "notice-dismiss",
    ]);
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
    expect(box).toHaveClass("notice-pane-box");
    expect(box).not.toHaveClass("notice-opened");
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
    expect(mark?.parentElement).toHaveClass("notice-says");
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
      expect(container.querySelector(".pane-notice-row")).toHaveClass("pane-notice-row-empty"),
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
    const box = document.querySelector(".notice-pane-box");
    const line = box?.querySelector(".notice-pane");
    const under = box?.querySelector(".notice-under-pane");
    expect(under?.children.length).toBe(2);
    expect(under?.children[0]).toHaveClass("dispatch-choice");
    expect(under?.children[1].tagName).toBe("PRE");
    expect(line?.querySelector(".dispatch-choice")).toBeNull();
    expect(line?.nextElementSibling).toBe(under);
  });
});

describe("a Notice in the Inbox", () => {
  it.each(["inbox"] as const)("is not boxed at %s: the line and what it opened", (at) => {
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

    expect(container.querySelector(".notice-pane-box")).toBeNull();
    // Standing alone, the line is the inline surface itself (#1855).
    expect([...container.children].map((one) => one.className)).toEqual([
      `notice notice-${at} ui-inline-surface`,
      `notice-under notice-under-${at}`,
    ]);
    // The mark stays before the sentence, where the Inbox's rules place it.
    const line = screen.getByRole("status");
    expect(line.firstElementChild).toHaveClass("persona-mark");
  });
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
    const row = rule("\\.pane-notice-row");
    expect(row).not.toMatch(/position|z-index/);
    expect(row).toMatch(/max-height:\s*60%/);
    expect(row).toMatch(/min-height:\s*0/);
    expect(rule("\\.pane-notice-row\\.pane-notice-row-empty")).toMatch(/display:\s*none/);
    // What the row keeps behind "+N more" is not drawn, whatever its box's own display says.
    expect(rule("\\.pane-notices > \\[hidden\\]")).toMatch(/display:\s*none/);
  });

  it("stacks a pane's Notices and scrolls them when they are taller than the pane", () => {
    const stack = rule("\\.pane-notices");
    expect(stack).toMatch(/flex-direction:\s*column/);
    expect(stack).toMatch(/min-height:\s*0/);
    expect(stack).toMatch(/overflow-y:\s*auto/);
    // Never wider than the pane, and no wider than a sentence reads well.
    expect(stack).toMatch(/max-width:\s*min\(100%, 40rem\)/);
    expect(rule("\\.notice-pane-box")).toMatch(/max-width:\s*100%/);
  });

  it("wraps the ways out under the sentence, and never breaks one that fits", () => {
    // The line is the shared row of buttons that wraps (`ButtonRow`, #1855), read through
    // the window's cascade.
    aPaneNotice();
    const line = screen.getByRole("status", { name: "Vault" });
    expect(drawnWith(line, "flex-wrap")).toBe("wrap");
    // The sentence is as wide as it reads, so it shares a row only when everything fits.
    expect(rule("\\.notice-pane > \\.notice-says")).toMatch(/flex:\s*0 1 auto/);
    // A way out never shrinks, so a row with no room sends it to the next; only its cap at the
    // row's width can make it narrower than its words, and only then do they wrap.
    for (const way of within(line).getAllByRole("button")) {
      expect(drawnWith(way, "flex")).toBe("none");
      expect(drawnWith(way, "max-inline-size")).toBe("100%");
    }
  });

  it("keeps what a way out opened at the Notice's width, its draft scrolling in its own box", () => {
    expect(rule("\\.notice-under-pane")).toMatch(/min-width:\s*0/);
    const draft = rule("\\.notice-under-pane pre");
    expect(draft).toMatch(/max-width:\s*100%/);
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
    // A box's name and what it works with shrink with the Notice, in the grid the set draws.
    const parts = rule(
      "\\.dispatch-also \\.ui-choice-option label,\\s*\\.dispatch-also \\.ui-choice-says",
    );
    expect(parts).toMatch(/min-width:\s*0/);
    expect(rule("\\.ui-choice-option")).toMatch(/grid-template-columns:\s*auto 1fr/);
    // It is drawn under the line, so the rules for what a way out opened hold it too.
    expect(rule("\\.notice-under-pane > \\*")).toMatch(/max-width:\s*100%/);
  });

  it("is never taller than its pane: what it opened gives way, the answers never do", () => {
    // The dispatch question in a short pane (#1483's train): the box shrinks in the stack,
    // its line and answers keep their height, and what is under them scrolls, the brief
    // first.
    const box = rule("\\.notice-pane-box");
    expect(box).toMatch(/flex-direction:\s*column/);
    expect(box).toMatch(/min-height:\s*0/);
    expect(box).toMatch(/flex:\s*0 1 auto/);
    expect(rule("\\.notice-pane")).toMatch(/flex:\s*none/);
    const under = rule("\\.notice-under-pane");
    expect(under).toMatch(/min-height:\s*0/);
    expect(under).toMatch(/overflow-y:\s*auto/);
    expect(rule("\\.notice-under-pane > \\.block-report")).toMatch(/min-height:\s*0/);
    // The stack still scrolls as a whole when the Notices together are too tall.
    expect(rule("\\.pane-notices")).toMatch(/overflow-y:\s*auto/);
  });

  it("is opaque", () => {
    const { container } = aPaneNotice();
    expect(drawnWith(container.firstElementChild as Element, "background")).toBe(
      "var(--surface-raised)",
    );
  });

  it("is under whatever is opened over the window: a dialog, a menu", () => {
    // A pane's corner has a z-index, and a dialog portaled to the body has none.
    expect(rule("#root")).toMatch(/isolation:\s*isolate/);
  });

  it("leaves the window's lines and the Inbox's rows as they were", () => {
    expect(rule("\\.notice-inbox \\.notice-says")).toMatch(/display:\s*inline/);
    // The Inbox's row puts its ways out under the words, in the shared row that wraps.
    expect(rule("\\.notice-list \\.notice-inbox \\.notice-says")).toMatch(/flex:\s*1 1 100%/);
    // No rule of the pane's reaches a Notice that is not in one.
    const reach = [...css.matchAll(/([^{}]+)\{[^{}]*\}/g)]
      .map((hit) => hit[1].trim())
      .filter((selector) => /notice-(pane|under-pane)|pane-notices/.test(selector));
    for (const selector of reach) expect(selector).not.toMatch(/notice-(inbox|list)/);
  });
});

describe("Ask {persona}'s rules", () => {
  const css = readFileSync(join(process.cwd(), "src/App.css"), "utf8").replace(
    /\/\*[\s\S]*?\*\//g,
    "",
  );
  const rule = (selector: string) =>
    new RegExp(`(^|[},])\\s*${selector}\\s*\\{([^}]*)\\}`).exec(css)?.[2] ?? "";

  it("draws the box you write in as every list field: the row's width, growing with its lines", () => {
    // The shared list field (DS-10, `ListField.test.tsx`) is the whole of it: no rule of the
    // dialog's own sizes it.
    expect(rule("\\.ask-persona \\.ui-setting-control > textarea\\.ui-field")).toBe("");
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
      const box = this.closest(".pane-notices > *");
      const top = box === null ? 0 : [...(box.parentElement?.children ?? [])].indexOf(box) * 40;
      return DOMRect.fromRect({ x: 0, y: top, width: 100, height: 30 });
    });
    let clock = 1_000_000;
    const now = vi.spyOn(Date, "now").mockImplementation(() => clock);
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
      const box = this.closest(".pane-notices > *");
      const top = box === null ? 0 : [...(box.parentElement?.children ?? [])].indexOf(box) * 40;
      return DOMRect.fromRect({ x: 0, y: top, width: 100, height: 30 });
    });
    const offset = vi.spyOn(HTMLElement.prototype, "offsetTop", "get").mockReturnValue(4);
    let clock = 1_000_000;
    const now = vi.spyOn(Date, "now").mockImplementation(() => clock);
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
