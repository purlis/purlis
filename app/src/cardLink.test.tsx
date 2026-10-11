// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import { cardEnding, FirstOnHarness, noteHarness } from "./cardLink";

/**
 * The card's label found at the end of an off control's reason, and the one-time note's memory
 * (#1134). The surfaces that draw them have their own window tests (Menus, FirstTaskTab,
 * StartChat); these are the edges those do not reach.
 */

afterEach(() => {
  cleanup();
  localStorage.clear();
});

const OPENCODE = { label: "What opencode can do here", name: "opencode" };
const LINE = "opencode cannot have a prompt typed in for you.";

describe("the card a reason ends with", () => {
  it("is found with the words before it, the label and its full stop left off", () => {
    expect(cardEnding(`${LINE} See What opencode can do here.`, [OPENCODE])).toEqual({
      before: `${LINE} See `,
      card: OPENCODE,
    });
  });

  it("is none where the label is only inside the reason, or not there at all", () => {
    expect(cardEnding(`What opencode can do here. ${LINE}`, [OPENCODE])).toBeUndefined();
    expect(cardEnding(LINE, [OPENCODE])).toBeUndefined();
    expect(cardEnding(LINE, [{ label: "", name: "x" }])).toBeUndefined();
  });
});

describe("the one-time note's memory", () => {
  const glance = {
    name: "codex",
    title: "Codex",
    label: "What Codex can do here",
    lines: ["Codex does not tell purlis when it is waiting for you."],
    cannot_type: null,
  };

  it("is kept per harness: another harness's start leaves this one's note said", () => {
    noteHarness("opencode");
    render(<FirstOnHarness glance={glance} />);

    expect(screen.getByRole("note")).toBeInTheDocument();
  });

  it("reads storage it cannot make sense of as nothing kept, and keeps over it", () => {
    localStorage.setItem("purlis.harness-noted", "{not json");
    render(<FirstOnHarness glance={glance} />);
    expect(screen.getByRole("note")).toBeInTheDocument();
    cleanup();

    noteHarness("codex");
    render(<FirstOnHarness glance={glance} />);
    expect(screen.queryByRole("note")).toBeNull();
  });
});
