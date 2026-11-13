// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it } from "vitest";
import { cleanup, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import {
  asPersona,
  asPersonaIn,
  inWorkspace,
  sameScope,
  scopeChat,
  scopeKey,
  scopeOfKey,
  skillStudioTitle,
  startedIn,
  whoseChats,
} from "./skillStudio";
import { skill, studio } from "./skillsListing.testkit";
import { forgetStudio, renderStudio } from "./SkillStudioTab.testkit";

const MENU = "Whose chats and where they start";

/** The project's two personas: steward, dispatched into smart-ide 41 times and billing once; qa,
 *  dispatched nowhere. */
const READ = studio([skill("deploy")], {
  workspace: "smart-ide",
  workspaces: ["smart-ide", "billing", "docs"],
  personas: [
    {
      name: "steward",
      role: "Control Plane Steward",
      dispatched_into: [
        { workspace: "smart-ide", chats: 41 },
        { workspace: "billing", chats: 1 },
      ],
    },
    { name: "qa", role: null, dispatched_into: [] },
  ],
});

/** Every `skill_studio` read the studio asks for. */
function answering(): Record<string, unknown>[] {
  const asked: Record<string, unknown>[] = [];
  mockIPC((cmd, args) => {
    if (cmd === "skills_deleted") return [];
    if (cmd !== "skill_studio") throw new Error(`unexpected ${cmd}`);
    const read = args as Record<string, unknown>;
    asked.push(read);
    return { ...READ, workspace: read.workspace ?? null, persona: read.persona ?? null };
  });
  return asked;
}

afterEach(() => {
  cleanup();
  clearMocks();
  forgetStudio();
});

describe("the Skill studio's picker lists the project's personas (#2935)", () => {
  it("lists a chat as each persona, its role under its name, and picking one reads the studio as it", async () => {
    const asked = answering();
    renderStudio(inWorkspace("smart-ide"));
    const user = userEvent.setup();
    await screen.findByTestId("skill-deploy");

    await user.click(screen.getByRole("button", { name: "A chat in smart-ide" }));
    const menu = await screen.findByRole("menu", { name: MENU });
    const personas = within(menu).getByRole("group", { name: "As a persona" });
    expect(
      within(personas)
        .getAllByRole("menuitemradio")
        .map((one) => one.textContent),
    ).toEqual(["A chat as stewardControl Plane Steward", "A chat as qa"]);
    await user.click(within(personas).getByRole("menuitemradio", { name: /A chat as steward/ }));

    await waitFor(() =>
      expect(asked.at(-1)).toMatchObject({ workspace: null, persona: "steward" }),
    );
    expect(await screen.findByRole("button", { name: "A chat as steward" })).toBeInTheDocument();
  });

  it("offers a persona in a workspace only where it was dispatched, and reads it as a dispatched chat sees it", async () => {
    const asked = answering();
    renderStudio(asPersona("steward"));
    const user = userEvent.setup();
    await screen.findByTestId("skill-deploy");

    await user.click(screen.getByRole("button", { name: "A chat as steward" }));
    const menu = await screen.findByRole("menu", { name: MENU });
    const into = within(menu).getByRole("group", { name: "In which workspace" });
    expect(
      within(into)
        .getAllByRole("menuitemradio")
        .map((one) => one.textContent),
    ).toEqual([
      "At the project's root",
      "Dispatched into smart-ide41 chats ran there",
      "Dispatched into billing1 chat ran there",
    ]);
    // docs is a workspace of the project steward was never dispatched into; and a persona has no
    // Where it starts.
    expect(within(menu).queryByRole("menuitemradio", { name: /into docs/ })).toBeNull();
    expect(within(menu).queryByRole("group", { name: "Where it starts" })).toBeNull();
    expect(
      within(into).getByRole("menuitemradio", { name: "At the project's root" }),
    ).toHaveAttribute("aria-checked", "true");
    await user.click(
      within(into).getByRole("menuitemradio", { name: /Dispatched into smart-ide/ }),
    );

    await waitFor(() =>
      expect(asked.at(-1)).toMatchObject({ workspace: "smart-ide", persona: "steward" }),
    );
    const picker = await screen.findByRole("button", { name: "A chat as steward in smart-ide" });
    await user.click(picker);
    const again = await screen.findByRole("menu", { name: MENU });
    expect(
      within(again).getByRole("menuitemradio", { name: /Dispatched into smart-ide/ }),
    ).toHaveAttribute("aria-checked", "true");
    expect(within(again).getByRole("menuitemradio", { name: /A chat as steward/ })).toHaveAttribute(
      "aria-checked",
      "true",
    );
  });

  it("offers a persona dispatched nowhere only the project's root", async () => {
    answering();
    renderStudio(asPersona("qa"));
    const user = userEvent.setup();
    await screen.findByTestId("skill-deploy");

    await user.click(screen.getByRole("button", { name: "A chat as qa" }));
    const menu = await screen.findByRole("menu", { name: MENU });
    const into = within(menu).getByRole("group", { name: "In which workspace" });
    expect(
      within(into)
        .getAllByRole("menuitemradio")
        .map((one) => one.textContent),
    ).toEqual(["At the project's root"]);
  });
});

describe("a persona in a workspace is a scope of its own (#2935)", () => {
  const there = asPersonaIn("steward", "smart-ide");

  it("asks the core for the persona and the workspace together", () => {
    expect(whoseChats(there)).toEqual({ workspace: "smart-ide", persona: "steward" });
    expect(whoseChats(asPersona("steward"))).toEqual({ workspace: null, persona: "steward" });
  });

  it("is keyed apart from the persona at the root, and read back from its key", () => {
    expect(scopeKey(there)).not.toBe(scopeKey(asPersona("steward")));
    expect(sameScope(scopeOfKey(scopeKey(there)), there)).toBe(true);
    expect(sameScope(scopeOfKey(scopeKey(asPersona("steward"))), asPersona("steward"))).toBe(true);
  });

  it("is named with its workspace, and keeps it whatever clone is asked", () => {
    expect(scopeChat(there)).toBe("A chat as steward in smart-ide");
    expect(skillStudioTitle(there)).toBe("Skill studio · steward in smart-ide");
    expect(sameScope(startedIn(there, "purlis"), there)).toBe(true);
  });
});
