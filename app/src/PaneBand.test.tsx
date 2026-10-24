// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/// <reference types="node" />
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState, type ReactNode } from "react";
import { Notice, NoticePaneRow } from "./Notice";
import { PaneHead } from "./LayoutPanes";
import { drawnWith } from "./cascade.testkit";

/**
 * **A chat's pane: the band, the header, then the terminal** (#3142, the operator's variant A of
 * 2026-10-24). The prototype's numbered rules are the acceptance lines; what jsdom can hold is
 * held here (the keyboard, the shape, the rules as the cascade draws them), and what needs a
 * layout is measured in `pane-warnings.e2e.ts`.
 */

afterEach(cleanup);

/** A pane as `PaneFrame` draws one: the band and the header at its top, the terminal under. */
function Pane({ children }: { children: ReactNode }) {
  return (
    <div className="pane-frame pane-headed" data-testid="frame">
      <div className="pane-top">
        <NoticePaneRow>{children}</NoticePaneRow>
        <PaneHead
          doing={
            <div className="pane-actions">
              <div className="pane-doing" />
            </div>
          }
        >
          <div className="pane-chips">
            <span className="pane-harness">Claude Code</span>
          </div>
        </PaneHead>
      </div>
      <div className="pane-body">
        <div className="pane" data-testid="pane">
          <textarea className="xterm-helper-textarea" aria-label="Terminal input" />
        </div>
      </div>
    </div>
  );
}

/** Warnings a person can put away, as `ChatNotices` draws them, each gone once dismissed. */
function Warnings({
  causes,
  consent = false,
  consentLast = false,
}: {
  causes: string[];
  consent?: boolean;
  /** The consent under the others, as a refusal that arrived after them. */
  consentLast?: boolean;
}) {
  const [left, setLeft] = useState(causes);
  const vault = consent && (
    <Notice
      cause="vault-refused:7:forge"
      at="pane"
      tone="trouble"
      label="Vault"
      fixes={[
        { label: "Allow steward to use this vault", onPress: () => undefined, allows: true },
        { label: "Keep blocked", onPress: () => undefined },
      ]}
    >
      This chat runs as steward, and vault forge is tagged for devops.
    </Notice>
  );
  return (
    <Pane>
      {!consentLast && vault}
      {left.map((cause) => (
        <Notice
          key={cause}
          cause={cause}
          at="pane"
          label={cause}
          fixes={[{ label: `Fix ${cause}`, onPress: () => undefined }]}
          onDismiss={() => setLeft((was) => was.filter((one) => one !== cause))}
        >
          {cause} says something.
        </Notice>
      ))}
      {consentLast && vault}
    </Pane>
  );
}

const shown = () => screen.queryAllByRole("status").map((line) => line.getAttribute("aria-label"));

describe("the band's keyboard", () => {
  it("puts away the warning Esc is pressed on, and goes on to the next one (rule 14)", async () => {
    const user = userEvent.setup();
    render(<Warnings causes={["first", "second"]} />);
    await waitFor(() => expect(shown()).toEqual(["first", "second"]));

    screen.getByRole("button", { name: "Fix first" }).focus();
    await user.keyboard("{Escape}");

    await waitFor(() => expect(shown()).toEqual(["second"]));
    await waitFor(() => expect(screen.getByRole("button", { name: "Fix second" })).toHaveFocus());
  });

  it("gives the keyboard to the pane's terminal once the last warning is put away", async () => {
    const user = userEvent.setup();
    render(<Warnings causes={["only"]} />);
    await waitFor(() => expect(shown()).toEqual(["only"]));

    screen.getByRole("button", { name: "Dismiss" }).focus();
    await user.keyboard("{Escape}");

    await waitFor(() => expect(shown()).toEqual([]));
    await waitFor(() => expect(screen.getByLabelText("Terminal input")).toHaveFocus());
  });

  it("goes on from a warning put away by its Dismiss as well", async () => {
    const user = userEvent.setup();
    render(<Warnings causes={["first", "second"]} />);
    await waitFor(() => expect(shown()).toEqual(["first", "second"]));

    const first = screen.getByRole("status", { name: "first" });
    await user.click(within(first).getByRole("button", { name: "Dismiss" }));

    await waitFor(() => expect(shown()).toEqual(["second"]));
    await waitFor(() => expect(screen.getByRole("button", { name: "Fix second" })).toHaveFocus());
  });

  it("leaves a consent standing: it has no Dismiss, and Esc answers nothing (rule 9)", async () => {
    const user = userEvent.setup();
    render(<Warnings causes={["later"]} consent />);
    await waitFor(() => expect(shown()).toEqual(["Vault", "later"]));
    const vault = screen.getByRole("status", { name: "Vault" });
    expect(within(vault).queryByRole("button", { name: "Dismiss" })).toBeNull();

    const keep = within(vault).getByRole("button", { name: "Keep blocked" });
    keep.focus();
    await user.keyboard("{Escape}");

    expect(shown()).toEqual(["Vault", "later"]);
    expect(keep).toHaveFocus();
  });

  it("leaves Esc in what a way out opened to that field", async () => {
    const user = userEvent.setup();
    render(
      <Pane>
        <Notice
          cause="sandbox-blocked:2:report"
          at="pane"
          label="Report"
          fixes={[{ label: "Report…", onPress: () => undefined }]}
          onDismiss={() => undefined}
          under={<input aria-label="What happened" />}
        >
          The sandbox blocked a host.
        </Notice>
      </Pane>,
    );
    const field = await screen.findByLabelText("What happened");
    field.focus();
    await user.keyboard("{Escape}");
    expect(shown()).toEqual(["Report"]);
    expect(field).toHaveFocus();
  });

  it("never lands the keyboard on an Allow: a consent next is landed on by its line", async () => {
    const user = userEvent.setup();
    render(<Warnings causes={["first"]} consent consentLast />);
    await waitFor(() => expect(shown()).toEqual(["first", "Vault"]));

    screen.getByRole("button", { name: "Fix first" }).focus();
    await user.keyboard("{Escape}");

    await waitFor(() => expect(shown()).toEqual(["Vault"]));
    await waitFor(() => expect(screen.getByRole("status", { name: "Vault" })).toHaveFocus());
  });

  it("names the band as the pane's warnings while it has any", async () => {
    render(<Warnings causes={["first"]} />);
    expect(await screen.findByRole("region", { name: "Warnings for this pane" })).toBeTruthy();
  });

  it("says Show fewer while every warning is open, and +N more again once closed", async () => {
    const user = userEvent.setup();
    render(<Warnings causes={["a", "b", "c", "d"]} />);
    const more = await screen.findByRole("button", { name: "+2 more" });

    await user.click(more);
    expect(more).toHaveAccessibleName("Show fewer");
    expect(more).toHaveAttribute("aria-expanded", "true");
    expect(shown()).toEqual(["a", "b", "c", "d"]);

    await user.click(more);
    expect(more).toHaveAccessibleName("+2 more");
    await waitFor(() => expect(shown()).toEqual(["a", "b"]));
  });
});

describe("the band, the header and the terminal, in a pane", () => {
  it("draws the band and the header at the top of the pane, inside it, before the terminal", async () => {
    render(<Warnings causes={["first"]} />);
    await waitFor(() => expect(shown()).toEqual(["first"]));
    const frame = screen.getByTestId("frame");
    expect([...frame.children].map((one) => one.className)).toEqual(["pane-top", "pane-body"]);
    const top = frame.firstElementChild as Element;
    expect([...top.children].map((one) => one.className)).toEqual(["pane-notice-row", "pane-head"]);
    const head = top.lastElementChild as Element;
    expect([...head.children].map((one) => one.className)).toEqual(["pane-chips", "pane-actions"]);
  });

  it("joins the tab: the pane is the terminal's fill, and the band has no ground or line", async () => {
    render(<Warnings causes={["first"]} />);
    await waitFor(() => expect(shown()).toEqual(["first"]));
    const frame = screen.getByTestId("frame");
    expect(drawnWith(frame, "background-color")).toBe("var(--terminal-background)");
    const band = frame.querySelector(".pane-notice-row") as Element;
    expect(drawnWith(band, "background-color")).toBe("transparent");
    expect(drawnWith(band, "border")).toBe("0");
    // The runway: the band's own top is the terminal's fill (rule 2).
    expect(drawnWith(band, "padding")).toMatch(/^var\(--space-4\)/);
  });

  it("draws a warning as a wash with no edge, in the warnings' corner (rules 7 and 8)", async () => {
    render(<Warnings causes={["first"]} consent />);
    await waitFor(() => expect(shown()).toEqual(["Vault", "first"]));
    const box = (name: string) => screen.getByRole("status", { name }).parentElement as HTMLElement;
    expect(drawnWith(box("first"), "background-color")).toBe("var(--wash-warning)");
    expect(drawnWith(box("first"), "border")).toBe("0");
    expect(drawnWith(box("first"), "border-radius")).toBe("var(--radius-lg)");
    // A consent is a question, not trouble, though its Notice's tone is trouble.
    expect(drawnWith(box("Vault"), "background-color")).toBe("var(--wash-warning)");
  });

  it("never shrinks a warning in the band: the stack scrolls instead (rule 6)", async () => {
    render(<Warnings causes={["first"]} />);
    await waitFor(() => expect(shown()).toEqual(["first"]));
    const box = screen.getByRole("status", { name: "first" }).parentElement as HTMLElement;
    expect(drawnWith(box, "flex")).toBe("none");
    expect(drawnWith(box.parentElement as Element, "overflow-y")).toBe("auto");
  });

  it("draws a warning's plain ways out without an edge, and keeps a grant's", async () => {
    render(<Warnings causes={["first"]} consent />);
    const keep = await screen.findByRole("button", { name: "Keep blocked" });
    expect(drawnWith(keep, "border-color")).toBe("transparent");
    const allow = screen.getByRole("button", { name: "Allow steward to use this vault" });
    expect(drawnWith(allow, "border-color")).toBe("var(--accent-base)");
  });

  it("puts the words above their ways out (variant A)", async () => {
    render(<Warnings causes={["first"]} />);
    await waitFor(() => expect(shown()).toEqual(["first"]));
    const says = screen.getByRole("status", { name: "first" }).firstElementChild as Element;
    expect(drawnWith(says, "flex")).toBe("1 1 100%");
  });

  it("sets a consent's Allow apart from Keep blocked (rule 9)", async () => {
    render(<Warnings causes={[]} consent />);
    const keep = await screen.findByRole("button", { name: "Keep blocked" });
    expect(drawnWith(keep, "margin-inline-start")).toBe("var(--space-6)");
    const allow = screen.getByRole("button", { name: "Allow steward to use this vault" });
    expect(drawnWith(allow, "margin-inline-start")).not.toBe("var(--space-6)");
  });

  it("pins the terminal to its bottom (rule 4)", () => {
    render(<Warnings causes={[]} />);
    expect(drawnWith(screen.getByTestId("pane"), "justify-content")).toBe("flex-end");
  });
});

/** The rules a layout needs, as written: each is named for what breaks without it. */
describe("the pane's rules", () => {
  const css = readFileSync(join(process.cwd(), "src/paneFrame.css"), "utf8").replace(
    /\/\*[\s\S]*?\*\//g,
    "",
  );
  const rule = (selector: string) =>
    new RegExp(`(^|[},])\\s*${selector}\\s*\\{([^}]*)\\}`).exec(css)?.[2] ?? "";

  it("never lets the band and the header take more than about half the pane (rule 6)", () => {
    const top = rule("\\.pane-top");
    expect(top).toMatch(/max-block-size:\s*46%/);
    expect(top).toMatch(/flex-direction:\s*column/);
  });

  it("scrolls the warnings inside the band past its room", () => {
    expect(css).toMatch(/:is\(\.pane-notices,[^)]*\)\s*\{[^}]*overflow-y:\s*auto/);
  });

  it("reserves +N more's width on a row of its own (rules 5 and 15)", () => {
    // Both of what it says in one cell, the one not said hidden: as wide as the longer.
    expect(rule("\\.notice-more-says")).toMatch(/display:\s*inline-grid/);
    expect(rule("\\.notice-more-says > \\*")).toMatch(/grid-area:\s*1 \/ 1/);
    expect(rule("\\.notice-more-says > \\[data-off\\]")).toMatch(/visibility:\s*hidden/);
  });

  it("keeps the header one row that never gives way", () => {
    expect(rule("\\.pane-head")).toMatch(/flex:\s*none/);
  });
});
