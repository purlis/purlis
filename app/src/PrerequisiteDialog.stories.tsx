// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The prerequisite modal** (#7719, ADR 0130 §4): the project's open policy prerequisite, raised
 * as an alert of the dialog family, under Waiting and Broken, with a foreign seal key, on a linked
 * page, and the Notice an Accept leaves. Each is drawn, held to axe, walked by the keyboard and
 * compared with its baseline in Chromium and WebKit; the baselines are drawn by the pull
 * request's `stories` run only, and Chromium's only when that run has the `update-baselines`
 * label (the ready gate runs WebKit alone on a pull request). The core is faked at its command
 * boundary.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, waitFor, within } from "storybook/test";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { PolicyPrerequisite, PolicyState, Prerequisites } from "./bindings";
import { SETTLE_MS } from "./noticeSettle";
import { forgetPolicies } from "./projectPolicy";
import { WATCH } from "./projectPolicy.fixture";
import { expectFocusInSight, pictureNow, untilFocused } from "./stories.testkit";
import {
  ACCEPTED_AFTER,
  ACCEPT_IN_THE_WINDOW,
  BROKEN_SUMMARY,
  PrerequisiteDialog,
  forgetPrerequisiteAnswers,
} from "./PrerequisiteDialog";

const PLANE = "/home/op/smart-ide";
const TITLE = "smart-ide's policy needs your answer";

const LOOSER: PolicyState = {
  ...WATCH,
  waiting: [
    { family: "guards.git", accepted: "enforce", committed: "log" },
    { family: "guards.merge", accepted: "log", committed: "off" },
    { family: "dispatch.denies steward -> devops", accepted: "denied", committed: "lifted" },
  ],
  locked: ["guards.credential"],
};

const WAITING: PolicyPrerequisite = {
  state: "open",
  reasons: ["waiting"],
  digest: "d1",
  needs_new_key: false,
  after_new_key: false,
  nothing_kept: false,
  policy: LOOSER,
};

function Page({ canAccept }: { entry: PolicyPrerequisite; canAccept: boolean }) {
  return (
    <div style={{ display: "grid", gap: "var(--space-3)", inlineSize: 720 }}>
      <PrerequisiteDialog plane={PLANE} canAccept={canAccept} onOpenPolicy={() => {}} />
    </div>
  );
}

const meta = {
  component: Page,
  args: { entry: WAITING, canAccept: true },
  parameters: { screenshot: "page" },
  beforeEach: ({ args }) => {
    let list: Prerequisites = { plane: PLANE, list: [{ kind: "policy", ...args.entry }] };
    mockIPC((cmd) => {
      if (cmd === "project_prerequisites") return list;
      if (cmd === "accept_project_policy" || cmd === "accept_project_policy_with_new_key") {
        list = { plane: PLANE, list: [] };
        return { ...WATCH, digest: args.entry.digest };
      }
      return null;
    });
    return () => {
      forgetPolicies();
      forgetPrerequisiteAnswers();
      clearMocks();
    };
  },
} satisfies Meta<typeof Page>;

export default meta;
type Story = StoryObj<typeof meta>;

const settles = () => new Promise((done) => setTimeout(done, SETTLE_MS + 100));

/** Waits for the alert, with the keyboard on its answer that grants nothing. */
async function opened(cancel: string) {
  const page = within(document.body);
  const alert = await page.findByRole("alertdialog", { name: TITLE });
  await untilFocused(
    () => page.getByRole("button", { name: cancel }),
    "the keyboard starts on the answer that grants nothing",
  );
  expectFocusInSight();
  return { page, alert };
}

/**
 * **Waiting**: the loosenings first with their count, the lock, Keep my current rules leading;
 * Tab walks to the link and Accept, and Esc keeps the rules.
 */
export const Waiting: Story = {
  play: async ({ userEvent }) => {
    const { page, alert } = await opened("Keep my current rules");
    await expect(alert).toHaveTextContent("Accept loosens 3 rules:");
    await pictureNow();
    await userEvent.tab();
    await untilFocused(
      () => page.getByRole("button", { name: "Accept" }),
      "Tab moves from Keep my current rules to Accept",
    );
    expectFocusInSight();
    await settles();
    await userEvent.keyboard("{Escape}");
    await waitFor(() => expect(page.queryByRole("alertdialog")).toBeNull());
  },
};

/** **Broken**: Work in strict mode leads, and the summary is everything Accept puts in force. */
export const Broken: Story = {
  args: { entry: { ...WAITING, reasons: ["broken"] } },
  play: async () => {
    const { alert } = await opened("Work in strict mode");
    await expect(alert).toHaveTextContent(BROKEN_SUMMARY);
    await pictureNow();
  },
};

/** **A foreign seal key**: the granting answer reads Make a new key and accept. */
export const ForeignKey: Story = {
  args: { entry: { ...WAITING, reasons: ["broken"], needs_new_key: true } },
  play: async () => {
    const { page } = await opened("Work in strict mode");
    // Waited for, not read once: the drawing pass of #7981's run found it still opening.
    await waitFor(() =>
      expect(page.getByRole("button", { name: "Make a new key and accept" })).toBeVisible(),
    );
    await pictureNow();
  },
};

/** **A linked page**: no Accept, and the footer says where to accept. */
export const LinkedPage: Story = {
  args: { canAccept: false },
  play: async () => {
    const { page, alert } = await opened("Keep my current rules");
    await expect(page.queryByRole("button", { name: "Accept" })).toBeNull();
    await expect(alert).toHaveTextContent(ACCEPT_IN_THE_WINDOW);
    await pictureNow();
  },
};

/** **After Accept**: the alert closes and the Notice says the policy is accepted. */
export const PolicyAccepted: Story = {
  play: async ({ userEvent }) => {
    const { page } = await opened("Keep my current rules");
    await settles();
    await userEvent.click(page.getByRole("button", { name: "Accept" }));
    await waitFor(() => expect(page.queryByRole("alertdialog")).toBeNull());
    await expect(page.getByText(ACCEPTED_AFTER, { exact: false })).toBeVisible();
    await userEvent.tab();
    await untilFocused(
      () => page.getByRole("button", { name: /Dismiss/ }),
      "Tab reaches the Notice's Dismiss",
    );
    expectFocusInSight();
  },
};
