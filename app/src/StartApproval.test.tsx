// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **Approve and start, in place** (#3942, #3968) at the window's seam: what a start the core
 * refused offers, from the approval the core read and never from the refusal's words, and the
 * question that shows the picker's sentence and line, approves that line alone on the person's
 * answer, and only then starts the same chat again. The core is faked at its command boundary.
 */
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { NeedsApproval } from "./bindings";
import { saidOf } from "./actions";
import {
  StartApproveAsk,
  startRefused,
  startRefusedRan,
  type StartAgain,
  type StartApproving,
} from "./StartApproval";

afterEach(() => {
  cleanup();
  clearMocks();
});

const PLANE = "/home/dev/plane";
const REFUSED =
  "profile 'work' is new, and nobody has approved it — nothing was started. Approve it once so purlis can run it.";

const WORK: NeedsApproval = {
  profile: "work",
  kind: "claude",
  source: "purlis.local.toml",
  approval: "new",
  shown: "CLAUDE_CONFIG_DIR=~/.claude-work claude",
};

/** A curation action's start, which starts again with `again`. */
function curation(again = vi.fn()): StartAgain {
  return { named: "The curation action Tidy", typed: "its prompt", again };
}

type Asked = { cmd: string; args: Record<string, unknown> }[];

/** A core answering `approve` for a profile's approval. */
function core(approve: () => unknown = () => null): Asked {
  const asked: Asked = [];
  mockIPC((cmd, args) => {
    asked.push({ cmd, args: (args ?? {}) as Record<string, unknown> });
    if (cmd === "approve_profile") return approve();
    return null;
  });
  return asked;
}

describe("a start refused for its profile's approval", () => {
  it("offers Approve and start… beside the refusal, from the approval the core read", () => {
    const ask = vi.fn();
    const start = curation();
    const said = startRefused({ words: REFUSED, approval: WORK }, start, ask);

    expect(said.words).toBe(REFUSED);
    expect(said.ways?.map((one) => one.label)).toEqual(["Approve and start…"]);
    // Offered, and nothing is approved or started until the person answers its question.
    said.ways?.[0].onPress();
    expect(ask).toHaveBeenCalledWith({ ...start, approval: WORK });
    expect(start.again).not.toHaveBeenCalled();
  });

  it("says the refusal alone where there is nothing to approve, or only its words came", () => {
    expect(startRefused({ words: "no profile", approval: null }, curation(), vi.fn())).toEqual({
      words: "no profile",
    });
    // A refusal that names approval in its words alone offers nothing: the words are not read.
    expect(startRefused(REFUSED, curation(), vi.fn())).toEqual({ words: REFUSED });
  });

  it("as a row's answer, keeps its way into the notice the row's refusal is said in", () => {
    const ran = startRefusedRan({ words: REFUSED, approval: WORK }, curation(), vi.fn());

    const said = saidOf("curate:workspace:alpha/ops/tidy", ran);

    expect(said).toMatchObject({ refused: true, words: REFUSED });
    expect(said?.ways?.map((one) => one.label)).toEqual(["Approve and start…"]);
    // Nothing to approve: the row's refusal as it always was.
    expect(saidOf("file.chat", startRefusedRan("gone", curation(), vi.fn()))).toEqual({
      from: "file.chat",
      refused: true,
      words: "gone",
    });
  });
});

describe("the question Approve and start… asks", () => {
  /** The question for `asked`, answered by `onApproved` and `onCancel`. */
  function asking(asked: StartApproving, onApproved = vi.fn(), onCancel = vi.fn()) {
    render(
      <StartApproveAsk plane={PLANE} asked={asked} onCancel={onCancel} onApproved={onApproved} />,
    );
    return { onApproved, onCancel };
  }

  it("shows the picker's sentence and line, approves that line, then starts the same chat", async () => {
    const asked = core();
    const again = vi.fn();
    const user = userEvent.setup();
    const { onApproved } = asking({ ...curation(again), approval: WORK });
    const question = screen.getByRole("alertdialog", { name: "Approve work?" });
    expect(question).toHaveTextContent(
      "The curation action Tidy starts a chat on the profile work. Approving records that purlis may run its command on this machine, then starts the chat with its prompt typed and not sent.",
    );
    expect(question).toHaveTextContent(
      "purlis has not run this profile before. It would run: CLAUDE_CONFIG_DIR=~/.claude-work claude",
    );
    expect(again).not.toHaveBeenCalled();

    await user.click(within(question).getByRole("button", { name: "Approve and start" }));

    await waitFor(() => expect(again).toHaveBeenCalledTimes(1));
    expect(asked.find((one) => one.cmd === "approve_profile")?.args).toEqual({
      plane: PLANE,
      name: "work",
      shown: WORK.shown,
    });
    expect(onApproved).toHaveBeenCalledTimes(1);
    // On the profile just approved.
    expect(again).toHaveBeenCalledWith("work");
  });

  it("keeps a refused approval in the question, and starts nothing", async () => {
    core(() => {
      throw "profile 'work' changed while you were reading it, so nothing was approved";
    });
    const again = vi.fn();
    const user = userEvent.setup();
    const { onApproved } = asking({ ...curation(again), approval: WORK });
    await user.click(screen.getByRole("button", { name: "Approve and start" }));
    expect(
      await screen.findByText(/changed while you were reading it, so nothing was approved/),
    ).toBeInTheDocument();
    expect(onApproved).not.toHaveBeenCalled();
    expect(again).not.toHaveBeenCalled();
  });

  it("Cancel approves nothing", async () => {
    const asked = core();
    const again = vi.fn();
    const user = userEvent.setup();
    const { onCancel } = asking({ ...curation(again), approval: WORK });
    await user.click(screen.getByRole("button", { name: "Cancel" }));
    expect(onCancel).toHaveBeenCalled();
    expect(asked.some((one) => one.cmd === "approve_profile")).toBe(false);
    expect(again).not.toHaveBeenCalled();
  });
});
