// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **Settings › Skill sources** (#6031): the project's `[skill_policy]` read only, each key saying
 * it is set in the shared file, and the skill acts this machine recorded, the core faked at its
 * command boundary.
 */
import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { LiveRow, ShownWhile } from "./SettingsTab";
import { projectSkillActsGroup, projectSkillSourcesGroup } from "./skillsSummary";
import type { LiveSetting } from "./groups";

afterEach(() => {
  cleanup();
  clearMocks();
});

const SET = {
  file: "purlis.toml",
  state: {
    kind: "set",
    allow: ["https://github.com/acme/skills", "pdf from https://github.com/anthropics/skills"],
    block: ["yolo from https://github.com/acme/skills"],
    min_age_days: 7,
    exempt: ["9f3c1d27a0b4e5f6a7b8c9d0e1f2a3b4c5d6e7f8"],
    said: "This project's controls apply: 2 sources allowed, 1 skill blocked, commits at least 7 days old.",
  },
  sources: [],
};

function Rows({ settings }: { settings: readonly LiveSetting[] }) {
  return (
    <>
      {settings.map((one) => (
        <ShownWhile key={one.id} useShown={one.useShown}>
          <LiveRow setting={one} />
        </ShownWhile>
      ))}
    </>
  );
}

function answering(answers: Record<string, unknown>) {
  mockIPC((cmd) => answers[cmd] ?? null);
}

describe("Settings › Skill sources", () => {
  it("shows each key of [skill_policy] read only, saying it is set in purlis.toml", async () => {
    answering({ skill_policy: SET });
    const group = projectSkillSourcesGroup("p", "purlis.toml");
    expect(group.kept).toBe(
      "Kept in purlis.toml, which your team sees. Read only here: change it by a reviewed change to that file.",
    );
    render(<Rows settings={group.settings as LiveSetting[]} />);

    // Each row reads the controls as it is drawn: wait for what its control says.
    await waitFor(() =>
      expect(screen.getByTestId("skill-policy-allow")).toHaveTextContent(
        "https://github.com/acme/skills · pdf from https://github.com/anthropics/skills",
      ),
    );
    expect(screen.getByTestId("skill-policy-block")).toHaveTextContent(
      "yolo from https://github.com/acme/skills",
    );
    expect(screen.getByTestId("skill-policy-min-age")).toHaveTextContent("7 days");
    expect(screen.getByTestId("skill-policy-exempt")).toHaveTextContent("9f3c1d27a0b4");
    expect(screen.getAllByText("Team")).toHaveLength(4);
    expect(screen.getByTestId("skill-policy-allow")).toHaveAccessibleDescription(
      expect.stringContaining("Set in purlis.toml, which your team sees.") as string,
    );
    // Nothing on the page writes it.
    expect(screen.queryByRole("button")).toBeNull();
    expect(screen.queryByTestId("skill-policy-controls")).toBeNull();
  });

  it("says where a project has no controls, and why a table cannot be read", async () => {
    answering({ skill_policy: { file: "purlis.toml", state: { kind: "none" }, sources: [] } });
    const { unmount } = render(
      <Rows settings={projectSkillSourcesGroup("p", "purlis.toml").settings as LiveSetting[]} />,
    );
    expect(await screen.findByTestId("skill-policy-controls")).toHaveTextContent(
      "None: purlis.toml holds no [skill_policy], so a skill installs from any source and no install, update or removal is audited.",
    );
    expect(screen.queryByTestId("skill-policy-allow")).toBeNull();
    unmount();

    answering({
      skill_policy: {
        file: "purlis.toml",
        state: {
          kind: "refused",
          why: "this project's [skill_policy] in purlis.toml could not be read (an entry names neither one source nor one extension, or an empty skill), so nothing is installed or updated until it is fixed",
        },
        sources: [],
      },
    });
    render(
      <Rows settings={projectSkillSourcesGroup("p", "purlis.toml").settings as LiveSetting[]} />,
    );
    expect(await screen.findByTestId("skill-policy-controls")).toHaveTextContent(
      /^This project's \[skill_policy\] in purlis\.toml could not be read/,
    );
  });
});

describe("Settings › Recorded skill acts", () => {
  it("lists each install, update and removal newest first, with where it was made", async () => {
    answering({
      skill_acts: {
        acts: [
          {
            at: Date.parse("2026-11-13T14:02:00Z") / 1000,
            act: "install",
            skills: ["release-train", "notify"],
            source: "https://github.com/acme/skills",
            pin: "4be9c21aa",
            level: "project",
            by: "window",
          },
          {
            at: Date.parse("2026-11-02T09:03:00Z") / 1000,
            act: "remove",
            skills: ["lint-fix"],
            source: "https://github.com/acme/skills",
            pin: "1aa20f9bb",
            level: "persona reviewer",
            by: "terminal",
          },
        ],
        more: false,
      },
    });
    render(<Rows settings={projectSkillActsGroup("p").settings as LiveSetting[]} />);
    const table = await screen.findByRole("grid", { name: "Recorded skill acts" });
    const rows = within(table).getAllByRole("row").slice(1);
    expect(rows).toHaveLength(2);
    expect(rows[0]).toHaveTextContent("Install");
    expect(rows[0]).toHaveTextContent("release-train, notify");
    expect(rows[0]).toHaveTextContent("https://github.com/acme/skills @ 4be9c21");
    expect(rows[0]).toHaveTextContent("You, in the Skill studio");
    expect(rows[1]).toHaveTextContent("Remove");
    expect(rows[1]).toHaveTextContent("You, at a terminal");
  });

  it("says so where nothing is recorded yet", async () => {
    answering({ skill_acts: { acts: [], more: false } });
    render(<Rows settings={projectSkillActsGroup("p").settings as LiveSetting[]} />);
    expect(await screen.findByTestId("skill-acts-none")).toHaveTextContent(
      "No skill install, update or removal is recorded on this machine yet.",
    );
  });
});
