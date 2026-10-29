// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The review page** (#3672): its body a region named for what is reviewed, its bar's summary
 * describing the main action, the dialog's answers in their places, Return in a field finding the
 * main action only where a dialog's would, and a body that scrolls with nothing to reach being a
 * Tab stop of its own. That the bar never covers a focused control, and that changing words moves
 * nothing, is held by the stories in a real browser (`ReviewPage.stories.tsx`).
 */
import { afterEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { useState } from "react";
import { TextField } from "../field/Field";
import type { DialogAction } from "../dialog/Dialog";
import { ReviewPage } from "./ReviewPage";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

function Plan({ main, alternative }: { main: DialogAction; alternative?: DialogAction }) {
  const [limit, setLimit] = useState("3.00");
  return (
    <ReviewPage
      label="Grade deploy"
      summary="Nothing runs until you press Start. It spends at most $3.00 of your account."
      onCancel={() => {}}
      main={main}
      alternative={alternative}
    >
      <TextField label="At most, in dollars" value={limit} onChange={setLimit} />
    </ReviewPage>
  );
}

/** Return in the limit's box, as a native form hears it. */
function returnInTheField() {
  const box = screen.getByRole("textbox", { name: "At most, in dollars" });
  const form = box.closest("form");
  if (form === null) throw new Error("the box is in no form");
  act(() => fireEvent.submit(form));
}

describe("a review page", () => {
  it("names its body for what is reviewed and lets its summary describe the main action", () => {
    render(<Plan main={{ children: "Start grading · 12 chats" }} />);
    expect(screen.getByRole("region", { name: "Grade deploy" })).toContainElement(
      screen.getByRole("textbox", { name: "At most, in dollars" }),
    );
    expect(
      screen.getByRole("button", { name: "Start grading · 12 chats" }),
    ).toHaveAccessibleDescription(
      "Nothing runs until you press Start. It spends at most $3.00 of your account.",
    );
  });

  it("holds the dialog's answers in their places: Cancel, the other, the main action", () => {
    const cancelled = vi.fn();
    render(
      <ReviewPage
        label="Update brainstorming"
        summary="Chats keep the current version until you accept."
        onCancel={cancelled}
        cancel={{ children: "Not now" }}
        alternative={{ children: "Keep mine" }}
        main={{ children: "Accept update" }}
      >
        <p>What changes</p>
      </ReviewPage>,
    );
    const bar = document.querySelector('[data-scope="review-page"][data-part="answers"]');
    if (bar === null) throw new Error("the page draws no answers");
    expect([...bar.querySelectorAll("button")].map((one) => one.textContent)).toEqual([
      "Not now",
      "Keep mine",
      "Accept update",
    ]);
    act(() => fireEvent.click(screen.getByRole("button", { name: "Not now" })));
    expect(cancelled).toHaveBeenCalledOnce();
  });

  it("starts with Return from a field", () => {
    const start = vi.fn();
    render(<Plan main={{ children: "Start grading · 12 chats", onPress: start }} />);
    returnInTheField();
    expect(start).toHaveBeenCalledOnce();
  });

  it("never finds a grant, an act that cannot be taken back, or an unavailable one by Return", () => {
    for (const main of [
      { children: "Allow", variant: "grants", onPress: vi.fn() },
      { children: "Remove source", variant: "danger", onPress: vi.fn() },
      {
        children: "Start grading · 12 chats",
        isDisabled: true,
        tooltip: "The plan changed: show the new plan first.",
        onPress: vi.fn(),
      },
    ] as const) {
      render(<Plan main={main as DialogAction} />);
      returnInTheField();
      expect(main.onPress, String(main.children)).not.toHaveBeenCalled();
      cleanup();
    }
  });

  it("presses a main action that says so by ⌘⏎, as a dialog's", () => {
    const start = vi.fn();
    render(
      <Plan
        main={{
          children: "Start grading · 12 chats",
          byReturn: false,
          byChord: true,
          onPress: start,
        }}
      />,
    );
    const box = screen.getByRole("textbox", { name: "At most, in dollars" });
    const mac = navigator.platform.startsWith("Mac");
    act(() => {
      fireEvent.keyDown(box, { key: "Enter", metaKey: mac, ctrlKey: !mac });
    });
    expect(start).toHaveBeenCalledOnce();
  });

  it("keeps an unavailable main action a Tab stop that says why", () => {
    render(
      <Plan
        main={{
          children: "Start grading · 12 chats",
          isDisabled: true,
          tooltip: "The plan changed: show the new plan first.",
        }}
      />,
    );
    const start = screen.getByRole("button", { name: "Start grading · 12 chats" });
    expect(start).toHaveAttribute("aria-disabled", "true");
    expect(start).not.toBeDisabled();
    expect(start.getAttribute("aria-describedby")).toBeTruthy();
  });

  it("is a Tab stop of its own while it scrolls with nothing in it to reach", () => {
    vi.spyOn(HTMLElement.prototype, "scrollHeight", "get").mockReturnValue(900);
    vi.spyOn(HTMLElement.prototype, "clientHeight", "get").mockReturnValue(300);
    render(
      <ReviewPage
        label="Grade deploy"
        summary="Nothing runs until you press Start."
        onCancel={() => {}}
      >
        <ol>
          <li>deploy this to staging</li>
        </ol>
      </ReviewPage>,
    );
    expect(screen.getByRole("region", { name: "Grade deploy" })).toHaveAttribute("tabindex", "0");
    cleanup();
    render(<Plan main={{ children: "Start" }} />);
    expect(screen.getByRole("region", { name: "Grade deploy" })).not.toHaveAttribute("tabindex");
  });
});
