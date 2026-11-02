// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { PersonaSkillAdd, changePersonaSkill } from "./personaSkills";

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
      <PersonaSkillAdd
        plane="p"
        persona="ops"
        known={["purlis:purlis-handoff"]}
        onChanged={changed}
      />,
    );
    fireEvent.change(screen.getByTestId("persona-skill-name"), { target: { value: " deploy " } });
    fireEvent.click(screen.getByRole("button", { name: "Add" }));

    await waitFor(() => expect(changed).toHaveBeenCalledTimes(1));
    expect(asked).toEqual([{ plane: "p", persona: "ops", written: "deploy", change: "add" }]);
  });

  it("offers each skill as the studio writes it, once, in the combobox's list", async () => {
    mockIPC(() => true);
    const user = userEvent.setup();
    render(
      <PersonaSkillAdd
        plane="p"
        persona="ops"
        known={["purlis:purlis-handoff", "deploy", "deploy"]}
        onChanged={() => {}}
      />,
    );
    await user.click(screen.getByRole("button", { name: /suggestions/i }));
    const list = await screen.findByRole("listbox");
    const offered = within(list)
      .getAllByRole("option")
      .map((one) => one.textContent);
    expect(offered).toEqual(["purlis:purlis-handoff", "deploy"]);
    await user.click(within(list).getByRole("option", { name: "deploy" }));
    expect(screen.getByRole("combobox", { name: "Add a skill" })).toHaveValue("deploy");
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

  it("takes a named skill out, as a studio row's Remove does", async () => {
    const asked: unknown[] = [];
    mockIPC((cmd, args) => {
      if (cmd === "persona_skill") {
        asked.push(args);
        return true;
      }
      throw new Error(`unexpected ${cmd}`);
    });

    expect(await changePersonaSkill("p", "ops", "purlis:purlis-handoff", "remove")).toBeUndefined();

    expect(asked).toEqual([
      { plane: "p", persona: "ops", written: "purlis:purlis-handoff", change: "remove" },
    ]);
  });
});
