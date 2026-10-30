// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import SkillEditor from "./SkillEditor";
import type { SkillLintFinding } from "./bindings";

afterEach(cleanup);

const WHOLE = "---\nname: deploy\ndescription: Ships it.\n---\n\nbody\n";

/** The editor on deploy, typed into (`text`), with what the lint found. */
function editor(lint: SkillLintFinding[], text = `${WHOLE}more\n`) {
  return render(
    <SkillEditor
      name="deploy"
      was={WHOLE}
      text={text}
      lint={lint}
      checking={false}
      saving={false}
      onChange={() => undefined}
      onSave={() => undefined}
      onDiscard={() => undefined}
    />,
  );
}

describe("a skill in the light editor", () => {
  it("offers Save only for a whole skill that changed", () => {
    editor([
      {
        level: "blocks",
        says: "its frontmatter names no description:, so no chat would know when to use it",
      },
    ]);
    expect(screen.getByText(/names no description/)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Save" })).toBeDisabled();
    cleanup();

    editor([], WHOLE);
    expect(screen.getByText("It is a whole skill.")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Save" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "Discard" })).toBeDisabled();
  });

  it("shows a warning and still offers Save (SK-8)", () => {
    const { container } = editor([
      { level: "warns", says: "its ! command calls jq, which this machine does not have" },
    ]);
    expect(
      screen.getByText("Worth knowing: its ! command calls jq, which this machine does not have"),
    ).toBeInTheDocument();
    expect(container.querySelector('[data-level="warns"]')).not.toBeNull();
    expect(screen.getByRole("button", { name: "Save" })).toBeEnabled();
  });
});
