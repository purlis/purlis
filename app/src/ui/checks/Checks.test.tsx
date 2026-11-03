// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { cleanup, render, screen, within } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import { CHECK_VERDICTS, CheckLine, CheckList } from "./Checks";

afterEach(cleanup);

describe("CheckList", () => {
  it("names the list for a reader", () => {
    render(
      <CheckList aria-label="What was checked">
        <CheckLine verdict="ok" title="A chat started" />
      </CheckList>,
    );
    expect(screen.getByRole("list", { name: "What was checked" })).toBeTruthy();
  });

  it("says each verdict in words as well as by its mark", () => {
    render(
      <CheckList aria-label="Checks">
        {CHECK_VERDICTS.map((verdict) => (
          <CheckLine key={verdict} verdict={verdict} title={verdict} />
        ))}
      </CheckList>,
    );
    const said = screen.getAllByRole("listitem").map((line) => line.textContent);
    expect(said).toEqual([
      "Passed: ok",
      "Failed: missing",
      "Running: running",
      "Not yet: todo",
      "Needs a look: warn",
      "Skipped: skip",
    ]);
  });

  it("says what's under a line and at its end", () => {
    render(
      <CheckList aria-label="Checks">
        <CheckLine verdict="ok" title="A chat started" trail="In 11 seconds">
          On dev-box
        </CheckLine>
      </CheckList>,
    );
    const line = screen.getByRole("listitem");
    expect(within(line).getByText("On dev-box")).toBeTruthy();
    expect(within(line).getByText("In 11 seconds")).toBeTruthy();
  });
});
