// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, render, screen, waitFor, within } from "@testing-library/react";
import { emit } from "@tauri-apps/api/event";
import userEvent from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { DraftFiled, ReportDraft } from "./bindings";
import { Inbox } from "./Inbox";
import {
  ALWAYS_SEND_FORBIDDEN,
  ReportDraftNotices,
  WRITTEN_BY_AN_AGENT,
} from "./ReportDraftNotices";
import { forgetReportConsent, UNANSWERED, type Consent } from "./reportAnswers";

/**
 * **A chat's report draft in the Inbox** (#5694): the card lists what the core holds, and every
 * press reaches the core as the command that does it. The core is a mock at its command boundary
 * that answers File with what each case gives it, and keeps every command it was sent.
 */

const DRAFT: ReportDraft = {
  id: "d1",
  at: "2026-11-08T10:00:00Z",
  feature: false,
  title: "pin refuses a workspace it just listed",
  body: "It said the branch had changes.",
  line: "purlis pin · exit 1 · purlis 0.41.2 · Darwin 27.2.0",
  scrubbed: [],
  chat: "steward 4",
  session: 4,
  edited: false,
  digest: "0a1b2c3d4e5f",
  sends_at: null,
};

let sent: { cmd: string; args: Record<string, unknown> }[] = [];

function core(filed: DraftFiled[], { membersMayAlwaysSend = true } = {}) {
  sent = [];
  mockIPC((cmd, args) => {
    sent.push({ cmd, args: args as Record<string, unknown> });
    if (cmd === "report_drafts") return [DRAFT];
    if (cmd === "members_may_always_send") return membersMayAlwaysSend;
    if (cmd === "edit_report_draft") {
      const { title, body } = args as { title: string; body: string };
      return { ...DRAFT, title, body, edited: true, digest: "ffffffffffff" };
    }
    if (cmd === "file_report_draft") return filed.shift();
    return null;
  });
}

const of = (cmd: string) => sent.filter((one) => one.cmd === cmd).map((one) => one.args);

function drawn() {
  const onGo = vi.fn();
  const onSent = vi.fn();
  render(
    <Inbox
      plane="/plane"
      asks={[]}
      onGo={() => undefined}
      onLeave={() => undefined}
      onIgnore={() => undefined}
      notices={<ReportDraftNotices plane="/plane" onGo={onGo} onSent={onSent} />}
    />,
  );
  return { onGo, onSent };
}

const card = () => screen.queryByText(`Report draft: ${DRAFT.title}`);

afterEach(() => {
  cleanup();
  clearMocks();
  forgetReportConsent();
});

describe("a report draft in the Inbox (#5694)", () => {
  it("offers Always send off, saying why, where the project turns sending without asking off (#5696)", async () => {
    core([], { membersMayAlwaysSend: false });
    drawn();
    await waitFor(() => expect(card()).not.toBeNull());
    const always = screen.getByRole("button", { name: "Always send" });
    await waitFor(() => expect(always).toHaveAttribute("aria-disabled", "true"));

    // Off, the keyboard still reaches it, and its tooltip says why.
    for (let i = 0; i < 8 && document.activeElement !== always; i++) await userEvent.tab();
    expect(document.activeElement).toBe(always);
    expect(await screen.findByRole("tooltip")).toHaveTextContent(ALWAYS_SEND_FORBIDDEN);
    await userEvent.click(always);

    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(of("always_send_report_draft")).toEqual([]);
  });

  it("offers Always send where the project lets its members send without asking", async () => {
    core([]);
    drawn();
    await waitFor(() => expect(card()).not.toBeNull());
    await waitFor(() => expect(of("members_may_always_send")).toEqual([{ plane: "/plane" }]));
    expect(screen.getByRole("button", { name: "Always send" })).not.toHaveAttribute(
      "aria-disabled",
    );
  });

  it("says a draft the report service did not answer waits to send", async () => {
    core([{ kind: "queued", title: DRAFT.title }]);
    const { onSent } = drawn();
    await waitFor(() => expect(card()).not.toBeNull());
    await userEvent.click(screen.getByRole("button", { name: "File" }));
    await waitFor(() =>
      expect(onSent).toHaveBeenCalledWith(expect.objectContaining({ id: "d1", to: "waiting" })),
    );
  });

  it("names the chat it came from and its line, and Go to chat goes there", async () => {
    core([]);
    const { onGo } = drawn();
    await waitFor(() => expect(card()).not.toBeNull());
    expect(screen.getByText("steward 4")).toBeTruthy();
    expect(screen.getByText(DRAFT.line)).toBeTruthy();
    await userEvent.click(screen.getByRole("button", { name: "Go to chat steward 4" }));
    expect(onGo).toHaveBeenCalledWith(4);
  });

  it("reads whole, marked as an agent's, in fields the person can change", async () => {
    core([]);
    drawn();
    await waitFor(() => expect(card()).not.toBeNull());
    await userEvent.click(screen.getByRole("button", { name: "Read" }));
    await waitFor(() => expect(screen.getByText(WRITTEN_BY_AN_AGENT)).toBeTruthy());
    expect(screen.getByRole("textbox", { name: "Title" })).toHaveProperty("value", DRAFT.title);
    expect(screen.getByRole("textbox", { name: "Body" })).toHaveProperty("value", DRAFT.body);
  });

  it("files with the duplicate check and leaves the Inbox once sent", async () => {
    core([{ kind: "sent", to: "intake", address: "event 9f", title: DRAFT.title }]);
    const { onSent } = drawn();
    await waitFor(() => expect(card()).not.toBeNull());
    await userEvent.click(screen.getByRole("button", { name: "File" }));
    await waitFor(() => expect(onSent).toHaveBeenCalled());
    expect(of("file_report_draft")).toEqual([
      { id: "d1", digest: DRAFT.digest, how: { how: "check" } },
    ]);
    expect(onSent.mock.calls[0]?.[0]).toMatchObject({
      id: "d1",
      title: DRAFT.title,
      session: 4,
      chat: "steward 4",
      to: "intake",
    });
    await waitFor(() => expect(card()).toBeNull());
  });

  it("keeps the person's changed words first and files them by their new digest", async () => {
    core([{ kind: "sent", to: "github", address: "issues/42", title: "Shorter" }]);
    const { onSent } = drawn();
    await waitFor(() => expect(card()).not.toBeNull());
    await userEvent.click(screen.getByRole("button", { name: "Read" }));
    const title = await waitFor(() => screen.getByRole("textbox", { name: "Title" }));
    await userEvent.clear(title);
    await userEvent.type(title, "Shorter");
    await userEvent.click(screen.getByRole("button", { name: "File" }));
    await waitFor(() => expect(onSent).toHaveBeenCalled());
    expect(of("edit_report_draft")).toEqual([{ id: "d1", title: "Shorter", body: DRAFT.body }]);
    expect(of("file_report_draft")).toEqual([
      { id: "d1", digest: "ffffffffffff", how: { how: "check" } },
    ]);
  });

  it("shows possible duplicates first, and Add to adds it to that issue instead", async () => {
    core([
      {
        kind: "duplicates",
        issues: [{ number: 41, title: "Refuses clean branches", state: "open", url: "u" }],
      },
      { kind: "sent", to: "github", address: "issues/41#c", title: DRAFT.title },
    ]);
    const { onSent } = drawn();
    await waitFor(() => expect(card()).not.toBeNull());
    await userEvent.click(screen.getByRole("button", { name: "File" }));
    const add = await waitFor(() => screen.getByRole("button", { name: "Add to #41" }));
    expect(screen.getByRole("button", { name: "File as new" })).toBeTruthy();
    expect(onSent).not.toHaveBeenCalled();
    await userEvent.click(add);
    await waitFor(() => expect(onSent).toHaveBeenCalled());
    expect(of("file_report_draft")[1]).toEqual({
      id: "d1",
      digest: DRAFT.digest,
      how: { how: "add-to", issue: 41 },
    });
  });

  it("is let go unsent on Dismiss", async () => {
    core([]);
    const { onSent } = drawn();
    await waitFor(() => expect(card()).not.toBeNull());
    await userEvent.click(screen.getByRole("button", { name: "Dismiss" }));
    await waitFor(() => expect(card()).toBeNull());
    expect(of("dismiss_report_draft")).toEqual([{ id: "d1" }]);
    expect(of("file_report_draft")).toEqual([]);
    expect(onSent).not.toHaveBeenCalled();
  });
});

describe("Always send on a report draft (#5695)", () => {
  /** A core holding `drafts` (changed by the test), whose answers are `consent`. */
  function sending(consent: Consent) {
    let kept = consent;
    let drafts: ReportDraft[] = [DRAFT];
    sent = [];
    forgetReportConsent();
    mockIPC(
      (cmd, args) => {
        sent.push({ cmd, args: args as Record<string, unknown> });
        if (cmd === "report_drafts") return drafts;
        if (cmd === "members_may_always_send") return true;
        if (cmd === "report_consent") return kept;
        if (cmd === "keep_report_consent") kept = (args as { consent: Consent }).consent;
        if (cmd === "always_send_report_draft") {
          drafts = [{ ...DRAFT, sends_at: "2026-11-08T10:00:10Z" }];
          return drafts[0];
        }
        return null;
      },
      { shouldMockEvents: true },
    );
    return {
      kept: () => kept,
      /** The core's list is now `next`, and it tells the windows so. */
      becomes: async (next: ReportDraft[]) => {
        drafts = next;
        await act(async () => {
          await emit("report-drafts-changed", null);
          await new Promise((done) => setTimeout(done, 50));
        });
      },
    };
  }

  const allowed: Consent = { ...UNANSWERED, agent_drafts: true, asked: true };

  it("asks once, and Cancel keeps the draft and sends nothing", async () => {
    sending(allowed);
    drawn();
    await waitFor(() => expect(card()).not.toBeNull());
    await userEvent.click(screen.getByRole("button", { name: "Always send" }));
    const dialog = await screen.findByRole("dialog", { name: "Always send report drafts?" });
    expect(dialog.textContent).toMatch(/private report service/);
    expect(dialog.textContent).toMatch(/Settings › You › Reports/);
    await userEvent.click(screen.getByRole("button", { name: "Cancel" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    expect(of("always_send_report_draft")).toEqual([]);
    expect(of("keep_report_consent")).toEqual([]);
    expect(card()).not.toBeNull();
  });

  it("keeps the standing yes and holds the draft to send once the person agrees", async () => {
    const core = sending(allowed);
    drawn();
    await waitFor(() => expect(card()).not.toBeNull());
    await userEvent.click(screen.getByRole("button", { name: "Always send" }));
    await screen.findByRole("dialog", { name: "Always send report drafts?" });
    await userEvent.click(
      within(screen.getByRole("dialog")).getByRole("button", { name: "Always send" }),
    );
    await waitFor(() => expect(of("always_send_report_draft")).toEqual([{ id: "d1" }]));
    expect(core.kept().always_send).toBe(true);
  });

  it("asks nothing once the person said yes before", async () => {
    sending({ ...allowed, always_send: true });
    drawn();
    await waitFor(() => expect(card()).not.toBeNull());
    await userEvent.click(screen.getByRole("button", { name: "Always send" }));
    await waitFor(() => expect(of("always_send_report_draft")).toEqual([{ id: "d1" }]));
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("draws a draft held to send as no card, but as sent with its hold, then as sent", async () => {
    const core = sending({ ...allowed, always_send: true });
    const onSent = vi.fn();
    const onUnsent = vi.fn();
    render(
      <ReportDraftNotices
        plane="/plane"
        onGo={() => undefined}
        onSent={onSent}
        onUnsent={onUnsent}
      />,
    );
    await waitFor(() => expect(card()).not.toBeNull());

    await core.becomes([{ ...DRAFT, sends_at: "2026-11-08T10:00:10Z" }]);
    await waitFor(() => expect(card()).toBeNull());
    expect(onSent).toHaveBeenLastCalledWith(
      expect.objectContaining({ id: "d1", to: "intake", holding: true }),
    );

    await core.becomes([]);
    expect(onSent).toHaveBeenLastCalledWith(
      expect.objectContaining({ id: "d1", to: "intake", holding: false }),
    );
    expect(onUnsent).not.toHaveBeenCalled();
  });

  it("hands back a draft whose hold was undone or capped, which is a card again", async () => {
    const core = sending({ ...allowed, always_send: true });
    const onUnsent = vi.fn();
    render(
      <ReportDraftNotices
        plane="/plane"
        onGo={() => undefined}
        onSent={vi.fn()}
        onUnsent={onUnsent}
      />,
    );
    await core.becomes([{ ...DRAFT, sends_at: "2026-11-08T10:00:10Z" }]);
    await waitFor(() => expect(card()).toBeNull());
    await core.becomes([DRAFT]);
    expect(onUnsent).toHaveBeenCalledWith("d1");
    await waitFor(() => expect(card()).not.toBeNull());
  });
});
