// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * Draft skills in the Skill studio (SK-9, #2772): a chat's draft listed with its files, Accept and
 * Reject asked of the core, a stale draft that cannot be accepted, Try it and "Ask a chat to draft
 * a skill" asked of the project's window, and the chat's tab saying only that the draft is ready.
 * The core is mocked; what each command does is the core's seam test.
 */
import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { forgetStudio, renderStudio } from "./SkillStudioTab.testkit";
import { DraftReadyNotice } from "./DraftReadyNotice";
import { AT_THE_ROOT, inWorkspace } from "./skillStudio";
import { SKILL_CHAT, draftReadySays, type SkillChatAsk } from "./skillDraftAsks";

afterEach(() => {
  cleanup();
  clearMocks();
  forgetStudio();
});

const draft = (name: string, over: Record<string, unknown> = {}) => ({
  id: `${name}-id`,
  name,
  level: { at: "project", local: false },
  chat: 4,
  new: true,
  stale: false,
  files: [
    { path: "SKILL.md", before: null, after: "---\nname: x\n---\n", hunks: [], stale: false },
  ],
  ...over,
});

/** A core with `drafts` waiting, recording every ask. */
function core(drafts: unknown[]) {
  const asked: { cmd: string; args: Record<string, unknown> }[] = [];
  mockIPC((cmd, args) => {
    const given = (args ?? {}) as Record<string, unknown>;
    asked.push({ cmd, args: given });
    if (cmd === "skill_studio")
      return {
        on: true,
        workspace: (given.workspace as string | null) ?? null,
        persona: null,
        first: { starts: false, unmoved: null },
        workspaces: ["ops"],
        levels: [{ level: { at: "project", local: false }, label: "project" }],
        harnesses: [],
        clashes: [],
        skills: [],
      };
    if (cmd === "skill_drafts") return drafts;
    if (cmd === "skill_studio_on") return true;
    if (cmd === "skills_deleted") return [];
    if (cmd === "start_options") return { profiles: [{ name: "claude" }] };
    if (cmd === "skill_draft_reject") return null;
    return { notes: [] };
  });
  return asked;
}

/** Every ask the studio made of the project's window, until `stop` is called. */
function windowAsks(): { asks: SkillChatAsk[]; stop: () => void } {
  const asks: SkillChatAsk[] = [];
  const heard = (event: Event) => asks.push((event as CustomEvent<SkillChatAsk>).detail);
  window.addEventListener(SKILL_CHAT, heard);
  return { asks, stop: () => window.removeEventListener(SKILL_CHAT, heard) };
}

describe("draft skills in the Skill studio", () => {
  it("lists a chat's draft, and asks the core to accept and to reject one", async () => {
    const asked = core([draft("triage"), draft("deploy", { new: false })]);
    const user = userEvent.setup();
    renderStudio(AT_THE_ROOT, "drafts");

    const row = await screen.findByTestId("draft-triage");
    expect(row).toHaveTextContent("a new skill");
    expect(screen.getByTestId("draft-deploy")).toHaveTextContent("a change to it");

    await user.click(screen.getByRole("button", { name: "Accept triage" }));
    await waitFor(() =>
      expect(asked.find((one) => one.cmd === "skill_draft_accept")?.args).toMatchObject({
        plane: "p",
        id: "triage-id",
      }),
    );
    await user.click(screen.getByRole("button", { name: "Reject deploy" }));
    await waitFor(() =>
      expect(asked.find((one) => one.cmd === "skill_draft_reject")?.args).toMatchObject({
        plane: "p",
        id: "deploy-id",
      }),
    );
  });

  it("says a stale draft cannot be accepted, and offers no Accept", async () => {
    core([draft("triage", { stale: true })]);
    renderStudio(AT_THE_ROOT, "drafts");
    expect(await screen.findByTestId("draft-stale-triage")).toHaveTextContent(
      "changed since the chat drafted it",
    );
    expect(screen.getByRole("button", { name: "Accept triage" })).toBeDisabled();
  });

  it("asks the project's window for Try it and for a chat to draft one, at the level it came from", async () => {
    core([draft("triage")]);
    const { asks, stop } = windowAsks();
    const user = userEvent.setup();
    renderStudio(inWorkspace("ops"), "drafts");

    await user.click(await screen.findByRole("button", { name: "Try triage" }));
    await user.click(screen.getByRole("button", { name: "Ask a chat to draft a skill" }));
    stop();
    expect(asks).toEqual([
      { plane: "p", kind: "try", id: "triage-id", profile: null },
      { plane: "p", kind: "draft", level: { at: "workspace", name: "ops", local: false } },
    ]);
  });
});

describe("the chat's tab", () => {
  it("says only that its draft is ready in the Skill studio", async () => {
    core([draft("triage"), draft("other", { chat: 9 })]);
    render(<DraftReadyNotice plane="p" session={4} />);
    expect(await screen.findByText(draftReadySays(["triage"], false))).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Open Skill studio" })).toBeInTheDocument();
  });

  it("says nothing for a chat that drafted nothing", async () => {
    const asked = core([draft("triage")]);
    render(<DraftReadyNotice plane="p" session={5} />);
    await waitFor(() => expect(asked.some((one) => one.cmd === "skill_drafts")).toBe(true));
    expect(screen.queryByText(/drafted/)).not.toBeInTheDocument();
  });
});
