// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/// <reference types="node" />
/**
 * **A Notice stands on the shared inline surface, and its ways out are the shared button in a
 * row that wraps** (#1855).
 *
 * A Notice is not drawn over anything (#1647), so it takes no overlay's lift; it is the inline
 * surface (`primitives/overlays.css`): the overlay's border and corner scale, a ground in the
 * page, and no shadow. Its ways out must still wrap in the narrowest pane (#1481), so they are
 * small `Button` inside a `ButtonRow` (`ui/button/button.css`), where a button may break its
 * words over lines instead of being clipped.
 *
 * Held through the window's cascade (`cascade.testkit.ts`), so a view rule cannot fake it; the
 * measuring in a real narrow pane is `pane-notices.e2e.ts`.
 */
import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import { drawnWith } from "./cascade.testkit";
import { Notice, NoticeList, NoticeOf, NoticePaneRow } from "./Notice";

afterEach(cleanup);

/** A Notice with every kind of way out: a fix, a link, Copy command and Dismiss. */
const everyWay = (at: "pane" | "inbox", tone: "news" | "trouble" = "news") => (
  <Notice
    key={at}
    cause={`every-way:${at}`}
    at={at}
    tone={tone}
    label="Every way"
    fixes={[{ label: "Allow steward to use this vault for every chat", onPress: () => undefined }]}
    link={{ label: "Open Settings", onPress: () => undefined }}
    copy="purlis vault allow devops"
    onDismiss={() => undefined}
  >
    This chat runs as steward, so purlis did not open the vault.
  </Notice>
);

/** The buttons a Notice's line draws, and the line. */
function waysOf(name = "Every way") {
  const line = screen.getByRole("status", { name });
  return { line, buttons: within(line).getAllByRole("button") };
}

/** A way out is the shared small button, and in its row it may wrap rather than be clipped. */
function isASharedWrappingButton(button: HTMLElement) {
  const what = button.textContent ?? "";
  expect(button, what).toHaveAttribute("data-size", "sm");
  expect(drawnWith(button, "border-radius"), what).toBe("var(--radius-control)");
  expect(drawnWith(button, "white-space"), what).toBe("normal");
  expect(drawnWith(button, "max-inline-size"), what).toBe("100%");
  expect(drawnWith(button, "block-size"), what).toBe("auto");
  expect(drawnWith(button, "min-block-size"), what).toBe(
    "max(24px, var(--size-chrome), var(--space-7))",
  );
}

describe("a Notice's ways out", () => {
  it("are the shared small button in a row that wraps, in a pane", () => {
    render(
      <NoticeOf.Provider value={{ whose: "steward 4", onGo: () => undefined }}>
        {everyWay("pane")}
      </NoticeOf.Provider>,
    );
    const { line, buttons } = waysOf("steward 4: Every way");
    expect(buttons.map((one) => one.textContent)).toEqual([
      "Go to it",
      "Allow steward to use this vault for every chat",
      "Open Settings",
      "Copy command",
      "Dismiss",
    ]);
    expect(line).toHaveAttribute("data-scope", "button-row");
    expect(drawnWith(line, "display")).toBe("flex");
    expect(drawnWith(line, "flex-wrap")).toBe("wrap");
    for (const button of buttons) isASharedWrappingButton(button);
    // Dismiss puts it away and fixes nothing: the quiet one.
    expect(screen.getByRole("button", { name: "Dismiss" })).toHaveAttribute(
      "data-variant",
      "ghost",
    );
  });

  it("are the shared small button in the Inbox's list too", async () => {
    render(<NoticeList>{everyWay("inbox")}</NoticeList>);
    const line = await screen.findByRole("status", { name: "Every way" });
    expect(line).toHaveAttribute("data-scope", "button-row");
    for (const button of within(line).getAllByRole("button")) isASharedWrappingButton(button);
  });

  it("say a press on its way as busy, still reachable", () => {
    render(
      <Notice
        cause="memory-deleted"
        label="Deleted"
        fixes={[{ label: "Undo", onPress: () => undefined, busy: true }]}
      >
        Deleted.
      </Notice>,
    );
    const undo = screen.getByRole("button", { name: "Undo" });
    expect(undo).toHaveAttribute("aria-busy", "true");
    expect(undo).toHaveAttribute("data-disabled", "true");
    expect(drawnWith(undo, "color")).toBe("var(--text-faint)");
    expect(drawnWith(undo, "cursor")).toBe("progress");
  });

  it("puts +N more in a pane's row as the shared small button", async () => {
    const one = (cause: string) => (
      <Notice
        key={cause}
        cause={cause}
        at="pane"
        label={cause}
        fixes={[{ label: `Answer ${cause}`, onPress: () => undefined }]}
      >
        {cause}
      </Notice>
    );
    render(<NoticePaneRow>{[one("a:1"), one("b:1"), one("c:1")]}</NoticePaneRow>);
    const more = await screen.findByRole("button", { name: "+1 more" });
    expect(more).toHaveAttribute("data-size", "sm");
    expect(drawnWith(more, "block-size")).toBe("max(24px, var(--size-chrome), var(--space-7))");
  });
});

describe("a Notice's surface", () => {
  /** The notice family's box (#2912): the raised ground in the page, the cards' corner, no lift. */
  const BOX: [string, string][] = [
    ["background-color", "var(--surface-raised)"],
    ["color", "var(--text-primary)"],
    ["border-radius", "var(--radius-lg)"],
  ];
  const boxOf = (line: HTMLElement) => line.closest('[data-scope="notice"]') as HTMLElement;

  it("is the notice family's box in a pane, edged in the waiting colour and never lifted", () => {
    const { container } = render(everyWay("pane"));
    const box = container.firstElementChild as HTMLElement;
    expect(box).toHaveAttribute("data-scope", "notice");
    expect(box).toHaveAttribute("data-placement", "pane");
    for (const [name, value] of BOX) expect(drawnWith(box, name), name).toBe(value);
    expect(drawnWith(box, "border")).toBe("1px solid var(--state-waiting)");
    expect(drawnWith(box, "box-shadow")).toBeUndefined();
  });

  it("is the same box where it stands alone, edged in the hairline, and the danger ground for trouble", () => {
    render(
      <>
        {everyWay("inbox")}
        <Notice cause="window-trouble" tone="trouble" label="Trouble" onDismiss={() => undefined}>
          It went wrong.
        </Notice>
      </>,
    );
    const news = boxOf(screen.getByRole("status", { name: "Every way" }));
    expect(news).toHaveAttribute("data-placement", "alone");
    for (const [name, value] of BOX) expect(drawnWith(news, name), name).toBe(value);
    expect(drawnWith(news, "border-color")).toBe("var(--border-subtle)");
    const trouble = boxOf(screen.getByRole("status", { name: "Trouble" }));
    expect(trouble).toHaveAttribute("data-variant", "trouble");
    expect(drawnWith(trouble, "background-color")).toBe("var(--danger-surface)");
    expect(drawnWith(trouble, "color")).toBe("var(--danger-text)");
  });

  it("is the list's row in the Inbox, not a box of its own", async () => {
    render(<NoticeList>{everyWay("inbox")}</NoticeList>);
    await waitFor(() => screen.getByRole("status", { name: "Every way" }));
    const row = boxOf(screen.getByRole("status", { name: "Every way" }));
    expect(row).toHaveAttribute("data-placement", "inbox");
    expect(drawnWith(row, "border-width")).toBe("0");
    expect(drawnWith(row, "background-color")).toBe("transparent");
  });
});
