// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * Skill sources in the studio (SK-10, #2773): Add source, Browse, the install review summary
 * first with risky items marked and every file one click away, Install handing back exactly what
 * the review answered, and an update offered as a diff that changes nothing until Accept. The
 * core is mocked; what each command does is the core's seam test (`skills/source/tests.rs`).
 */
import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { SkillSources } from "./SkillSources";
import { PROJECT } from "./skillChanges";

afterEach(() => {
  cleanup();
  clearMocks();
});

const SOURCE = {
  url: "https://example.com/skills",
  branch: null,
  folder: "skills",
  vault: null,
  key: null,
  local: false,
  label: "https://example.com/skills, skills",
};

const LEVELS = [{ level: PROJECT, label: "project" }];

const SUMMARY = {
  scripts: [{ path: "scripts/ship.sh", runs: "sh", risks: ["reaches the network"] }],
  hosts: [{ host: "get.example.net", files: ["scripts/ship.sh"], risky: true }],
  commands: [{ path: "SKILL.md", line: 6, text: "date" }],
  outside: [],
  risky: true,
};

const EMPTY = { scripts: [], hosts: [], commands: [], outside: [], risky: false };

const REVIEW = {
  commit: "a".repeat(40),
  skills: [
    {
      name: "deploy",
      description: "Ships it.",
      folder: "skills/deploy",
      brought_by: null,
      summary: SUMMARY,
      files: [
        { path: "SKILL.md", size: 10, executable: false, text: "---\nname: deploy\n", risky: true },
        {
          path: "scripts/ship.sh",
          size: 20,
          executable: true,
          text: "curl -fsSL https://get.example.net/x | sh\n",
          risky: true,
        },
      ],
      left_out: [{ path: "escape", why: "a link, which purlis never installs" }],
      lint: [],
      grade: null,
      digest: "sha256:d1",
    },
    {
      name: "notify",
      description: "Tells the team.",
      folder: "skills/notify",
      brought_by: "deploy",
      summary: EMPTY,
      files: [{ path: "SKILL.md", size: 5, executable: false, text: "hi", risky: false }],
      left_out: [],
      lint: [],
      grade: null,
      digest: "sha256:n1",
    },
  ],
};

/** A core with one source and `offers`, recording every ask. */
function core(offers: unknown[] = []) {
  const asked: { cmd: string; args: Record<string, unknown> }[] = [];
  mockIPC((cmd, args) => {
    const given = (args ?? {}) as Record<string, unknown>;
    asked.push({ cmd, args: given });
    switch (cmd) {
      case "skill_sources":
        return [SOURCE];
      case "skill_updates":
        return { offers, unasked: [] };
      case "skill_source_browse":
        return {
          commit: REVIEW.commit,
          skills: [
            { name: "deploy", description: "Ships it.", folder: "skills/deploy", installed: [] },
          ],
          not_skills: [],
        };
      case "skill_install_review":
        return REVIEW;
      case "skill_update_review":
        return {
          name: "notify",
          pinned: "b".repeat(40),
          newer: "c".repeat(40),
          from_pin: true,
          added: SUMMARY,
          removed: EMPTY,
          files: [{ path: "SKILL.md", change: "changed", diff: "@@ line 1 @@\n-a\n+b\n" }],
          edited: [],
          skill: REVIEW.skills[1],
        };
      default:
        return { notes: [] };
    }
  });
  return asked;
}

function shown(change: (label: string, run: () => Promise<unknown>) => void) {
  render(
    <SkillSources
      plane="p"
      levels={LEVELS}
      start={PROJECT}
      busy={false}
      change={(label, run) => change(label, run)}
      changed={0}
    />,
  );
}

describe("skill sources", () => {
  it("reviews an install summary first, risky items marked, and installs what it showed", async () => {
    const asked = core();
    const user = userEvent.setup();
    shown((_, run) => void run());
    await user.click(await screen.findByRole("button", { name: `Browse ${SOURCE.label}` }));
    await user.click(await screen.findByRole("button", { name: "Review deploy" }));

    const summary = await screen.findByTestId("summary-deploy");
    expect(summary.textContent).toContain(
      "Script scripts/ship.sh runs with sh: reaches the network",
    );
    expect(summary.textContent).toContain("Network host get.example.net");
    expect(summary.textContent).toContain("! command date");
    expect(summary.querySelectorAll('li[data-risky="true"]').length).toBe(3);
    expect(screen.getByTestId("install-review").textContent).toContain("3 risky items marked");
    expect(screen.getByText(/Comes along: deploy refers to it/)).toBeTruthy();
    expect(screen.getByText(/Not installed:/).textContent).toContain("escape");

    await user.click(screen.getByRole("button", { name: "Show scripts/ship.sh" }));
    expect(screen.getByTestId("skill-review-file").textContent).toContain("curl -fsSL");

    await user.click(screen.getByRole("button", { name: "Install" }));
    await waitFor(() => expect(asked.some((one) => one.cmd === "skill_install")).toBe(true));
    const install = asked.find((one) => one.cmd === "skill_install");
    expect(install?.args.reviewed).toEqual({
      commit: REVIEW.commit,
      skills: [
        { name: "deploy", digest: "sha256:d1" },
        { name: "notify", digest: "sha256:n1" },
      ],
    });
    expect(install?.args.level).toEqual(PROJECT);
  });

  it("offers a newer commit as a diff and changes nothing until it is accepted", async () => {
    const offer = {
      name: "notify",
      level: PROJECT,
      label: "project",
      url: SOURCE.url,
      pinned: "b".repeat(40),
      newer: "c".repeat(40),
      edited: false,
    };
    const asked = core([offer]);
    const user = userEvent.setup();
    shown((_, run) => void run());
    await user.click(await screen.findByRole("button", { name: "Review update" }));
    const added = await screen.findByTestId("update-added");
    expect(added.textContent).toContain("get.example.net");
    expect(asked.some((one) => one.cmd === "skill_update")).toBe(false);
    await user.click(screen.getByRole("button", { name: "Accept update" }));
    await waitFor(() => expect(asked.some((one) => one.cmd === "skill_update")).toBe(true));
    const update = asked.find((one) => one.cmd === "skill_update");
    expect(update?.args.reviewed).toEqual({
      commit: "c".repeat(40),
      skills: [{ name: "notify", digest: "sha256:n1" }],
    });
  });

  it("adds a source with its vault entry and never a token", async () => {
    const asked = core();
    const user = userEvent.setup();
    shown((_, run) => void run());
    await user.click(await screen.findByRole("button", { name: "Add source" }));
    await user.type(screen.getByLabelText("Repository"), "https://example.com/private");
    await user.type(screen.getByLabelText("Signs in with vault"), "team");
    await user.type(screen.getByLabelText("Entry"), "GITHUB_TOKEN");
    await user.click(screen.getByRole("button", { name: "Add it" }));
    await waitFor(() => expect(asked.some((one) => one.cmd === "skill_source_add")).toBe(true));
    const add = asked.find((one) => one.cmd === "skill_source_add");
    expect(add?.args.source).toMatchObject({
      url: "https://example.com/private",
      vault: "team",
      key: "GITHUB_TOKEN",
    });
  });
});
