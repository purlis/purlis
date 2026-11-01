// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { StrictMode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, render as renderBare, screen } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { Channel } from "@tauri-apps/api/core";
import type { CreateProgress, ForgeOwners, TemplateRow } from "./bindings";
import { CreateOnForge } from "./CreateOnForge";
import { REMEMBERED_PARENT } from "./GetProject";
import { offeredIn, pickIn, selectNamed } from "./ui/select/select.testkit";

/**
 * **Create a project on a forge, as the page draws it** (#1819): the owner picker from the
 * person's sign-in, the form's answers reaching the core, each stage as it runs with Cancel, a
 * refusal with Retry, and "sign in first" with no account. The creation itself is the core's,
 * tested against the forge contract's recordings in `purlis-core`'s `createproject`; here the
 * core is a mock that answers as it does.
 */

const render = (ui: React.ReactElement) => renderBare(<StrictMode>{ui}</StrictMode>);

const FOLDER = "/Users/someone/Projects/garden";
const STAGES = [
  "Checking the owner on GitHub",
  "Laying out the project",
  "Making the first commit",
  "Creating the repo on GitHub",
  "Pushing the first commit",
];
const OWNERS: ForgeOwners = {
  host: "github.com",
  login: "octocat",
  owners: [
    { name: "octocat", title: "The Octocat", mine: true },
    { name: "acme", title: "acme", mine: false },
  ],
};
const TEMPLATES: TemplateRow[] = [{ id: "rust", title: "Rust", summary: "A Cargo workspace." }];

afterEach(() => {
  cleanup();
  clearMocks();
});

beforeEach(() => {
  globalThis.localStorage.removeItem(REMEMBERED_PARENT);
});

function core(owners: ForgeOwners = OWNERS) {
  const asked: { cmd: string; args: Record<string, unknown> }[] = [];
  let finish: (answer: string | null) => void = () => {};
  let fail: (sentence: string) => void = () => {};
  let progress: Channel<CreateProgress> | undefined;
  mockIPC((cmd, args) => {
    const given = (args ?? {}) as Record<string, unknown>;
    asked.push({ cmd, args: given });
    if (cmd === "forge_owners") return owners;
    if (cmd === "new_project_folder") {
      if (given.name === "not a name") return Promise.reject("A project's name is letters.");
      const parent = (given.parent as string | null) ?? "/Users/someone/Projects";
      return `${parent}/${String(given.name)}`;
    }
    if (cmd === "create_project_on_forge") {
      progress = given.progress as Channel<CreateProgress>;
      return new Promise((resolve, reject) => {
        finish = resolve;
        fail = (sentence) => reject(sentence);
      });
    }
    return null;
  });
  return {
    asked,
    finish: (answer: string | null) => finish(answer),
    fail: (sentence: string) => fail(sentence),
    tell: (at: number) => progress?.onmessage({ stage: STAGES[at], at, stages: STAGES }),
  };
}

function page() {
  const onOpen = vi.fn();
  const onNeedsSignIn = vi.fn();
  render(
    <CreateOnForge
      forge="github"
      templates={TEMPLATES}
      onOpen={onOpen}
      onNeedsSignIn={onNeedsSignIn}
    />,
  );
  return { onOpen, onNeedsSignIn, person: userEvent.setup() };
}

describe("create a project on a forge (#1819)", () => {
  it("offers the account and its organisations, the account first", async () => {
    core();
    page();
    const owner = await screen.findByLabelText("Owner");
    expect(owner).toHaveValue("octocat");
    expect(owner).toHaveTextContent("The Octocat (you)");
    expect(await offeredIn(owner)).toEqual(["The Octocat (you)", "acme"]);
  });

  it("sends the owner, the name, the visibility, the template and the folder, private by default", async () => {
    const creation = core();
    const { person, onOpen } = page();
    await pickIn(await screen.findByLabelText("Owner"), "acme", person);
    await person.type(screen.getByLabelText("Name"), "garden");
    expect(await screen.findByText(FOLDER)).toBeInTheDocument();
    expect(screen.getByRole("radio", { name: /Private/ })).toBeChecked();
    await pickIn(selectNamed("Project template"), "Rust");

    await person.click(screen.getByRole("button", { name: "Create project" }));
    await vi.waitFor(() =>
      expect(creation.asked.some((one) => one.cmd === "create_project_on_forge")).toBe(true),
    );
    const asked = creation.asked.find((one) => one.cmd === "create_project_on_forge");
    expect(asked?.args.ask).toEqual({
      forge: "github",
      owner: "acme",
      name: "garden",
      private: true,
      template: "rust",
      folder: FOLDER,
    });

    creation.finish(FOLDER);
    await vi.waitFor(() => expect(onOpen).toHaveBeenCalledWith(FOLDER));
  });

  it("sends public when the person picks it, and no template for Empty", async () => {
    const creation = core();
    const { person } = page();
    await screen.findByLabelText("Owner");
    await person.type(screen.getByLabelText("Name"), "garden");
    await screen.findByText(FOLDER);
    await person.click(screen.getByRole("radio", { name: /Public/ }));
    await person.click(screen.getByRole("button", { name: "Create project" }));
    await vi.waitFor(() =>
      expect(creation.asked.some((one) => one.cmd === "create_project_on_forge")).toBe(true),
    );
    const ask = creation.asked.find((one) => one.cmd === "create_project_on_forge")?.args.ask;
    expect(ask).toMatchObject({ owner: "octocat", private: false, template: null });
  });

  it("lists every stage as it runs, and Cancel calls the creation off", async () => {
    const creation = core();
    const { person, onOpen } = page();
    await screen.findByLabelText("Owner");
    await person.type(screen.getByLabelText("Name"), "garden");
    await screen.findByText(FOLDER);
    await person.click(screen.getByRole("button", { name: "Create project" }));
    creation.tell(3);

    const list = await screen.findByRole("list", { name: "Creating garden" });
    expect(list.querySelectorAll("li")).toHaveLength(5);
    expect(screen.getByText("Creating the repo on GitHub…")).toHaveAttribute(
      "aria-current",
      "step",
    );
    expect(screen.getByText("Making the first commit")).toHaveClass("done");
    expect(screen.getByRole("button", { name: "Create project" })).toBeDisabled();

    await person.click(screen.getByRole("button", { name: "Cancel" }));
    expect(creation.asked.some((one) => one.cmd === "cancel_create_project")).toBe(true);
    creation.finish(null);
    expect(await screen.findByText("Cancelled. Nothing was left behind.")).toBeInTheDocument();
    expect(onOpen).not.toHaveBeenCalled();
  });

  it("says why a creation failed and what was taken back, with Retry", async () => {
    const creation = core();
    const { person } = page();
    await screen.findByLabelText("Owner");
    await person.type(screen.getByLabelText("Name"), "garden");
    await screen.findByText(FOLDER);
    await person.click(screen.getByRole("button", { name: "Create project" }));
    const sentence =
      "GitHub already has a repo named octocat/garden. Pick another name, then Retry. Nothing was left behind.";
    creation.fail(sentence);

    expect(await screen.findByRole("alert")).toHaveTextContent(sentence);
    await person.click(screen.getByRole("button", { name: "Retry" }));
    await vi.waitFor(() =>
      expect(creation.asked.filter((one) => one.cmd === "create_project_on_forge")).toHaveLength(2),
    );
  });

  it("asks for a sign-in first when nobody is signed in to that forge", async () => {
    core({ host: null, login: "", owners: [] });
    const { person, onNeedsSignIn } = page();
    await person.click(await screen.findByRole("button", { name: "Sign in to GitHub" }));
    expect(onNeedsSignIn).toHaveBeenCalled();
    expect(screen.queryByRole("button", { name: "Create project" })).not.toBeInTheDocument();
  });

  it("says why a name cannot be one, and does not create", async () => {
    core();
    const { person } = page();
    await screen.findByLabelText("Owner");
    await person.type(screen.getByLabelText("Name"), "not a name");
    expect(await screen.findByRole("alert")).toHaveTextContent("A project's name is letters.");
    expect(screen.getByRole("button", { name: "Create project" })).toBeDisabled();
  });

  it("calls the creation off when the page is left mid-way", async () => {
    const creation = core();
    const { person } = page();
    await screen.findByLabelText("Owner");
    await person.type(screen.getByLabelText("Name"), "garden");
    await screen.findByText(FOLDER);
    await person.click(screen.getByRole("button", { name: "Create project" }));
    await vi.waitFor(() =>
      expect(creation.asked.some((one) => one.cmd === "create_project_on_forge")).toBe(true),
    );
    cleanup();
    expect(creation.asked.some((one) => one.cmd === "cancel_create_project")).toBe(true);
  });
});
