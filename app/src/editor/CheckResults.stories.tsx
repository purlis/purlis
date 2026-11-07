// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **A branch's check results** (ADR 0116, #5407): as reported, and out of date. The results are
 * shaped on CodeQL's and Scorecard's real output (`tests/fixtures/sarif`).
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, userEvent } from "storybook/test";
import type { CheckResult, CheckResults, PlaneId } from "../bindings";
import { expectFocusInSight, untilFocused } from "../stories.testkit";
import { CheckResultsList, marksOf } from "./CheckResults";
import { MergeViewer } from "./LightEditor";

const one = (over: Partial<CheckResult>): CheckResult => ({
  tool: "CodeQL",
  outOfDate: false,
  severity: "error",
  path: "release.yml",
  line: 3,
  shownAt: 3,
  rule: "actions/cache-poisoning/poisonable-step",
  message: "Potential cache poisoning in the context of the default branch.",
  ...over,
});

const CHECKS: CheckResults = {
  sets: [{ tool: "CodeQL", head: "32f2ddcb58188242", outOfDate: false, results: 3 }],
  onLines: [one({})],
  outside: [one({ line: 90, shownAt: null, severity: "warning", message: "Far from the change." })],
  elsewhere: [
    one({
      tool: "Scorecard",
      path: "no file associated with this alert",
      line: 1,
      shownAt: null,
      severity: "note",
      rule: "FuzzingID",
      message: "Project is not fuzzed.",
    }),
  ],
  moreOnLines: 0,
  moreOutside: 0,
  moreElsewhere: 2,
  trouble: null,
  faults: [],
};

const meta = {
  component: CheckResultsList,
  args: {
    plane: "p" as PlaneId,
    cut: { workspace: "w", repo: "r", piece: "b" },
    path: "release.yml",
    results: CHECKS,
  },
  decorators: [
    (Story) => (
      <div style={{ width: 860, height: 320, display: "flex", flexDirection: "column" }}>
        <Story />
      </div>
    ),
  ],
} satisfies Meta<typeof CheckResultsList>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Reported: Story = {
  play: async ({ canvasElement }) => {
    const region = canvasElement.querySelector<HTMLElement>(".check-results");
    await userEvent.tab();
    await untilFocused(() => region, "the list, which scrolls");
    expectFocusInSight();
    await userEvent.tab();
    await expect(document.activeElement?.textContent).toBe("Open at line 3");
    expectFocusInSight();
  },
};

export const OutOfDate: Story = {
  args: {
    results: {
      ...CHECKS,
      sets: [{ tool: "CodeQL", head: "32f2ddcb58188242", outOfDate: true, results: 3 }],
      onLines: [one({ outOfDate: true })],
    },
  },
};

export const CouldNotBeRead: Story = {
  args: {
    results: {
      ...CHECKS,
      sets: [],
      onLines: [],
      outside: [],
      elsewhere: [],
      moreElsewhere: 0,
      trouble: "purlis could not read the results: delete the file, and report them again",
    },
  },
};

/** A Check reporter answered something that is not SARIF (#5410): a Notice naming it, above the
 *  results recorded before, which stay. Dismiss is reached by the keyboard and puts it away. */
export const ReporterFault: Story = {
  args: {
    results: {
      ...CHECKS,
      faults: [
        {
          extension: "mypy-sarif",
          said:
            "The Check reporter 'mypy-sarif' answered something purlis cannot read as SARIF " +
            "2.1.0: it is not JSON (expected value at line 1 column 1). Nothing it said was " +
            "recorded, and the results before stay. Run the check again once the extension is " +
            "fixed",
        },
      ],
    },
  },
  play: async ({ canvasElement }) => {
    const notice = () =>
      canvasElement.querySelector('[data-cause="check-reporter-fault:mypy-sarif"]');
    await expect(notice()).not.toBeNull();
    await userEvent.tab();
    await userEvent.tab();
    await untilFocused(
      () => canvasElement.querySelector<HTMLElement>("[data-cause] button"),
      "the Notice's Dismiss",
    );
    expectFocusInSight();
    await userEvent.keyboard("{Enter}");
    await expect(notice()).toBeNull();
  },
};

/** The marks in the head's gutter, one of each severity. */
export const InTheGutter: StoryObj<typeof MergeViewer> = {
  parameters: { a11y: { context: { exclude: [".cm-editor"] } } },
  render: () => (
    <MergeViewer
      path="release.yml"
      base={"one\ntwo\nthree\nfour\n"}
      head={"one\nTWO\nTHREE\nFOUR\n"}
      hunks={[{ oldStart: 2, oldLines: 3, newStart: 2, newLines: 3 }]}
      marks={marksOf({
        ...CHECKS,
        onLines: [
          one({ line: 2, shownAt: 2 }),
          one({ line: 3, shownAt: 3, severity: "warning" }),
          one({ line: 4, shownAt: 4, severity: "note" }),
        ],
      })}
    />
  ),
};
