// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { KnownGaps, KnownGapsList, STILL_STARTS } from "./KnownGaps";
import { KnownGapsNotice } from "../KnownGapsNotice";
import { SETTINGS_LINK, type SettingsLinkAsk } from "./links";
import { CLAUDE_READS, HARNESSES } from "./knownGapsFixture";

afterEach(() => {
  cleanup();
  clearMocks();
});

const PLANE = "/home/op/project";
const IDS = { id: "known-gaps", labelledBy: undefined };

describe("known gaps", () => {
  it("a_declared_gap_shows_in_settings_doctor_and_the_pane_head", async () => {
    // Doctor's half is the core's: `a_declared_gap_shows_in_settings_doctor_and_the_pane_head`
    // in purlis-core's tests. Here: Settings and the pane head say the same declared gap.
    mockIPC((cmd) => {
      if (cmd === "known_gaps") return HARNESSES;
      if (cmd === "chat_known_gaps") return [CLAUDE_READS];
      throw new Error(`unexpected ${cmd}`);
    });
    const links: SettingsLinkAsk[] = [];
    const hear = (event: Event) => links.push((event as CustomEvent<SettingsLinkAsk>).detail);
    window.addEventListener(SETTINGS_LINK, hear);

    render(
      <>
        <KnownGaps plane={PLANE} ids={IDS} />
        <KnownGapsNotice plane={PLANE} session={4} />
      </>,
    );
    const list = await screen.findByRole("list", { name: "Claude Code known gaps" });
    expect(list).toHaveTextContent(CLAUDE_READS.said);
    expect(screen.getByText(STILL_STARTS)).toBeTruthy();

    const notice = await waitFor(() => {
      const found = document.querySelector('[data-cause^="known-gaps:4"]');
      expect(found).not.toBeNull();
      return found as HTMLElement;
    });
    expect(notice).toHaveTextContent("This chat started with 1 known gap: claude-code-reads");
    fireEvent.click(within(notice).getByRole("button", { name: "Show known gaps" }));
    window.removeEventListener(SETTINGS_LINK, hear);
    expect(links[0]?.link).toEqual({
      group: "project.sandbox",
      setting: "project.sandbox.knownGaps",
    });
  });

  it("says none are declared where every harness has none", () => {
    render(
      <KnownGapsList
        harnesses={[
          { harness: "claude", gaps: [] },
          { harness: "codex", gaps: [] },
        ]}
        ids={IDS}
      />,
    );
    expect(screen.getByText(/No known gaps are declared/)).toBeTruthy();
    expect(screen.queryByText(STILL_STARTS)).toBeNull();
  });

  it("lists a harness only where it has a gap, and marks one that holds for some chats", () => {
    render(<KnownGapsList harnesses={HARNESSES} ids={IDS} />);
    expect(screen.queryByRole("list", { name: "OpenCode known gaps" })).toBeNull();
    const claude = screen.getByRole("list", { name: "Claude Code known gaps" });
    expect(within(claude).getAllByRole("listitem")).toHaveLength(2);
    expect(claude).toHaveTextContent("Only for some chats of this harness.");
  });
});
