// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { PersonaSkillAdd, PersonaSkillRemove } from "./personaSkills";

afterEach(() => {
  cleanup();
  clearMocks();
});

describe("a persona's skills in the Skill studio", () => {
  it("adds a skill by name and asks the studio again", async () => {
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
      <PersonaSkillAdd plane="p" persona="ops" known={["purlis:handoff"]} onChanged={changed} />,
    );
    fireEvent.change(screen.getByTestId("persona-skill-name"), { target: { value: " deploy " } });
    fireEvent.click(screen.getByRole("button", { name: "Add" }));

    await waitFor(() => expect(changed).toHaveBeenCalledTimes(1));
    expect(asked).toEqual([{ plane: "p", persona: "ops", written: "deploy", change: "add" }]);
  });

  it("offers each skill as the studio writes it", () => {
    mockIPC(() => true);
    const { container } = render(
      <PersonaSkillAdd
        plane="p"
        persona="ops"
        known={["purlis:handoff", "deploy", "deploy"]}
        onChanged={() => {}}
      />,
    );
    const offered = [...container.querySelectorAll("datalist option")].map((one) =>
      one.getAttribute("value"),
    );
    expect(offered).toEqual(["purlis:handoff", "deploy"]);
  });

  it("says the core's refusal and keeps what was typed", async () => {
    mockIPC((cmd) => {
      if (cmd === "persona_skill") throw "“:x” names no skill";
      throw new Error(`unexpected ${cmd}`);
    });

    render(<PersonaSkillAdd plane="p" persona="ops" known={[]} onChanged={() => {}} />);
    fireEvent.change(screen.getByTestId("persona-skill-name"), { target: { value: ":x" } });
    fireEvent.click(screen.getByRole("button", { name: "Add" }));

    expect(await screen.findByText(/names no skill/)).toBeInTheDocument();
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
    expect(asked).toEqual([
      { plane: "p", persona: "ops", written: "purlis:handoff", change: "remove" },
    ]);
  });

  it("is off on a read-only project", () => {
    mockIPC(() => true);
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
