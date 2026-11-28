// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { ChatMove } from "./bindings";
import {
  chatMovesOf,
  dismissMovedAfterSignIn,
  forgetMove,
  heardMove,
  movedAfterSignIn,
} from "./chatMoves";
import { forgetDismissals, useHiddenForNow } from "./dismissals";
import { DraftReadyNotice } from "./DraftReadyNotice";
import { forgetInputRefused, heardInputRefused } from "./inputRefused";
import { InputRefusedNotice } from "./InputRefusedNotice";
import { MoveNotice } from "./MoveNotice";
import { letGoHere } from "./notesClearedEverywhere";
import { forgetDraftsPutAway } from "./skillDraftAsks";

/**
 * **A note put away in one window is put away in every window of the project** (#5363, after
 * #5225): the notes kept outside PlaneView's state, in a store of the window's own, say so too.
 * Each window hears what a chat says from when it is listening, so another may hold the same
 * note for the chat coming back to it, and must not say it again.
 */

const PLANE = "/home/op/project";

let told: unknown[];
let drafts: unknown[];

beforeEach(() => {
  told = [];
  drafts = [];
  mockIPC((cmd, args) => {
    const given = (args ?? {}) as { event?: string; payload?: unknown };
    if (cmd === "plugin:event|emit" && given.event === "note-cleared") told.push(given.payload);
    if (cmd === "plugin:event|listen") return 1;
    if (cmd === "skill_drafts") return drafts;
    return null;
  });
});
afterEach(() => {
  cleanup();
  forgetInputRefused(PLANE, 3);
  forgetMove(PLANE, 3);
  dismissMovedAfterSignIn(PLANE, 8);
  forgetDismissals();
  forgetDraftsPutAway();
  clearMocks();
});

const failed = (): ChatMove => ({
  session: 3,
  from: null,
  to: "builds",
  stage: "failed",
  why: "the pool is full",
  limit: null,
  with: [],
  sign_in: null,
});

/** A draft of chat 3's, as the core lists it; only its id, name and chat are read here. */
const draft = (name: string) => ({
  id: `${name}-id`,
  name,
  level: { at: "project", local: false },
  chat: 3,
  chat_open: true,
  new: true,
  stale: false,
  files: [],
});

const dismiss = async () =>
  userEvent.setup().click(await screen.findByRole("button", { name: "Dismiss" }));

describe("put away here, it is told to every window of the project", () => {
  it("a send that was refused", async () => {
    heardInputRefused({ plane: PLANE, session: 3, why: "the runner is reconnecting" });
    render(<InputRefusedNotice plane={PLANE} session={3} />);
    await dismiss();
    await waitFor(() =>
      expect(told).toEqual([
        { plane: PLANE, session: 3, note: { kind: "input-refused" }, from: "main" },
      ]),
    );
  });

  it("a move that did not happen", async () => {
    heardMove(PLANE, failed());
    render(<MoveNotice plane={PLANE} session={3} />);
    await dismiss();
    await waitFor(() =>
      expect(told).toEqual([{ plane: PLANE, session: 3, note: { kind: "move" }, from: "main" }]),
    );
  });

  it("a move being made, whose line is only hidden", async () => {
    heardMove(PLANE, { ...failed(), stage: "switching", why: null });
    render(<MoveNotice plane={PLANE} session={3} />);
    await dismiss();
    await waitFor(() =>
      expect(told).toEqual([{ plane: PLANE, session: 3, note: { kind: "move" }, from: "main" }]),
    );
  });

  it("a move that went ahead after its sign-in", async () => {
    heardMove(PLANE, { ...failed(), stage: "signing-in", why: null });
    movedAfterSignIn(PLANE, 3, 8);
    render(<MoveNotice plane={PLANE} session={8} />);
    await dismiss();
    await waitFor(() =>
      expect(told).toEqual([{ plane: PLANE, session: 8, note: { kind: "moved" }, from: "main" }]),
    );
  });

  it("a draft ready in the Skill studio", async () => {
    drafts = [draft("triage"), draft("tidy")];
    render(<DraftReadyNotice plane={PLANE} session={3} />);
    await dismiss();
    await waitFor(() =>
      expect(told).toEqual([
        {
          plane: PLANE,
          session: 3,
          note: { kind: "draft-ready", drafts: "triage\ntidy" },
          from: "main",
        },
      ]),
    );
  });
});

describe("put away in another window, it is let go here", () => {
  it("a send that was refused", async () => {
    heardInputRefused({ plane: PLANE, session: 3, why: "the runner is reconnecting" });
    letGoHere(PLANE, 3, { kind: "input-refused" });
    render(<InputRefusedNotice plane={PLANE} session={3} />);
    expect(screen.queryByText(/could not send/)).toBeNull();
  });

  it("a move, failed or being made", () => {
    heardMove(PLANE, failed());
    letGoHere(PLANE, 3, { kind: "move" });
    expect(chatMovesOf(PLANE)[3]).toBeUndefined();
  });

  it("a move that went ahead after its sign-in", () => {
    heardMove(PLANE, { ...failed(), stage: "signing-in", why: null });
    movedAfterSignIn(PLANE, 3, 8);
    letGoHere(PLANE, 8, { kind: "moved" });
    render(<MoveNotice plane={PLANE} session={8} />);
    expect(screen.queryByText(/Moved to/)).toBeNull();
  });

  it("a line hidden for this wait", () => {
    letGoHere(PLANE, 3, { kind: "hidden", causes: ["task-permission:3:ask-1"] });
    function Probe() {
      return <span>{useHiddenForNow(PLANE, "task-permission:3:ask-1") ? "hidden" : "shown"}</span>;
    }
    render(<Probe />);
    expect(screen.getByText("hidden")).toBeInTheDocument();
  });

  it("a draft ready, until the chat drafts again", async () => {
    drafts = [draft("triage"), { ...draft("other"), chat: 4 }];
    letGoHere(PLANE, 3, { kind: "draft-ready", drafts: "triage" });
    const { unmount } = render(
      <>
        <DraftReadyNotice plane={PLANE} session={3} />
        <DraftReadyNotice plane={PLANE} session={4} />
      </>,
    );
    // Chat 4's says its draft once the drafts are read, and chat 3's says nothing.
    await screen.findByRole("button", { name: "Open the draft" });
    expect(screen.getAllByRole("button", { name: "Open the draft" })).toHaveLength(1);
    expect(screen.getByText(/other/)).toBeInTheDocument();
    unmount();

    drafts = [draft("triage"), draft("tidy")];
    render(<DraftReadyNotice plane={PLANE} session={3} />);
    expect(await screen.findByRole("button", { name: "Open the drafts" })).toBeInTheDocument();
  });

  it("a note of the same chat in another project stays", async () => {
    heardInputRefused({ plane: PLANE, session: 3, why: "the runner is reconnecting" });
    letGoHere("/home/op/other", 3, { kind: "input-refused" });
    render(<InputRefusedNotice plane={PLANE} session={3} />);
    expect(await screen.findByText(/could not send/)).toBeInTheDocument();
  });
});
