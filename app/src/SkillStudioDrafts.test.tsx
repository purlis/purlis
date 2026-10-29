// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The Drafts page and a draft's page** (S9, #3690; SK-9, #2772), at the window seam: the drafts
 * listed with their marks, a draft's page with its files, Try it, Accept and Discard, an
 * out-of-date draft's "Ask the chat to redo it", the page's empty state, and the chat's tab
 * opening its own draft. The core is faked at its command boundary; what each command does is
 * the core's seam test.
 */
import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { SkillStudioTab } from "./SkillStudioTab";
import { PLANE, STUDIO_HISTORY, forgetStudio, renderStudio } from "./SkillStudioTab.testkit";
import { openViewAt } from "./ui/panel/viewHistory";
import { DraftReadyNotice } from "./DraftReadyNotice";
import { AT_THE_ROOT, SKILL_STUDIO_ASKED, inWorkspace, type SkillStudioAsk } from "./skillStudio";
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
  chat_open: true,
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

/** Chooses the draft row `name` and answers its page once it is drawn. */
async function openDraft(name: string, user: ReturnType<typeof userEvent.setup>) {
  await user.click(await screen.findByTestId(`draft-${name}`));
  return screen.findByTestId("skill-studio-draft");
}

describe("the Drafts page", () => {
  it("lists each draft with its level and one mark, and what it writes under it", async () => {
    core([draft("triage"), draft("deploy", { new: false }), draft("lint", { stale: true })]);
    renderStudio(AT_THE_ROOT, "drafts");

    const triage = await screen.findByTestId("draft-triage");
    expect(triage).toHaveTextContent("project");
    expect(triage).toHaveTextContent("A new skill");
    expect(triage).toHaveTextContent("1 file · drafted by chat 4");
    expect(screen.getByTestId("draft-deploy")).toHaveTextContent("A change to it");
    expect(screen.getByTestId("draft-lint")).toHaveTextContent("Out of date");
    // Drafts is in the navigation with its count while drafts wait.
    expect(screen.getByRole("button", { name: /^Drafts/ })).toBeInTheDocument();
    expect(screen.getByTestId("skill-drafts-unpicked")).toBeInTheDocument();
  });

  it("opens a draft's page with its files, and accepts it into the Library", async () => {
    const asked = core([draft("triage"), draft("deploy", { new: false })]);
    const user = userEvent.setup();
    renderStudio(AT_THE_ROOT, "drafts");

    const page = await openDraft("triage", user);
    expect(within(page).getByTestId("draft-file-SKILL.md")).toBeInTheDocument();
    expect(page).toHaveTextContent(
      "Accept writes its 1 file into the project skills, in one commit.",
    );
    await user.click(within(page).getByRole("button", { name: "Accept triage" }));
    await waitFor(() =>
      expect(asked.find((one) => one.cmd === "skill_draft_accept")?.args).toMatchObject({
        plane: "p",
        id: "triage-id",
      }),
    );
    // Accepted, it is a skill: the Library is shown, at it.
    expect(await screen.findByRole("heading", { name: "Library" })).toBeInTheDocument();
  });

  it("discards a draft and stays on Drafts, saying so", async () => {
    const asked = core([draft("triage"), draft("deploy", { new: false })]);
    const user = userEvent.setup();
    renderStudio(AT_THE_ROOT, "drafts");

    const page = await openDraft("deploy", user);
    await user.click(within(page).getByRole("button", { name: "Discard deploy" }));
    await waitFor(() =>
      expect(asked.find((one) => one.cmd === "skill_draft_reject")?.args).toMatchObject({
        plane: "p",
        id: "deploy-id",
      }),
    );
    expect(await screen.findByRole("status", { name: /Draft discarded/ })).toHaveTextContent(
      "deploy is discarded.",
    );
    expect(screen.getByRole("heading", { name: "Drafts" })).toBeInTheDocument();
  });

  it("says an out-of-date draft needs a decision, and offers the redo in place of Accept", async () => {
    core([draft("triage", { stale: true }), draft("lint", { stale: true, chat_open: false })]);
    const { asks, stop } = windowAsks();
    const user = userEvent.setup();
    renderStudio(AT_THE_ROOT, "drafts");

    const page = await openDraft("triage", user);
    expect(within(page).getByTestId("draft-stale-triage")).toHaveTextContent(
      "changed since chat 4 drafted it, so it can't be accepted",
    );
    expect(within(page).queryByRole("button", { name: /Accept/ })).not.toBeInTheDocument();
    expect(page).toHaveTextContent("Discard deletes the draft. The project is not changed.");
    await user.click(within(page).getByRole("button", { name: "Ask the chat to redo it" }));

    // Its chat closed: a chat is asked afresh.
    const lint = await openDraft("lint", user);
    await user.click(within(lint).getByRole("button", { name: "Ask a chat to redo it" }));
    stop();
    expect(asks).toEqual([
      { plane: "p", kind: "redo", id: "triage-id", name: "triage", chat: 4 },
      { plane: "p", kind: "redo", id: "lint-id", name: "lint", chat: null },
    ]);
  });

  it("asks the project's window for Try it and for a chat to draft one, at the level it came from", async () => {
    core([draft("triage")]);
    const { asks, stop } = windowAsks();
    const user = userEvent.setup();
    renderStudio(inWorkspace("ops"), "drafts");

    const page = await openDraft("triage", user);
    await user.click(within(page).getByRole("button", { name: "Try it" }));
    await user.click(screen.getByRole("button", { name: "Ask a chat to draft a skill" }));
    stop();
    expect(asks).toEqual([
      { plane: "p", kind: "try", id: "triage-id", profile: null },
      { plane: "p", kind: "draft", level: { at: "workspace", name: "ops", local: false } },
    ]);
  });

  it("opens at a draft an entry point names", async () => {
    core([draft("triage"), draft("deploy")]);
    openViewAt(STUDIO_HISTORY, { page: "drafts", scope: AT_THE_ROOT, item: "deploy-id" });
    render(<SkillStudioTab plane={PLANE} changed={0} />);
    const page = await screen.findByTestId("skill-studio-draft");
    expect(within(page).getByRole("button", { name: "Accept deploy" })).toBeInTheDocument();
  });

  it("says what appears there when no draft waits, with the one way to get one", async () => {
    core([]);
    renderStudio(AT_THE_ROOT, "drafts");
    const none = await screen.findByTestId("skill-drafts-none");
    expect(none).toHaveTextContent("No drafts");
    expect(
      within(none).getByRole("button", { name: "Ask a chat to draft a skill" }),
    ).toBeInTheDocument();
  });

  it("holds its rows while the drafts are read, never saying there are none", async () => {
    mockIPC((cmd) => {
      if (cmd === "skill_drafts") return new Promise(() => undefined);
      if (cmd === "skill_studio_on") return true;
      if (cmd === "skills_deleted") return [];
      if (cmd === "skill_studio")
        return {
          on: true,
          workspace: null,
          persona: null,
          first: { starts: false, unmoved: null },
          workspaces: [],
          levels: [],
          harnesses: [],
          clashes: [],
          skills: [],
        };
      return null;
    });
    renderStudio(AT_THE_ROOT, "drafts");
    expect(await screen.findByRole("heading", { name: "Drafts" })).toBeInTheDocument();
    expect(screen.queryByTestId("skill-drafts-none")).not.toBeInTheDocument();
  });
});

describe("the chat's tab", () => {
  it("says only that its draft is ready in the Skill studio", async () => {
    core([draft("triage"), draft("other", { chat: 9 })]);
    render(<DraftReadyNotice plane="p" session={4} />);
    expect(await screen.findByText(draftReadySays(["triage"], false))).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Open Skill studio" })).toBeInTheDocument();
  });

  it("opens the studio at its draft's own page", async () => {
    core([draft("triage"), draft("other", { chat: 9 })]);
    const heard: SkillStudioAsk[] = [];
    const hear = (event: Event) => heard.push((event as CustomEvent<SkillStudioAsk>).detail);
    window.addEventListener(SKILL_STUDIO_ASKED, hear);
    try {
      const user = userEvent.setup();
      render(<DraftReadyNotice plane="p" session={4} />);
      await user.click(await screen.findByRole("button", { name: "Open Skill studio" }));
    } finally {
      window.removeEventListener(SKILL_STUDIO_ASKED, hear);
    }
    expect(heard).toEqual([{ plane: "p", scope: AT_THE_ROOT, page: "drafts", item: "triage-id" }]);
  });

  it("says nothing for a chat that drafted nothing", async () => {
    const asked = core([draft("triage")]);
    render(<DraftReadyNotice plane="p" session={5} />);
    await waitFor(() => expect(asked.some((one) => one.cmd === "skill_drafts")).toBe(true));
    expect(screen.queryByText(/drafted/)).not.toBeInTheDocument();
  });
});
