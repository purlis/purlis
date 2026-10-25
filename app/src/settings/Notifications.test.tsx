// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { notificationPrefs } from "../notificationPrefs";
import { forgetThisLaunch } from "../regions";
import type { LiveSetting } from "./groups";
import { notificationsGroup } from "./notifications";
import { selectNamed } from "../ui/select/select.testkit";

/**
 * **Settings › You › Notifications** (#1989): the quiet hours, off until turned on, each time
 * chosen while they are on.
 */

beforeEach(() => {
  forgetThisLaunch();
  mockIPC(() => null);
});
afterEach(() => {
  cleanup();
  clearMocks();
});

function Row({ id }: { id: string }) {
  const setting = notificationsGroup().settings.find((one) => one.id === id) as LiveSetting;
  const { control } = setting.useControl();
  return <>{control({ id, label: setting.label, describedBy: undefined })}</>;
}

describe("quiet hours", () => {
  it("are off, and their times out of reach, until turned on", async () => {
    render(
      <>
        <Row id="you.notifications.quiet" />
        <Row id="you.notifications.quiet.from" />
      </>,
    );
    const on = screen.getByRole("switch");
    expect(on).not.toBeChecked();
    expect(selectNamed("Quiet from")).toBeDisabled();

    await userEvent.click(on);

    expect(notificationPrefs().quiet.on).toBe(true);
    await waitFor(() => expect(selectNamed("Quiet from")).toBeEnabled());
  });
});
