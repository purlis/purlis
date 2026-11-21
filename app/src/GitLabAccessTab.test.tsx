// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { afterEach, describe, expect, it } from "vitest";
import { SETTINGS_LINK, type SettingsLinkAsk } from "./settings/links";
import type { GitlabShown } from "./bindings";
import { GitLabAccessTab } from "./GitLabAccessTab";

const NONE: GitlabShown = {
  rows: [{ host: "git.acme.internal", personas: [], by_hand: [] }],
  personas: ["steward", "reviewer"],
};
const SET_UP: GitlabShown = {
  rows: [{ host: "git.acme.internal", personas: ["steward"], by_hand: [] }],
  personas: ["steward", "reviewer"],
};
const LINES = '[sandbox.personas.steward.tools]\nglab = { vault = "steward" }';

function fake(answers: Record<string, unknown>) {
  const asked: { cmd: string; args: Record<string, unknown> }[] = [];
  mockIPC((cmd, args) => {
    asked.push({ cmd, args: args as Record<string, unknown> });
    if (cmd in answers) {
      const said = answers[cmd];
      if (said instanceof Error) throw said;
      return said;
    }
    throw new Error(`nothing fakes ${cmd}`);
  });
  return asked;
}

afterEach(() => {
  cleanup();
  clearMocks();
});

describe("GitLab access", () => {
  it("says what token to make and that the chat never sees it", async () => {
    fake({ gitlab_access_list: NONE });
    render(<GitLabAccessTab plane="p" />);
    expect(await screen.findByRole("heading", { name: "git.acme.internal" })).toBeTruthy();
    expect(screen.getByText(/Developer role with the api scope/)).toBeTruthy();
    expect(screen.getByText(/chat never sees the token/)).toBeTruthy();
  });

  it("sets up with the lines it showed and the token, then says the Allow", async () => {
    const asked = fake({
      gitlab_access_list: NONE,
      gitlab_access_lines: LINES,
      gitlab_access_set: { to_allow: ["steward"] },
    });
    render(<GitLabAccessTab plane="p" />);
    await userEvent.click(await screen.findByRole("checkbox", { name: "steward" }));
    expect(await screen.findByText(/glab = \{ vault = "steward" \}/)).toBeTruthy();
    await userEvent.type(
      screen.getByLabelText("GitLab token for git.acme.internal"),
      "fabricated-not-a-gitlab-token",
    );
    await userEvent.click(screen.getByRole("button", { name: "Set up GitLab access" }));
    expect(await screen.findByText(/Give your Allow for steward/)).toBeTruthy();
    const set = asked.find((one) => one.cmd === "gitlab_access_set");
    expect(set?.args).toMatchObject({
      host: "git.acme.internal",
      personas: ["steward"],
      token: "fabricated-not-a-gitlab-token",
      shown: LINES,
    });
    expect(
      (screen.getByLabelText("GitLab token for git.acme.internal") as HTMLInputElement).value,
    ).toBe("");
  });

  it("removes only after asking", async () => {
    const asked = fake({ gitlab_access_list: SET_UP, gitlab_access_remove: null });
    render(<GitLabAccessTab plane="p" />);
    await userEvent.click(await screen.findByRole("button", { name: "Remove" }));
    expect(asked.some((one) => one.cmd === "gitlab_access_remove")).toBe(false);
    await userEvent.click(
      await screen.findByRole("button", { name: "Remove", description: undefined }),
    );
  });

  it("says what happened and what to do when setting up is refused", async () => {
    fake({
      gitlab_access_list: NONE,
      gitlab_access_lines: LINES,
      gitlab_access_set: new Error("That isn't a GitLab token. Paste the token alone."),
    });
    render(<GitLabAccessTab plane="p" />);
    await userEvent.click(await screen.findByRole("checkbox", { name: "steward" }));
    await screen.findByText(/glab = /);
    await userEvent.type(screen.getByLabelText("GitLab token for git.acme.internal"), "x");
    await userEvent.click(screen.getByRole("button", { name: "Set up GitLab access" }));
    expect(await screen.findByText(/Paste the token alone/)).toBeTruthy();
  });
});

describe("The one sign-in", () => {
  it("points to Accounts, and says what is set up here keeps working until then", async () => {
    fake({ gitlab_access_list: NONE });
    const heard: SettingsLinkAsk[] = [];
    const hear = (event: Event) => heard.push((event as CustomEvent<SettingsLinkAsk>).detail);
    window.addEventListener(SETTINGS_LINK, hear);
    render(<GitLabAccessTab plane="p" />);
    expect(await screen.findByText(/keeps working until then/)).toBeTruthy();
    await userEvent.click(screen.getByRole("button", { name: "Open accounts" }));
    window.removeEventListener(SETTINGS_LINK, hear);
    expect(heard).toEqual([{ plane: "p", link: { group: "you.accounts" } }]);
  });
});
