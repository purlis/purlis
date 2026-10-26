// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render, screen, within } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { SkillTab, groupSaid } from "./SkillTab";
import type { Offer } from "./actions";
import type { SkillStats } from "./bindings";

const NO_TOTALS: SkillStats = { by_harness: [], by_persona: [], by_workspace: [] };

const EMPTY_PAGE = { copies: [], uses: [], used_by: [], uses_cut: false, pinned_by: [] };

afterEach(() => {
  cleanup();
  clearMocks();
});

const RUN = "01JB0000000000000000000001";

const COLUMNS = [
  "Started",
  "Persona",
  "Ran on",
  "Started by",
  "Ended",
  "Tool calls",
  "Errors",
  "Refused",
  "Asks",
  "Tokens",
  "Time",
  "Nested",
];

/** purlis's Runs panel, as the core answers it: a sentence and the table of runs. */
function runsPanel(rows: { key: string; cells: string[] }[]) {
  return {
    key: { kind: "runs" },
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
      if (cmd === "skill_details") return EMPTY_PAGE;
      if (cmd === "skill_stats") return NO_TOTALS;
      if (cmd === "skill_panels")
        return [
          runsPanel([
            {
              key: RUN,
              cells: [
                "2026-10-20 10:00 UTC",
                "ops",
                "Not recorded",
                "The chat read the skill",
                "Interrupted",
                "7",
                "1",
                "2",
                "1",
                "Not reported",
                "3 s",
                "",
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
      if (cmd === "skill_details") return EMPTY_PAGE;
      if (cmd === "skill_stats") return NO_TOTALS;
      if (cmd === "skill_panels") return [runsPanel([])];
      throw new Error(`unexpected ${cmd}`);
    });
    render(<SkillTab plane="p" name="deploy" changed={0} />);

    expect((await screen.findByTestId("table-empty")).textContent).toContain(
      "No runs of deploy yet",
    );
  });

  it("offers a new chat where there are no runs, as the catalogue's own row", async () => {
    const pressed: string[] = [];
    const panel = runsPanel([]);
    const table = panel.blocks[panel.blocks.length - 1] as { empty: { offer: string | null } };
    table.empty.offer = "chat.new";
    mockIPC((cmd) => {
      if (cmd === "skill_details") return EMPTY_PAGE;
      if (cmd === "skill_stats") return NO_TOTALS;
      if (cmd === "skill_panels") return [panel];
      throw new Error(`unexpected ${cmd}`);
    });
    const newTab = { id: "chat.new", title: "New tab", available: true } as unknown as Offer;
    render(
      <SkillTab
        plane="p"
        name="deploy"
        changed={0}
        offerFor={(id) => (id === "chat.new" ? newTab : undefined)}
        onPress={(offer) => pressed.push(offer.id)}
      />,
    );

    const empty = await screen.findByTestId("table-empty");
    within(empty).getByRole("button", { name: "New tab" }).click();
    expect(pressed).toEqual(["chat.new"]);
  });
});

describe("a panel an extension adds to a skill's tab", () => {
  it("is drawn after purlis's own, under its title and the extension's name", async () => {
    mockIPC((cmd) => {
      if (cmd === "skill_details") return EMPTY_PAGE;
      if (cmd === "skill_stats") return NO_TOTALS;
      if (cmd === "skill_panels")
        return [
          runsPanel([]),
          {
            key: { kind: "extension", extension: "house", panel: "checks" },
            title: "Checks",
            by: "House rules",
            refused: null,
            blocks: [{ kind: "note", text: "Two checks passed review.", tone: "plain" }],
          },
          {
            key: { kind: "extension", extension: "house", panel: "broken" },
            title: "Broken",
            by: "House rules",
            refused: "'house' did not answer within 5 seconds, so purlis stopped it.",
            blocks: [],
          },
        ];
      throw new Error(`unexpected ${cmd}`);
    });
    render(<SkillTab plane="p" name="deploy" changed={0} />);

    const checks = await screen.findByTestId("skill-panel-house/checks");
    expect(within(checks).getByRole("heading").textContent).toBe("Checks · House rules");
    expect(checks.textContent).toContain("Two checks passed review.");
    const broken = screen.getByTestId("skill-panel-house/broken");
    expect(within(broken).getByTestId("skill-panel-refused").textContent).toContain(
      "did not answer within 5 seconds",
    );
    const order = screen
      .getAllByRole("heading", { level: 2 })
      .map((heading) => heading.textContent)
      .filter((text) => text === "Runs" || (text ?? "").endsWith("· House rules"));
    expect(order).toEqual(["Runs", "Checks · House rules", "Broken · House rules"]);
  });
});

describe("a skill's totals", () => {
  it("adds up its runs by what they ran on, persona and workspace, with their signals and tokens", async () => {
    const row = (
      group: SkillStats["by_harness"][number]["group"],
      runs: number,
      ended: number,
    ) => ({
      group,
      runs,
      ended,
      tool_calls: 5,
      tool_errors: 1,
      refusals: 0,
      asks: 2,
      tokens: ended > 0 ? 1200 : null,
      ms: 65_000,
    });
    mockIPC((cmd) => {
      if (cmd === "skill_details") return EMPTY_PAGE;
      if (cmd === "skill_panels") return [runsPanel([])];
      if (cmd === "skill_stats")
        return {
          by_harness: [
            row({ kind: "named", name: "Claude Code" }, 2, 2),
            row({ kind: "not-recorded" }, 1, 0),
          ],
          by_persona: [row({ kind: "named", name: "ops" }, 3, 2)],
          by_workspace: [row({ kind: "none" }, 3, 2)],
        } satisfies SkillStats;
      throw new Error(`unexpected ${cmd}`);
    });
    render(<SkillTab plane="p" name="deploy" changed={0} />);

    const harness = await screen.findByTestId("skill-totals-by_harness");
    expect(harness.textContent).toContain("Claude Code");
    expect(harness.textContent).toContain("2 runs");
    expect(harness.textContent).toContain("1 run, 0 ended");
    expect(harness.textContent).toContain("Not recorded");
    expect(harness.textContent).toContain("1200");
    expect(harness.textContent).toContain("1 min 5 s");
    expect(screen.getByTestId("skill-totals-by_persona").textContent).toContain("ops");
    expect(screen.getByTestId("skill-totals-by_workspace").textContent).toContain("Project root");
    expect(screen.getByTestId("skill-totals").textContent).not.toMatch(/pass|fail|harness/i);
  });

  it("shows nothing to total before a skill has run", async () => {
    mockIPC((cmd) => {
      if (cmd === "skill_details") return EMPTY_PAGE;
      if (cmd === "skill_panels") return [runsPanel([])];
      if (cmd === "skill_stats") return NO_TOTALS;
      throw new Error(`unexpected ${cmd}`);
    });
    render(<SkillTab plane="p" name="deploy" changed={0} />);
    await screen.findByTestId("table-empty");
    expect(screen.queryByTestId("skill-totals")).toBeNull();
  });

  it("says each group in plain words", () => {
    expect(groupSaid({ kind: "named", name: "ops" }, "No persona")).toBe("ops");
    expect(groupSaid({ kind: "none" }, "No persona")).toBe("No persona");
    expect(groupSaid({ kind: "not-recorded" }, "No persona")).toBe("Not recorded");
  });
});
