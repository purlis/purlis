// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { SkillDetailsPanel, diffLineKind, lifecycleSaid } from "./SkillDetails";

afterEach(() => {
  cleanup();
  clearMocks();
});

const NEW = "a".repeat(40);
const OLD = "b".repeat(40);

function totals(group: unknown, runs: number) {
  return {
    group,
    runs,
    ended: runs,
    tool_calls: runs * 2,
    tool_errors: 0,
    refusals: 0,
    asks: 0,
    tokens: null,
    ms: 100,
  };
}

function version(commit: string, subject: string, runs: number) {
  return {
    commit,
    at: "2026-10-20T10:00:00+00:00",
    author: "Ana",
    subject,
    runs: {
      runs,
      ended: runs,
      tool_calls: runs * 2,
      tool_errors: 0,
      refusals: 0,
      asks: 0,
      ms: 100,
    },
    grade: null,
  };
}

function page(lifecycle: unknown, unused: number | null) {
  return {
    copies: [
      {
        level: { at: "project", local: false },
        label: "project",
        lifecycle,
        unread: false,
        owners: ["alice"],
        unused_days: unused,
        versions: [version(NEW, "skill edit deploy", 2), version(OLD, "skill new deploy", 0)],
        uncommitted: totals({ kind: "none" }, 0),
      },
    ],
    uses: [
      { said: "skills:ship", name: "ship", project: true, broken: false },
      { said: "skills:gone", name: "gone", project: true, broken: true },
    ],
    used_by: ["release"],
    uses_cut: false,
  };
}

describe("a skill's history, state and uses", () => {
  it("lists each version with its runs, shows its changes and reverts to an older one", async () => {
    const asked: Array<{ cmd: string; args: unknown }> = [];
    mockIPC((cmd, args) => {
      asked.push({ cmd, args });
      if (cmd === "skill_details") return page({ state: "active" }, null);
      if (cmd === "skill_diff")
        return {
          text: "--- a\n+++ b\n-Step one.\n+Step two.\n",
          cut: false,
          unseen: [
            "line 3 holds a character that draws as nothing (U+202E), shown by its code point",
          ],
        };
      if (cmd === "skill_revert_widens")
        return "Widens the chat: allowed-tools apply while it runs. purlis leaves them out when it hands this skill to a chat.";
      if (cmd === "skill_revert") return { notes: [] };
      throw new Error(`unexpected ${cmd}`);
    });
    render(<SkillDetailsPanel plane="p" name="deploy" changed={0} />);

    const newest = await screen.findByTestId(`skill-version-${NEW}`);
    expect(newest.textContent).toContain("2 runs, 4 tool calls, 0 errors");
    expect(newest.textContent).not.toContain("Revert to this");
    const older = screen.getByTestId(`skill-version-${OLD}`);
    expect(older.textContent).toContain("No runs");

    fireEvent.click(screen.getAllByText("Show changes")[0]);
    const diff = await screen.findByTestId("skill-diff");
    expect(diff.querySelector('[data-line="added"]')?.textContent).toContain("+Step two.");
    expect(diff.querySelector('[data-line="removed"]')?.textContent).toContain("-Step one.");
    expect(screen.getByText(/U\+202E/)).toBeTruthy();

    fireEvent.click(screen.getByText("Revert to this"));
    const widens = await screen.findByTestId("skill-revert-widens");
    expect(widens.textContent).toContain("Widens the chat");
    expect(asked.some((one) => one.cmd === "skill_revert")).toBe(false);
    fireEvent.click(screen.getByText("Revert"));
    await waitFor(() => expect(asked.some((one) => one.cmd === "skill_revert")).toBe(true));
    const revert = asked.find((one) => one.cmd === "skill_revert")?.args as { commit: string };
    expect(revert.commit).toBe(OLD);
  });

  it("names the uses, marks a broken one, and lists what uses it", async () => {
    mockIPC((cmd) => {
      if (cmd === "skill_details") return page({ state: "active" }, null);
      throw new Error(`unexpected ${cmd}`);
    });
    render(<SkillDetailsPanel plane="p" name="deploy" changed={0} />);

    const uses = await screen.findByTestId("skill-uses");
    expect(uses.textContent).toContain("Uses ship");
    expect(uses.querySelector("[data-broken]")?.textContent).toContain("gone");
    expect(uses.textContent).toContain("Used by release");
  });

  it("suggests deprecating a skill no chat ran for 60 days, and deprecates it for a replacement", async () => {
    const asked: Array<{ cmd: string; args: unknown }> = [];
    mockIPC((cmd, args) => {
      asked.push({ cmd, args });
      if (cmd === "skill_details") return page({ state: "active" }, 61);
      if (cmd === "skill_lifecycle") return { notes: [] };
      throw new Error(`unexpected ${cmd}`);
    });
    render(<SkillDetailsPanel plane="p" name="deploy" changed={0} />);

    expect((await screen.findByTestId("skill-unused")).textContent).toContain("61 days");
    fireEvent.change(screen.getByLabelText("The skill to use instead"), {
      target: { value: "ship" },
    });
    fireEvent.click(screen.getByText("Deprecate"));
    await waitFor(() => expect(asked.some((one) => one.cmd === "skill_lifecycle")).toBe(true));
    const set = asked.find((one) => one.cmd === "skill_lifecycle")?.args as { lifecycle: unknown };
    expect(set.lifecycle).toEqual({ state: "deprecated", replacement: "ship" });
  });

  it("offers Restore for an archived skill and says it never loads", async () => {
    mockIPC((cmd) => {
      if (cmd === "skill_details") return page({ state: "archived" }, 90);
      throw new Error(`unexpected ${cmd}`);
    });
    render(<SkillDetailsPanel plane="p" name="deploy" changed={0} />);

    expect((await screen.findByTestId("skill-lifecycle")).textContent).toContain("never loads");
    expect(screen.getByText("Restore")).toBeTruthy();
    expect(screen.queryByTestId("skill-unused")).toBeNull();
  });
});

describe("diffLineKind", () => {
  it("marks added and removed lines, and neither a file's header nor context", () => {
    expect(diffLineKind("+Step two.")).toBe("added");
    expect(diffLineKind("-Step one.")).toBe("removed");
    expect(diffLineKind("+++ b/SKILL.md")).toBeUndefined();
    expect(diffLineKind("--- a/SKILL.md")).toBeUndefined();
    expect(diffLineKind(" context")).toBeUndefined();
  });
});

describe("lifecycleSaid", () => {
  it("names the replacement of a deprecated skill", () => {
    expect(lifecycleSaid("deploy", { state: "deprecated", replacement: "ship" })).toContain(
      "ship instead",
    );
    expect(lifecycleSaid("deploy", { state: "active" })).toBe("In use.");
  });
});
