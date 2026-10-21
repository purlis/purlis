// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { PersonaSkillAdd, PersonaSkillRemove, isPersonaSkills, namedOf } from "./personaSkills";
import type { PanelBlock, PanelRow } from "./bindings";

afterEach(() => {
  cleanup();
  clearMocks();
});

const row = (key: string): PanelRow =>
  ({
    key,
    text: key,
    note: null,
    mark: "dot",
    tone: "plain",
    detail: null,
    runs: "skills.studio:persona/ops",
    actions: [],
  }) as unknown as PanelRow;

describe("a persona's Skills section", () => {
  it("is the list whose empty state offers the persona's studio", () => {
    const list = (offer: string | null) =>
      ({
        kind: "list",
        rows: [],
        empty: { headline: "", body: null, offer },
      }) as unknown as PanelBlock;
    expect(isPersonaSkills(list("skills.studio:persona/ops"), "ops")).toBe(true);
    expect(isPersonaSkills(list("skills.studio:persona/ops"), "qa")).toBe(false);
    expect(isPersonaSkills(list("memory.new:persona/ops"), "ops")).toBe(false);
  });

  it("removes only what its skills line names", () => {
    expect(namedOf(row("named:superpowers:tdd"))).toBe("superpowers:tdd");
    expect(namedOf(row("own:triage"))).toBeUndefined();
  });

  it("adds a skill by name and asks the view again", async () => {
    const asked: unknown[] = [];
    mockIPC((cmd, args) => {
      if (cmd === "skill_studio")
        return { on: true, workspace: null, persona: "ops", workspaces: [], skills: [] };
      if (cmd === "persona_skill") {
        asked.push(args);
        return true;
      }
      throw new Error(`unexpected ${cmd}`);
    });
    const changed = vi.fn();

    render(<PersonaSkillAdd plane="p" persona="ops" changed={0} onChanged={changed} />);
    fireEvent.change(screen.getByTestId("persona-skill-name"), { target: { value: " deploy " } });
    fireEvent.click(screen.getByRole("button", { name: "Add" }));

    await waitFor(() => expect(changed).toHaveBeenCalledTimes(1));
    expect(asked).toEqual([{ plane: "p", persona: "ops", written: "deploy", add: true }]);
  });

  it("says the core's refusal and keeps what was typed", async () => {
    mockIPC((cmd) => {
      if (cmd === "skill_studio")
        return { on: true, workspace: null, persona: "ops", workspaces: [], skills: [] };
      if (cmd === "persona_skill") throw '":x" names no skill';
      throw new Error(`unexpected ${cmd}`);
    });

    render(<PersonaSkillAdd plane="p" persona="ops" changed={0} onChanged={() => {}} />);
    fireEvent.change(screen.getByTestId("persona-skill-name"), { target: { value: ":x" } });
    fireEvent.click(screen.getByRole("button", { name: "Add" }));

    expect(await screen.findByRole("alert")).toHaveTextContent("names no skill");
    expect(screen.getByTestId("persona-skill-name")).toHaveValue(":x");
  });

  it("takes a named skill out", async () => {
    const asked: unknown[] = [];
    mockIPC((cmd, args) => {
      if (cmd === "persona_skill") {
        asked.push(args);
        return true;
      }
      throw new Error(`unexpected ${cmd}`);
    });
    const changed = vi.fn();

    render(
      <PersonaSkillRemove plane="p" persona="ops" written="purlis:handoff" onChanged={changed} />,
    );
    fireEvent.click(screen.getByRole("button", { name: "Remove purlis:handoff from ops" }));

    await waitFor(() => expect(changed).toHaveBeenCalledTimes(1));
    expect(asked).toEqual([{ plane: "p", persona: "ops", written: "purlis:handoff", add: false }]);
  });

  it("is off on a read-only project", () => {
    mockIPC(() => ({ on: true, workspace: null, persona: "ops", workspaces: [], skills: [] }));
    render(
      <PersonaSkillRemove
        plane="p"
        persona="ops"
        written="deploy"
        onChanged={() => {}}
        readOnly="This project is read-only."
      />,
    );
    expect(screen.getByRole("button", { name: "Remove deploy from ops" })).toBeDisabled();
  });
});
