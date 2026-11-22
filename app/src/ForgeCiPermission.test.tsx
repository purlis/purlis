// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { ForgeCiPermission } from "./ForgeCiPermission";

/**
 * **Allow CI files** (ADR 0128 D1.1): the dialog asks for the second sign-in's code as it opens,
 * waits, says the core's answer, and stops the waiting on Cancel.
 */

let sent: { cmd: string; args: Record<string, unknown> }[];
let finish: ((answer: { state: string; says: string }) => void) | undefined;

beforeEach(() => {
  sent = [];
  finish = undefined;
  mockIPC((cmd, args) => {
    sent.push({ cmd, args: (args ?? {}) as Record<string, unknown> });
    if (cmd === "forge_ci_permission_start") {
      return {
        flow: 7,
        host: "github.com",
        title: "GitHub",
        user_code: "ABCD-1234",
        verification_uri: "https://github.com/login/device",
        expires_in: 900,
        scopes: ["repo", "read:org", "workflow"],
      };
    }
    if (cmd === "forge_ci_permission_wait") {
      return new Promise((ok) => {
        finish = ok as (answer: { state: string; says: string }) => void;
      });
    }
    return null;
  });
});

afterEach(() => {
  cleanup();
  clearMocks();
});

describe("Allow CI files", () => {
  it("asks for the account's code, shows it, and says the core's answer when it is allowed", async () => {
    const onDone = vi.fn();
    render(
      <ForgeCiPermission
        accountId="acct"
        login="octocat"
        host="github.com"
        onDone={onDone}
        onCancel={vi.fn()}
      />,
    );
    expect(await screen.findByLabelText("The code to enter on GitHub")).toHaveTextContent(
      "ABCD-1234",
    );
    expect(sent.find((one) => one.cmd === "forge_ci_permission_start")?.args).toEqual({
      accountId: "acct",
    });
    await waitFor(() => expect(finish).toBeDefined());
    finish?.({ state: "ready", says: "octocat may now push CI files when you approve one." });
    expect(await screen.findByText(/may now push CI files/)).toBeTruthy();
    await userEvent.click(screen.getByRole("button", { name: "Done" }));
    expect(onDone).toHaveBeenCalled();
  });

  it("stops the waiting when it is cancelled", async () => {
    const onCancel = vi.fn();
    render(
      <ForgeCiPermission
        accountId="acct"
        login="octocat"
        host="github.com"
        onDone={vi.fn()}
        onCancel={onCancel}
      />,
    );
    await screen.findByLabelText("The code to enter on GitHub");
    await userEvent.keyboard("{Escape}");
    await waitFor(() => expect(onCancel).toHaveBeenCalled());
    cleanup();
    await waitFor(() =>
      expect(sent.find((one) => one.cmd === "forge_ci_permission_cancel")?.args).toEqual({
        flow: 7,
      }),
    );
  });
});
