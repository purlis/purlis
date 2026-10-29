// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **What needs a decision** (#3678): an attention Notice, where nothing went wrong but the
 * person must choose. It is a polite status and never an alert, it says so in words and with its
 * mark (never by colour alone), and the Inbox lists it after trouble and before news.
 */
import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import { Notice, NoticeList } from "./Notice";

afterEach(cleanup);

const listed = () =>
  [...document.querySelectorAll("[data-scope=notice-list] [data-cause]")].map((one) =>
    one.getAttribute("data-cause"),
  );

describe("an attention Notice", () => {
  it("is a status, never an alert, and says it needs a decision with its mark", () => {
    render(
      <Notice cause="draft-stale:review-pr" tone="attention" onDismiss={() => undefined}>
        The draft of review-pr is out of date.
      </Notice>,
    );
    const line = screen.getByRole("status");
    expect(screen.queryByRole("alert")).toBeNull();
    expect(line).toHaveTextContent("Needs a decision: The draft of review-pr is out of date.");
    const box = line.closest('[data-scope="notice"][data-part="root"]');
    expect(box).toHaveAttribute("data-variant", "attention");
    expect(box?.querySelector('[data-part="mark"]')).toHaveAttribute("aria-hidden", "true");
  });

  it("as a card, says it needs a decision before its title, and leads with its mark", () => {
    render(
      <Notice
        cause="skill-changed:grilling"
        tone="attention"
        title="A chat changed grilling"
        fixes={[{ label: "Review the change", onPress: () => undefined }]}
      >
        Chats keep the version you accepted.
      </Notice>,
    );
    expect(screen.getByRole("status")).toHaveTextContent(
      "Needs a decision: A chat changed grilling",
    );
    const box = screen.getByRole("status").closest('[data-part="root"]');
    expect(box?.firstElementChild).toHaveAttribute("data-part", "mark");
  });

  it("in a pane, leaves its mark to the band", () => {
    render(
      <Notice cause="grade-plan:7" at="pane" tone="attention" onDismiss={() => undefined}>
        The grade plan changed.
      </Notice>,
    );
    expect(screen.getByRole("status")).toHaveTextContent(
      "Needs a decision: The grade plan changed.",
    );
    expect(document.querySelector('[data-part="mark"]')).toBeNull();
  });

  it("stands after trouble and before news in the Inbox's list", () => {
    render(
      <NoticeList>
        <Notice cause="pin-dormant:ide" onDismiss={() => undefined}>
          news
        </Notice>
        <Notice cause="draft-stale:a" tone="attention" onDismiss={() => undefined}>
          decide
        </Notice>
        <Notice cause="window-trouble" tone="trouble" onDismiss={() => undefined}>
          trouble
        </Notice>
      </NoticeList>,
    );
    expect(listed()).toEqual(["window-trouble", "draft-stale:a", "pin-dormant:ide"]);
  });
});
