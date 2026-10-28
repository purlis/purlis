// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The review page** (#3672, the skill sources and skill grading prototypes): the install
 * review and the grade plan, short and long, the plan's Start unavailable with its reason, and a
 * narrow page. The bar stays in view, a focused control is never under it, and changing the main
 * action's words moves nothing. Each is walked by the keyboard and held to axe in Chromium and
 * WebKit in every cell of the theme matrix (`docs/component-stories.md`).
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState, type ReactNode } from "react";
import { expect, waitFor } from "storybook/test";
import { expectFocusInSight, untilFocused } from "../../stories.testkit";
import { Badge } from "../badge/Badge";
import { Button } from "../button/Button";
import { Disclosure } from "../disclosure/Disclosure";
import { TextField } from "../field/Field";
import { Panel } from "../panel/Panel";
import { ReviewPage } from "./ReviewPage";

/** The pane a review page is drawn in: a panel of a view tab, `width` by `height`. */
function Pane({ width, height, children }: { width: number; height: number; children: ReactNode }) {
  return (
    <div style={{ display: "grid", inlineSize: width, blockSize: height }}>
      <Panel variant="content">{children}</Panel>
    </div>
  );
}

/** One file the install adds, to read under its risk mark. */
function File({ name, risk }: { name: string; risk?: string }) {
  return (
    <Disclosure
      label={`Show ${name}`}
      trail={<Badge variant={risk ? "waiting" : "ok"}>{risk ?? "ok"}</Badge>}
    >
      <pre style={{ margin: 0, whiteSpace: "pre-wrap" }}>{`# ${name}\n\n1. …`}</pre>
    </Disclosure>
  );
}

const SCRIPTS = Array.from({ length: 12 }, (_, at) => `scripts/step-${at + 1}.sh`);

/** **The install review** (#3632): what it adds, file by file, then Cancel and Install. */
function InstallReview({
  width = 720,
  height = 420,
  long = false,
}: {
  width?: number;
  height?: number;
  long?: boolean;
}) {
  const [said, setSaid] = useState("Not installed");
  return (
    <>
      <p role="status">{said}</p>
      <Pane width={width} height={height}>
        <ReviewPage
          label="Install systematic-debugging from Anthropic's skills"
          summary="Nothing is installed until you press Install."
          onCancel={() => setSaid("Cancelled")}
          main={{ children: "Install", onPress: () => setSaid("Installed") }}
        >
          <h1 style={{ margin: 0, fontSize: "var(--font-size-lg)" }}>
            Install systematic-debugging from Anthropic's skills
          </h1>
          <p style={{ margin: 0 }}>
            You are reviewing Anthropic's skills at commit 3f2a1c9. What you install stays pinned to
            this commit.
          </p>
          <File name="SKILL.md" />
          <File name="scripts/repro.sh" risk="runs a script" />
          {long && SCRIPTS.map((name) => <File key={name} name={name} risk="runs a script" />)}
        </ReviewPage>
      </Pane>
    </>
  );
}

const meta = { component: InstallReview } satisfies Meta<typeof InstallReview>;

export default meta;
type Story = StoryObj<typeof meta>;

/** The review page a story draws. */
function pageIn(canvas: HTMLElement): HTMLElement {
  const page = canvas.querySelector<HTMLElement>('[data-scope="review-page"]');
  if (page === null) throw new Error("the story draws no review page");
  return page;
}

/** The bar's and the body's boxes, to tell where the one ends and the other starts. */
function parts(root: HTMLElement) {
  const box = (part: string) => {
    const found = root.querySelector(`[data-scope="review-page"][data-part="${part}"]`);
    if (found === null) throw new Error(`the review page draws no ${part}`);
    return found.getBoundingClientRect();
  };
  return { body: box("body"), bar: box("bar"), summary: box("summary"), answers: box("answers") };
}

/** Whether the focused control is wholly above the bar. */
async function clearOfTheBar(root: HTMLElement) {
  const focused = document.activeElement as HTMLElement;
  const { bar } = parts(root);
  await expect(focused.getBoundingClientRect().bottom).toBeLessThanOrEqual(bar.top + 0.5);
}

/**
 * **The install review, short**: the files, then the bar with its summary, Cancel and Install.
 * Tab goes through the files to the answers, in their places, and Install says what it does.
 */
export const InstallShort: Story = {
  play: async ({ canvas, canvasElement, userEvent }) => {
    const install = canvas.getByRole("button", { name: "Install" });
    await expect(install).toHaveAccessibleDescription(
      "Nothing is installed until you press Install.",
    );
    const answers = canvasElement.querySelectorAll(
      '[data-scope="review-page"][data-part="answers"] button',
    );
    await expect([...answers].map((one) => one.textContent)).toEqual(["Cancel", "Install"]);
    await userEvent.tab();
    await untilFocused(() => canvas.getByRole("button", { name: "Show SKILL.md" }), "a file");
    await userEvent.tab();
    await userEvent.tab();
    await untilFocused(() => canvas.getByRole("button", { name: "Cancel" }), "Cancel");
    expectFocusInSight();
    await userEvent.tab();
    await untilFocused(() => install, "Install");
    expectFocusInSight();
    await userEvent.keyboard("{Enter}");
    await waitFor(() => expect(canvas.getByRole("status")).toHaveTextContent("Installed"));
  },
};

/**
 * **The install review, long**: fourteen files in a short pane. The body scrolls under the bar,
 * which stays in view, and every file the keyboard reaches is brought wholly above it.
 */
export const InstallLong: Story = {
  args: { long: true, height: 360 },
  play: async ({ canvas, canvasElement, userEvent }) => {
    const root = pageIn(canvasElement);
    const before = parts(root);
    await expect(before.body.bottom).toBeLessThanOrEqual(before.bar.top + 0.5);
    const last = canvas.getByRole("button", { name: `Show ${SCRIPTS.at(-1)}` });
    await expect(last.getBoundingClientRect().top).toBeGreaterThan(before.bar.top);
    await userEvent.tab();
    for (let at = 0; at < 13; at += 1) {
      await userEvent.tab();
      expectFocusInSight();
      await clearOfTheBar(root);
    }
    await untilFocused(() => last, "the last file, above the bar");
    // The bar stayed where it was, and so did its answers.
    await expect(parts(root).bar.top).toBe(before.bar.top);
    await userEvent.tab();
    await untilFocused(() => canvas.getByRole("button", { name: "Cancel" }), "Cancel");
    expectFocusInSight();
    // Shift+Tab from the bar goes back to the last file, again wholly above the bar.
    await userEvent.tab({ shift: true });
    await untilFocused(() => last, "back on the last file");
    await clearOfTheBar(root);
  },
};

/** The trigger tests a grade runs, by name. */
const TESTS = [
  "deploy this to staging",
  "ship the release",
  "what does the deploy script do?",
  "roll back yesterday's deploy",
];

/** **The grade plan** (#3644): the limit, the tests, and Start, which says how many chats it runs. */
function GradePlan({
  width = 720,
  height = 420,
  long = false,
  changed = false,
}: {
  width?: number;
  height?: number;
  long?: boolean;
  changed?: boolean;
}) {
  const [limit, setLimit] = useState("3.00");
  const [chats, setChats] = useState(12);
  const [stale, setStale] = useState(changed);
  const [said, setSaid] = useState("Not started");
  const tests = long ? [...TESTS, ...TESTS, ...TESTS, ...TESTS] : TESTS;
  const summaries = ["3.00", "2.00"].map(
    (one) => `Nothing runs until you press Start. It spends at most $${one} of your account.`,
  );
  return (
    <>
      <p role="status">{said}</p>
      <Pane width={width} height={height}>
        <ReviewPage
          label="Grade deploy"
          summary={`Nothing runs until you press Start. It spends at most $${limit} of your account.`}
          summaryReserve={summaries}
          onCancel={() => setSaid("Cancelled")}
          main={{
            children: `Start grading · ${chats} chats`,
            reserve: ["Start grading · 12 chats", "Start grading · 9 chats"],
            onPress: () => setSaid(`Started ${chats} chats`),
            isDisabled: stale,
            tooltip: stale
              ? "The plan changed since you read it: show the new plan first."
              : undefined,
          }}
        >
          <h1 style={{ margin: 0, fontSize: "var(--font-size-lg)" }}>Grade deploy</h1>
          {stale && (
            <div style={{ display: "flex", gap: "var(--space-4)", alignItems: "center" }}>
              <p style={{ margin: 0 }}>deploy's tests changed since this plan was drawn.</p>
              <Button size="sm" onPress={() => setStale(false)}>
                Show the new plan
              </Button>
            </div>
          )}
          <TextField
            label="At most, in dollars"
            value={limit}
            onChange={setLimit}
            validate={(value) =>
              /^\d+(\.\d\d)?$/.test(value) ? undefined : "A sum, such as 3.00."
            }
          />
          <ul style={{ margin: 0, paddingInlineStart: "var(--space-6)" }}>
            {tests.map((one, at) => (
              <li key={at}>{one}</li>
            ))}
          </ul>
          <Button size="sm" onPress={() => setChats(9)}>
            Drop a test
          </Button>
        </ReviewPage>
      </Pane>
    </>
  );
}

/**
 * **The grade plan**: Start says how many chats it runs. Dropping a test turns "· 12 chats" to "·
 * 9 chats", which moves nothing in the bar; Return in the limit's box starts it.
 */
export const GradePlanShort: StoryObj<typeof GradePlan> = {
  render: (args) => <GradePlan {...args} />,
  play: async ({ canvas, canvasElement, userEvent }) => {
    const root = pageIn(canvasElement);
    const start = canvas.getByRole("button", { name: "Start grading · 12 chats" });
    const before = { ...parts(root), start: start.getBoundingClientRect() };
    await userEvent.click(canvas.getByRole("button", { name: "Drop a test" }));
    await waitFor(() => expect(start).toHaveAccessibleName("Start grading · 9 chats"));
    const after = { ...parts(root), start: start.getBoundingClientRect() };
    for (const part of ["bar", "summary", "answers", "start"] as const)
      await expect(JSON.stringify(after[part])).toBe(JSON.stringify(before[part]));
    const limit = canvas.getByRole("textbox", { name: "At most, in dollars" });
    await userEvent.click(limit);
    await userEvent.keyboard("{Backspace}{Backspace}{Backspace}{Backspace}2.00");
    // A summary of other words keeps the bar's height.
    await expect(parts(root).bar.height).toBe(before.bar.height);
    await userEvent.keyboard("{Enter}");
    await waitFor(() => expect(canvas.getByRole("status")).toHaveTextContent("Started 9 chats"));
    await untilFocused(() => limit, "the limit keeps the keyboard");
    expectFocusInSight();
  },
};

/** **The grade plan, long**: sixteen tests in a short pane; the bar stays in view. */
export const GradePlanLong: StoryObj<typeof GradePlan> = {
  render: (args) => <GradePlan {...args} />,
  args: { long: true, height: 360 },
  play: async ({ canvas, canvasElement, userEvent }) => {
    const root = pageIn(canvasElement);
    const { bar } = parts(root);
    await expect(bar.bottom).toBeLessThanOrEqual(canvasElement.getBoundingClientRect().bottom + 1);
    await userEvent.tab();
    await untilFocused(
      () => canvas.getByRole("textbox", { name: "At most, in dollars" }),
      "the limit",
    );
    await userEvent.tab();
    const drop = canvas.getByRole("button", { name: "Drop a test" });
    await untilFocused(() => drop, "the last control above the bar");
    expectFocusInSight();
    await clearOfTheBar(root);
  },
};

/**
 * **A plan that changed**: Start is unavailable and stays a Tab stop, its reason heard as its
 * description, until Show the new plan.
 */
export const GradePlanChanged: StoryObj<typeof GradePlan> = {
  render: (args) => <GradePlan {...args} />,
  args: { changed: true },
  play: async ({ canvas, userEvent }) => {
    const start = canvas.getByRole("button", { name: "Start grading · 12 chats" });
    await expect(start).toHaveAttribute("aria-disabled", "true");
    await userEvent.tab();
    await untilFocused(() => canvas.getByRole("button", { name: "Show the new plan" }), "Show");
    await userEvent.tab();
    await userEvent.tab();
    await userEvent.tab();
    await untilFocused(() => canvas.getByRole("button", { name: "Cancel" }), "Cancel");
    await userEvent.tab();
    await untilFocused(() => start, "Start, unavailable, still a Tab stop");
    await waitFor(() =>
      expect(start).toHaveAccessibleDescription(
        expect.stringContaining("The plan changed since you read it: show the new plan first."),
      ),
    );
    await userEvent.keyboard("{Enter}");
    await expect(canvas.getByRole("status")).toHaveTextContent("Not started");
    // Return in a field finds no Start that cannot be pressed.
    await userEvent.click(canvas.getByRole("textbox", { name: "At most, in dollars" }));
    await userEvent.keyboard("{Enter}");
    await expect(canvas.getByRole("status")).toHaveTextContent("Not started");
    await userEvent.click(canvas.getByRole("button", { name: "Show the new plan" }));
    await waitFor(() => expect(start).not.toHaveAttribute("aria-disabled"));
  },
};

/** **Narrow**: the summary takes its own line over the answers. */
export const Narrow: Story = {
  args: { width: 340 },
  play: async ({ canvas, canvasElement, userEvent }) => {
    const root = pageIn(canvasElement);
    const { summary, answers } = parts(root);
    await expect(summary.bottom).toBeLessThanOrEqual(answers.top + 0.5);
    await userEvent.tab();
    await userEvent.tab();
    await userEvent.tab();
    await untilFocused(() => canvas.getByRole("button", { name: "Cancel" }), "Cancel");
    expectFocusInSight();
  },
};
