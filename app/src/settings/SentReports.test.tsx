// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, beforeEach, expect, it } from "vitest";
import { cleanup, render, screen, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { SentReport } from "../bindings";
import { SentReports } from "./SentReports";

/**
 * **The sent list** (#5689): every report this machine sent, as the core lists it
 * (`reports_sent`), each opening exactly the bytes that left, not a summary of them.
 */

const BYTES =
  '{"event_id":"9f1c","sent_at":"2026-11-08T03:19:00Z"}\n' +
  '{"type":"event","length":40}\n' +
  '{"level":"fatal","culprit":"crates/purlis-core/src/plane.rs:12:5"}\n';

const SENT: SentReport = {
  at: "2026-11-08T03:19:00Z",
  title: "purlis panicked at crates/purlis-core/src/plane.rs:12:5",
  to: "purlis's report service",
  address: "purlis's report service, event 9f1c",
  bytes: BYTES,
  waiting: false,
};

let sent: SentReport[] = [];

beforeEach(() => {
  sent = [];
  mockIPC((cmd) => (cmd === "reports_sent" ? sent : null));
});
afterEach(() => {
  cleanup();
  clearMocks();
});

const ids = { id: "sent", labelledBy: "sent-label", describedBy: "sent-help" };

it("an entry of the sent list opens exactly the bytes that were sent", async () => {
  sent = [SENT];
  render(<SentReports ids={ids} />);

  const row = await screen.findByRole("row", { name: new RegExp(SENT.title) });
  expect(within(row).getByText(/purlis's report service/)).toBeTruthy();
  await userEvent.click(row);

  const dialog = await screen.findByRole("dialog", { name: SENT.title });
  // The bytes as one text, whatever element holds them: not a summary, not a line of them.
  const [shown] = within(dialog).getAllByText((_, node) => node?.textContent === BYTES, {
    selector: "pre, code",
  });
  expect(shown.textContent).toBe(BYTES);
});

it("a machine that sent nothing says so", async () => {
  render(<SentReports ids={ids} />);
  expect(await screen.findByText("No report has been sent from this machine.")).toBeTruthy();
});

it("a report made offline is listed as waiting to send, and opens the bytes that will go", async () => {
  sent = [{ ...SENT, address: "", waiting: true }];
  render(<SentReports ids={ids} />);

  const row = await screen.findByRole("row", { name: new RegExp(SENT.title) });
  expect(within(row).getByText(/Waiting to send/)).toBeTruthy();
  await userEvent.click(row);

  const dialog = await screen.findByRole("dialog", { name: SENT.title });
  expect(within(dialog).getByText(/will be sent to purlis's report service/)).toBeTruthy();
});
