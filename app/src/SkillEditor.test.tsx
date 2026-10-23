// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import SkillEditor from "./SkillEditor";
import type { SkillLintFinding } from "./bindings";

afterEach(() => {
  cleanup();
  clearMocks();
});

const WHOLE = "---\nname: deploy\ndescription: Ships it.\n---\n\nbody\n";

describe("a skill in the light editor", () => {
  it("shows the core's live lint and offers Save only for a whole skill that changed", async () => {
    let lint: SkillLintFinding[] = [
      {
        level: "blocks",
        says: "its frontmatter names no description:, so no chat would know when to use it",
      },
    ];
    mockIPC((cmd) => {
      if (cmd === "skill_lint") return lint;
      throw new Error(`unexpected ${cmd}`);
    });
    const onSave = vi.fn();
    const { rerender } = render(
      <SkillEditor
        plane="p"
        level={{ workspace: null, local: false }}
        name="deploy"
        was={WHOLE}
        saving={false}
        onSave={onSave}
        onClose={() => undefined}
      />,
    );

    expect(await screen.findByText(/names no description/)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Save" })).toBeDisabled();

    lint = [];
    rerender(
      <SkillEditor
        plane="p"
        level={{ workspace: null, local: false }}
        name="deploy"
        was={WHOLE}
        saving={true}
        onSave={onSave}
        onClose={() => undefined}
      />,
    );
    expect(screen.getByTestId("text-editor")).toBeInTheDocument();
  });

  it("shows a warning and still offers Save (SK-8)", async () => {
    mockIPC((cmd) => {
      if (cmd === "skill_lint")
        return [
          { level: "warns", says: "its ! command calls jq, which this machine does not have" },
        ];
      throw new Error(`unexpected ${cmd}`);
    });
    const { container } = render(
      <SkillEditor
        plane="p"
        level={{ workspace: null, local: false }}
        name="deploy"
        was={WHOLE}
        saving={false}
        onSave={() => undefined}
        onClose={() => undefined}
      />,
    );
    expect(
      await screen.findByText(
        "Worth knowing: its ! command calls jq, which this machine does not have",
      ),
    ).toBeInTheDocument();
    expect(container.querySelector('[data-level="warns"]')).not.toBeNull();
  });
});
