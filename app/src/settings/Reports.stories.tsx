// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **You › Reports** (#5688, spec #5683): the two answers as Settings holds them, unanswered,
 * all on and all off, with "See exactly what's sent…", and Always send drawn only while Agent
 * drafts is allowed (#5695), and held off by a project that turns it off for its members (#5696).
 * Drawn, held to axe, walked by the keyboard
 * and pictured in every pictured theme cell (`docs/component-stories.md`). The core is faked at
 * its command boundary, keeping what it is told.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, waitFor, within } from "storybook/test";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { ReportConsent, SentReport } from "../bindings";
import { forgetReportConsent, UNANSWERED } from "../reportAnswers";
import { expectFocusInSight, untilFocused } from "../stories.testkit";
import { SettingGroup } from "./components";
import { LiveRow, ShownWhile } from "./SettingsTab";
import type { LiveSetting } from "./groups";
import { reportsGroup } from "./reports";

let kept: ReportConsent = UNANSWERED;

/** A Crash report as the core lists it: its envelope, byte for byte. */
const SENT: SentReport = {
  at: "2026-11-08T03:19:00Z",
  title: "purlis panicked at crates/purlis-core/src/plane.rs:12:5",
  to: "purlis's report service",
  address: "purlis's report service, event 9f1c2a7be0d34b6c8e5f0a1b2c3d4e5f",
  bytes:
    '{"event_id":"9f1c2a7be0d34b6c8e5f0a1b2c3d4e5f","sent_at":"2026-11-08T03:19:00Z"}\n' +
    '{"type":"event","length":312,"content_type":"application/json"}\n' +
    '{"event_id":"9f1c2a7be0d34b6c8e5f0a1b2c3d4e5f","level":"fatal","platform":"native",' +
    '"release":"purlis@0.42.0","environment":"stable","culprit":"crates/purlis-core/src/plane.rs:12:5",' +
    '"exception":{"values":[{"type":"panic","value":"could not read [path] for [workspace]"}]}}\n',
  waiting: false,
};

/** The Reports group as Settings › You draws it, in the project `p1`. */
function Reports() {
  const group = reportsGroup("p1");
  return (
    <SettingGroup label={group.label} help={group.help}>
      {group.settings.map((one) => (
        <ShownWhile key={one.id} useShown={(one as LiveSetting).useShown}>
          <LiveRow setting={one as LiveSetting} />
        </ShownWhile>
      ))}
    </SettingGroup>
  );
}

const meta = {
  component: Reports,
  // What this machine keeps: a story's `parameters.consent`, nothing answered by default.
  beforeEach: ({ parameters }) => {
    kept = (parameters.consent as ReportConsent | undefined) ?? UNANSWERED;
    const sent = (parameters.sent as SentReport[] | undefined) ?? [];
    // Whether the project lets its members send without asking: yes unless a story says no.
    const may = (parameters.membersMayAlwaysSend as boolean | undefined) ?? true;
    forgetReportConsent();
    mockIPC((cmd, args) => {
      if (cmd === "report_consent") return kept;
      if (cmd === "reports_sent") return sent;
      if (cmd === "members_may_always_send") return may;
      if (cmd === "keep_report_consent") kept = (args as { consent: ReportConsent }).consent;
      return null;
    });
    return () => clearMocks();
  },
  decorators: [
    (Story) => (
      <div style={{ inlineSize: 720 }}>
        <Story />
      </div>
    ),
  ],
} satisfies Meta<typeof Reports>;

export default meta;
type Story = StoryObj<typeof meta>;

const radios = (group: HTMLElement, name: string) =>
  within(within(group).getByRole("radiogroup", { name })).getAllByRole("radio");

/** **Unanswered**: neither choice made for the person; choosing one keeps it. */
export const Unanswered: Story = {
  play: async ({ canvas, userEvent }) => {
    const group = await canvas.findByRole("region", { name: "Reports" });
    for (const stream of ["Crash reports", "Agent drafts"])
      for (const radio of radios(group, stream)) await expect(radio).not.toBeChecked();
    await userEvent.tab();
    await untilFocused(
      () => within(group).getByRole("radio", { name: "Send" }),
      "Tab reaches Crash reports",
    );
    expectFocusInSight();
    await userEvent.keyboard(" ");
    await waitFor(() => expect(kept.crash_reports).toBe(true));
    await expect(kept.agent_drafts).toBeNull();
  },
};

/** **All on**: Send, Allow and Always send chosen; Tab walks on to See exactly what's sent…. */
export const AllOn: Story = {
  parameters: {
    consent: {
      ...UNANSWERED,
      crash_reports: true,
      agent_drafts: true,
      always_send: true,
      asked: true,
    },
  },
  play: async ({ canvas, userEvent }) => {
    const group = await canvas.findByRole("region", { name: "Reports" });
    await waitFor(() => expect(within(group).getByRole("radio", { name: "Send" })).toBeChecked());
    await expect(within(group).getByRole("radio", { name: "Allow" })).toBeChecked();
    await expect(within(group).getByRole("radio", { name: "Send without asking" })).toBeChecked();
    await userEvent.tab();
    await userEvent.tab();
    await userEvent.tab();
    await userEvent.tab();
    await untilFocused(
      () => within(group).getByRole("button", { name: "See exactly what's sent…" }),
      "Tab reaches See exactly what's sent…",
    );
    expectFocusInSight();
  },
};

/**
 * **All off**: Don't send and Don't allow chosen, and no Always send row; Allow from the keyboard
 * turns drafts on, and the Always send row appears with nothing chosen (#5695).
 */
export const AllOff: Story = {
  parameters: {
    consent: { ...UNANSWERED, crash_reports: false, agent_drafts: false, asked: true },
  },
  play: async ({ canvas, userEvent }) => {
    const group = await canvas.findByRole("region", { name: "Reports" });
    await waitFor(() =>
      expect(within(group).getByRole("radio", { name: "Don't send" })).toBeChecked(),
    );
    const dont = within(group).getByRole("radio", { name: "Don't allow" });
    await expect(dont).toBeChecked();
    dont.focus();
    await untilFocused(() => dont, "Don't allow takes the keyboard");
    expectFocusInSight();
    await expect(
      within(group).queryByRole("radiogroup", { name: "Always send drafts" }),
    ).toBeNull();
    await userEvent.keyboard("{ArrowLeft}");
    await waitFor(() => expect(kept.agent_drafts).toBe(true));
    for (const radio of await waitFor(() => radios(group, "Always send drafts")))
      await expect(radio).not.toBeChecked();
  },
};

/**
 * **Held by the project** (#5696): the project turns sending without asking off, so Always send
 * is shown off and out of reach, badged Project; Tab passes from Agent drafts to the badge,
 * whose tooltip says why.
 */
export const HeldByProject: Story = {
  parameters: {
    consent: {
      ...UNANSWERED,
      crash_reports: true,
      agent_drafts: true,
      always_send: true,
      asked: true,
    },
    membersMayAlwaysSend: false,
  },
  play: async ({ canvas, userEvent }) => {
    const group = await canvas.findByRole("region", { name: "Reports" });
    const always = () => within(group).getByRole("radiogroup", { name: "Always send drafts" });
    await waitFor(() =>
      expect(within(always()).getByRole("radio", { name: "Ask me first" })).toBeChecked(),
    );
    for (const radio of within(always()).getAllByRole("radio")) await expect(radio).toBeDisabled();
    within(group).getByRole("radio", { name: "Allow" }).focus();
    await userEvent.tab();
    await untilFocused(
      () => within(group).getByText("Project").closest("[tabindex]"),
      "Tab reaches the Project badge",
    );
    expectFocusInSight();
    await expect(await within(document.body).findByRole("tooltip")).toHaveTextContent(
      /Your project turns this off/,
    );
    await userEvent.keyboard("{Escape}");
  },
};

/** **Sent**: the sent list holds a Crash report; Enter opens exactly its bytes, Esc closes. */
export const Sent: Story = {
  parameters: {
    consent: { ...UNANSWERED, crash_reports: true, agent_drafts: false, asked: true },
    sent: [SENT],
  },
  play: async ({ canvas, userEvent }) => {
    const list = await canvas.findByRole("grid", { name: "Reports sent from this machine" });
    const row = await within(list).findByRole("row", { name: new RegExp(SENT.title) });
    row.focus();
    await untilFocused(() => row, "the sent report takes the keyboard");
    expectFocusInSight();
    await userEvent.keyboard("{Enter}");
    const dialog = await within(document.body).findByRole("dialog", { name: SENT.title });
    await expect(within(dialog).getByText(/"level":"fatal"/)).toBeVisible();
    await userEvent.keyboard("{Escape}");
    await waitFor(() => expect(within(document.body).queryByRole("dialog")).toBeNull());
  },
};

/** **Waiting to send**: a Crash report made offline waits under the one that left (#5693). */
export const WaitingToSend: Story = {
  parameters: {
    consent: { ...UNANSWERED, crash_reports: true, agent_drafts: false, asked: true },
    sent: [
      SENT,
      {
        ...SENT,
        at: "2026-11-08T09:42:00Z",
        title: "purlis panicked at crates/purlis-core/src/watch.rs:88:13",
        address: "",
        waiting: true,
      },
    ],
  },
  play: async ({ canvas }) => {
    const list = await canvas.findByRole("grid", { name: "Reports sent from this machine" });
    const row = await within(list).findByRole("row", { name: /watch\.rs:88:13/ });
    await expect(within(row).getByText(/Waiting to send/)).toBeVisible();
    row.focus();
    await untilFocused(() => row, "the waiting report takes the keyboard");
    expectFocusInSight();
  },
};

/** **Nothing sent**: the list says so, in a line that is not an instruction. */
export const NothingSent: Story = {
  play: async ({ canvas }) => {
    await expect(
      await canvas.findByText("No report has been sent from this machine."),
    ).toBeVisible();
  },
};
