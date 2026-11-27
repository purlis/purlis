// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// Pictures: drawn by the update-baselines bot (#7733 merged these stories without them).
/**
 * **An alert the app raises** (#7728, ADR 0104 as amended 2026-11-26): opened with no press, it
 * starts on the answer that grants nothing, answers nothing until it has settled, and says so
 * when what it shows changes. Each is drawn, held to axe, walked by the keyboard, and compared
 * with its baseline in Chromium and WebKit (`docs/component-stories.md`).
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { useEffect, useState } from "react";
import { expect, waitFor, within } from "storybook/test";
import { expectFocusInSight, pictureNow, untilFocused } from "../../stories.testkit";
import { SETTLE_MS } from "../../noticeSettle";
import { RAISED_ALERT_CHANGED, RAISED_ALERT_TOO_SOON, RaisedAlert } from "./RaisedAlert";

/** What the page says once the alert is answered. */
function Said({ said }: { said: string }) {
  return (
    <p role="status" style={{ margin: 0 }}>
      {said}
    </p>
  );
}

/** The branches a policy loosens, a real list in the alert's body. */
function Loosens({ branches }: { branches: readonly string[] }) {
  return (
    <ul className="ui-text-body" style={{ margin: 0 }}>
      {branches.map((branch) => (
        <li key={branch}>
          Chats on <code>{branch}</code> may push without asking
        </li>
      ))}
    </ul>
  );
}

/**
 * **A page the app raises an alert over**: it is raised as the page is drawn, as the core's event
 * raises one, and goes once it is answered. With `changesAfter`, what it shows changes that many
 * ms after it is raised, as a new digest from the core would change it.
 */
function Raised({ changesAfter }: { changesAfter?: number }) {
  const [raised, setRaised] = useState(false);
  const [said, setSaid] = useState("Not answered yet");
  const [branches, setBranches] = useState<readonly string[]>(["main", "release"]);
  useEffect(() => {
    const soon = setTimeout(() => setRaised(true), 0);
    return () => clearTimeout(soon);
  }, []);
  useEffect(() => {
    if (changesAfter === undefined) return;
    const later = setTimeout(() => setBranches((was) => [...was, "nightly"]), changesAfter);
    return () => clearTimeout(later);
  }, [changesAfter]);
  const answered = (how: string) => {
    setSaid(how);
    setRaised(false);
  };
  return (
    <div style={{ display: "grid", gap: "var(--space-3)", justifyItems: "start" }}>
      <Said said={said} />
      <RaisedAlert
        raised={raised}
        shows={branches.join(",")}
        title="smart-ide's policy needs your answer"
        why="Until you accept it, your chats keep the policy you accepted before."
        cancel={{ children: "Keep my current rules", onPress: () => answered("Kept") }}
        main={{ children: "Accept", variant: "grants", onPress: () => answered("Accepted") }}
      >
        <Loosens branches={branches} />
      </RaisedAlert>
    </div>
  );
}

const meta = {
  component: Raised,
  parameters: { screenshot: "page" },
} satisfies Meta<typeof Raised>;

export default meta;
type Story = StoryObj<typeof meta>;

const settles = () => new Promise((done) => setTimeout(done, SETTLE_MS + 100));

/**
 * **Raised with no press**: it starts on Keep my current rules, a press before it settles does
 * nothing and says why, Return never finds Accept, and Esc, once it has settled, keeps the rules
 * and gives the keyboard back.
 */
export const RaisedByTheApp: Story = {
  play: async ({ userEvent }) => {
    const page = within(document.body);
    const alert = await page.findByRole("alertdialog", {
      name: "smart-ide's policy needs your answer",
    });
    await expect(alert).toHaveAccessibleDescription(
      "Until you accept it, your chats keep the policy you accepted before.",
    );
    await untilFocused(
      () => page.getByRole("button", { name: "Keep my current rules" }),
      "the keyboard starts on the answer that grants nothing",
    );
    expectFocusInSight();
    // Pressed from the keyboard, not with `userEvent.click`: that one draws Accept under a pointer
    // that is not there, until the engine's own pointer moves, which WebKit does when it will, so
    // the picture was of either (#7847).
    await userEvent.tab();
    await untilFocused(() => page.getByRole("button", { name: "Accept" }), "Tab reaches Accept");
    await userEvent.keyboard(" ");
    await expect(within(alert).getByRole("status")).toHaveTextContent(RAISED_ALERT_TOO_SOON);
    await expect(alert.querySelector("[data-hovered]")).toBeNull();
    await pictureNow();
    await settles();
    await userEvent.keyboard("{Escape}");
    await waitFor(() => expect(page.queryByRole("alertdialog")).toBeNull());
    await expect(page.getByText("Kept")).toBeVisible();
  },
};

/**
 * **Its content changes while it is up**: it says so, draws the new list, takes the keyboard off
 * Accept and waits to settle again before Accept answers.
 */
export const ItsContentChanges: Story = {
  args: { changesAfter: SETTLE_MS * 2 },
  parameters: { screenshot: false },
  play: async ({ userEvent }) => {
    const page = within(document.body);
    const alert = await page.findByRole("alertdialog", {
      name: "smart-ide's policy needs your answer",
    });
    await settles();
    const accept = page.getByRole("button", { name: "Accept" });
    accept.focus();
    await waitFor(() => expect(alert).toHaveTextContent("nightly"), { timeout: SETTLE_MS * 3 });
    await expect(within(alert).getByRole("status")).toHaveTextContent(RAISED_ALERT_CHANGED);
    await untilFocused(
      () => page.getByRole("button", { name: "Keep my current rules" }),
      "a change takes the keyboard off Accept",
    );
    await userEvent.click(accept);
    await expect(page.queryByRole("alertdialog")).not.toBeNull();
    await settles();
    await userEvent.click(accept);
    await waitFor(() => expect(page.queryByRole("alertdialog")).toBeNull());
    await expect(page.getByText("Accepted")).toBeVisible();
  },
};
