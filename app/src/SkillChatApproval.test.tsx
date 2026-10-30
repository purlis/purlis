// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **Approve and start, in place** (#3942) at the window's seam: what a studio start the core
 * refused offers, from the approval the core read and never from the refusal's words, and the question that
 * shows the picker's sentence and line, approves that line alone on the person's answer, and only
 * then starts the same chat again. The core is faked at its command boundary.
 */
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { NeedsApproval } from "./bindings";
import type { SkillChatAsk } from "./skillDraftAsks";
import {
  SkillChatApproveAsk,
  skillChatRefused,
  startedAgain,
  studioStartNamed,
} from "./SkillChatApproval";

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

const TRY: SkillChatAsk = { plane: PLANE, kind: "try", id: "d1", profile: null };
const DRAFT: SkillChatAsk = {
  plane: PLANE,
  kind: "draft",
  level: { at: "project", local: false },
  what: "",
};

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

describe("a studio start refused for its profile's approval", () => {
  it("offers Approve and start… beside the refusal, from the approval the core read", () => {
    const ask = vi.fn();
    const wanted = { ...TRY, profile: "work" };
    const said = skillChatRefused(wanted, { words: REFUSED, approval: WORK }, ask);

    expect(said).toMatchObject({ from: "skill-chat", refused: true, words: REFUSED });
    expect(said.ways?.map((one) => one.label)).toEqual(["Approve and start…"]);
    // Offered, and nothing is approved until the person answers its question.
    said.ways?.[0].onPress();
    expect(ask).toHaveBeenCalledWith({ wanted, approval: WORK });
  });

  it("says the refusal alone where there is nothing to approve, or only its words came", () => {
    expect(skillChatRefused(DRAFT, { words: "no profile", approval: null }, vi.fn())).toEqual({
      from: "skill-chat",
      refused: true,
      words: "no profile",
    });
    // A refusal that names approval in its words alone offers nothing: the words are not read.
    expect(skillChatRefused(DRAFT, REFUSED, vi.fn())).toEqual({
      from: "skill-chat",
      refused: true,
      words: REFUSED,
    });
  });
});

describe("the question Approve and start… asks", () => {
  it("shows the picker's sentence and line, approves that line, then starts the same chat", async () => {
    const asked = core();
    const onApproved = vi.fn();
    const user = userEvent.setup();
    render(
      <SkillChatApproveAsk
        plane={PLANE}
        asked={{ wanted: TRY, approval: WORK }}
        onCancel={vi.fn()}
        onApproved={onApproved}
      />,
    );
    const question = screen.getByRole("alertdialog", { name: "Approve work?" });
    expect(question).toHaveTextContent(
      "Try it starts a chat on the profile work. Approving records that purlis may run its command on this machine, then starts the chat with its request typed and not sent.",
    );
    expect(question).toHaveTextContent(
      "purlis has not run this profile before. It would run: CLAUDE_CONFIG_DIR=~/.claude-work claude",
    );
    expect(onApproved).not.toHaveBeenCalled();

    await user.click(within(question).getByRole("button", { name: "Approve and start" }));

    await waitFor(() => expect(onApproved).toHaveBeenCalledTimes(1));
    expect(asked.find((one) => one.cmd === "approve_profile")?.args).toEqual({
      plane: PLANE,
      name: "work",
      shown: WORK.shown,
    });
    // Try it starts again on the profile just approved.
    expect(onApproved).toHaveBeenCalledWith({ ...TRY, profile: "work" });
  });

  it("keeps a refused approval in the question, and starts nothing", async () => {
    core(() => {
      throw "profile 'work' changed while you were reading it, so nothing was approved";
    });
    const onApproved = vi.fn();
    const user = userEvent.setup();
    render(
      <SkillChatApproveAsk
        plane={PLANE}
        asked={{ wanted: DRAFT, approval: WORK }}
        onCancel={vi.fn()}
        onApproved={onApproved}
      />,
    );
    await user.click(screen.getByRole("button", { name: "Approve and start" }));
    expect(
      await screen.findByText(/changed while you were reading it, so nothing was approved/),
    ).toBeInTheDocument();
    expect(onApproved).not.toHaveBeenCalled();
  });

  it("Cancel approves nothing", async () => {
    const asked = core();
    const onCancel = vi.fn();
    const user = userEvent.setup();
    render(
      <SkillChatApproveAsk
        plane={PLANE}
        asked={{ wanted: DRAFT, approval: WORK }}
        onCancel={onCancel}
        onApproved={vi.fn()}
      />,
    );
    await user.click(screen.getByRole("button", { name: "Cancel" }));
    expect(onCancel).toHaveBeenCalled();
    expect(asked.some((one) => one.cmd === "approve_profile")).toBe(false);
  });
});

describe("what it names and starts again", () => {
  it("names each studio start, and starts Try it again on the approved profile", () => {
    expect(studioStartNamed(DRAFT)).toBe("Ask a chat to draft a skill");
    expect(studioStartNamed(TRY)).toBe("Try it");
    expect(
      studioStartNamed({ plane: PLANE, kind: "redo", id: "d1", name: "deploy", chat: null }),
    ).toBe("Ask a chat to redo deploy");
    expect(
      studioStartNamed({
        plane: PLANE,
        kind: "triggers",
        level: { at: "project", local: false },
        name: "deploy",
      }),
    ).toBe("Ask a chat to write the trigger tests of deploy");
    expect(startedAgain(TRY, "work")).toEqual({ ...TRY, profile: "work" });
    expect(startedAgain(DRAFT, "work")).toBe(DRAFT);
  });
});
