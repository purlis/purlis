// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render, screen, within } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { SkillTab } from "./SkillTab";

afterEach(() => {
  cleanup();
  clearMocks();
});

const RUN = "01JB0000000000000000000001";

const COLUMNS = [
  "Started",
  "Persona",
  "Started by",
  "Ended",
  "Tool calls",
  "Errors",
  "Refused",
  "Asks",
  "Tokens",
  "Time",
];

/** purlis's Runs panel, as the core answers it: a sentence and the table of runs. */
function runsPanel(rows: { key: string; cells: string[] }[]) {
  return {
    key: "runs",
    title: "Runs",
    by: null,
    refused: null,
    blocks: [
      ...(rows.length === 0
        ? []
        : [
            {
              kind: "note",
              text: "Each time a chat used the skill, from then until its turn ended. What happened inside it, never a verdict.",
              tone: "plain",
            },
          ]),
      {
        kind: "table",
        caption: "Runs of deploy",
        columns: COLUMNS,
        rows,
        empty: {
          headline: "No runs of deploy yet",
          body: "A run starts when a chat uses the skill, and ends with its turn.",
          offer: null,
        },
      },
    ],
  };
}

describe("a skill's Runs panel", () => {
  it("lists each run's signals, never a verdict, and marks the run the chip opened it on", async () => {
    mockIPC((cmd) => {
      if (cmd === "skill_panels")
        return [
          runsPanel([
            {
              key: RUN,
              cells: [
                "2026-10-20 10:00 UTC",
                "ops",
                "The chat read the skill",
                "Interrupted",
                "7",
                "1",
                "2",
                "1",
                "Not reported",
                "3 s",
              ],
            },
          ]),
        ];
      throw new Error(`unexpected ${cmd}`);
    });
    render(<SkillTab plane="p" name="deploy" run={RUN} changed={0} />);

    const row = await screen.findByTestId(`table-row-${RUN}`);
    expect(row.getAttribute("aria-current")).toBe("true");
    const text = row.textContent ?? "";
    expect(text).toContain("The chat read the skill");
    expect(text).toContain("Interrupted");
    expect(text).toContain("Not reported");
    expect(text).toContain("3 s");
    expect(text).toContain("2026-10-20 10:00 UTC");
    const runs = screen.getByTestId("skill-runs");
    expect(within(runs).getByRole("columnheader", { name: "Tool calls" })).toBeTruthy();
    expect(runs.textContent).not.toMatch(/pass|fail/i);
  });

  it("says there are no runs yet, and how one starts", async () => {
    mockIPC((cmd) => {
      if (cmd === "skill_panels") return [runsPanel([])];
      throw new Error(`unexpected ${cmd}`);
    });
    render(<SkillTab plane="p" name="deploy" changed={0} />);

    expect((await screen.findByTestId("table-empty")).textContent).toContain(
      "No runs of deploy yet",
    );
  });
});

describe("a panel an extension adds to a skill's tab", () => {
  it("is drawn after purlis's own, under its title and the extension's name", async () => {
    mockIPC((cmd) => {
      if (cmd === "skill_panels")
        return [
          runsPanel([]),
          {
            key: "ext/house/checks",
            title: "Checks",
            by: "House rules",
            refused: null,
            blocks: [{ kind: "note", text: "Two checks passed review.", tone: "plain" }],
          },
          {
            key: "ext/house/broken",
            title: "Broken",
            by: "House rules",
            refused: "'house' did not answer within 5 seconds, so purlis stopped it.",
            blocks: [],
          },
        ];
      throw new Error(`unexpected ${cmd}`);
    });
    render(<SkillTab plane="p" name="deploy" changed={0} />);

    const checks = await screen.findByTestId("skill-panel-ext/house/checks");
    expect(within(checks).getByRole("heading").textContent).toBe("Checks · House rules");
    expect(checks.textContent).toContain("Two checks passed review.");
    const broken = screen.getByTestId("skill-panel-ext/house/broken");
    expect(within(broken).getByTestId("skill-panel-refused").textContent).toContain(
      "did not answer within 5 seconds",
    );
    const order = screen
      .getAllByRole("heading", { level: 3 })
      .map((heading) => heading.textContent);
    expect(order).toEqual(["Runs", "Checks · House rules", "Broken · House rules"]);
  });
});
