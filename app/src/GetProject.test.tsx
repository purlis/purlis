// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { StrictMode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, render as renderBare, screen } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { Channel } from "@tauri-apps/api/core";
import type { GetProgress } from "./bindings";
import { GetProject, REMEMBERED_PARENT } from "./GetProject";

/**
 * **Get a project by URL, as the page draws it** (#1815): the folder a URL goes into, the
 * clone's progress and its Cancel, each refusal's sentence with Retry, and the place the person
 * picked remembered for the next project. The clone itself is the core's, tested against a
 * local stand-in forge in `purlis-core`'s `getproject`; here the core is a mock that answers as
 * it does.
 */

const render = (ui: React.ReactElement) => renderBare(<StrictMode>{ui}</StrictMode>);

const URL = "https://github.com/acme/widget.git";
const HOME_FOLDER = "/Users/someone/Projects/widget";

afterEach(() => {
  cleanup();
  clearMocks();
});

beforeEach(() => {
  globalThis.localStorage.removeItem(REMEMBERED_PARENT);
});

/** A clone the test answers by hand, and every call the page made. */
function core(answers: Partial<Record<string, (args: Record<string, unknown>) => unknown>> = {}) {
  const asked: { cmd: string; args: Record<string, unknown> }[] = [];
  let finish: (answer: string | null) => void = () => {};
  let fail: (sentence: string) => void = () => {};
  let progress: Channel<GetProgress> | undefined;
  mockIPC((cmd, args) => {
    const given = (args ?? {}) as Record<string, unknown>;
    asked.push({ cmd, args: given });
    const answer = answers[cmd];
    if (answer) return answer(given);
    if (cmd === "project_url_folder") {
      const parent = (given.parent as string | null) ?? "/Users/someone/Projects";
      return { name: "widget", folder: `${parent}/widget` };
    }
    if (cmd === "get_project") {
      progress = given.progress as Channel<GetProgress>;
      return new Promise((resolve, reject) => {
        finish = resolve;
        // A refused command rejects with the core's sentence, as Tauri's IPC does.
        fail = (sentence) => reject(sentence);
      });
    }
    return null;
  });
  return {
    asked,
    finish: (answer: string | null) => finish(answer),
    fail: (sentence: string) => fail(sentence),
    tell: (now: GetProgress) => progress?.onmessage(now),
  };
}

function page(onOpen = vi.fn()) {
  render(<GetProject onBack={vi.fn()} onOpen={onOpen} onOpenFolder={vi.fn()} />);
  return { onOpen, person: userEvent.setup() };
}

describe("get a project by URL (#1815)", () => {
  it("shows the folder a pasted URL goes into: Projects in the home folder", async () => {
    core();
    const { person } = page();

    await person.type(screen.getByLabelText("Project URL"), URL);

    expect(await screen.findByText(HOME_FOLDER)).toBeInTheDocument();
  });

  it("clones into that folder with progress, then opens it through the trust question", async () => {
    const clone = core();
    const { onOpen, person } = page();
    await person.type(screen.getByLabelText("Project URL"), URL);
    await screen.findByText(HOME_FOLDER);

    await person.click(screen.getByRole("button", { name: "Get project" }));
    await vi.waitFor(() => expect(clone.asked.some((one) => one.cmd === "get_project")).toBe(true));
    const asked = clone.asked.find((one) => one.cmd === "get_project");
    expect(asked?.args.url).toBe(URL);
    expect(asked?.args.folder).toBe(HOME_FOLDER);

    clone.tell({ stage: "Downloading", percent: 45 });
    expect(await screen.findByText("Downloading 45%")).toBeInTheDocument();
    expect(screen.getByRole("progressbar", { name: "Getting widget" })).toHaveAttribute(
      "value",
      "45",
    );
    expect(screen.getByRole("button", { name: "Get project" })).toBeDisabled();

    clone.finish(HOME_FOLDER);
    await vi.waitFor(() => expect(onOpen).toHaveBeenCalledWith(HOME_FOLDER));
    expect(screen.queryByRole("progressbar")).not.toBeInTheDocument();
  });

  it("calls the clone off with Cancel, and says nothing was left behind", async () => {
    const clone = core();
    const { onOpen, person } = page();
    await person.type(screen.getByLabelText("Project URL"), URL);
    await person.click(screen.getByRole("button", { name: "Get project" }));
    clone.tell({ stage: "Downloading", percent: 10 });

    await person.click(await screen.findByRole("button", { name: "Cancel" }));
    expect(clone.asked.some((one) => one.cmd === "cancel_get_project")).toBe(true);
    clone.finish(null);

    expect(await screen.findByText("Cancelled. Nothing was left behind.")).toBeInTheDocument();
    expect(onOpen).not.toHaveBeenCalled();
  });

  it("calls the clone off when the person leaves the page while it runs", async () => {
    const clone = core();
    const { person } = page();
    await person.type(screen.getByLabelText("Project URL"), URL);
    await person.click(screen.getByRole("button", { name: "Get project" }));
    await vi.waitFor(() => expect(clone.asked.some((one) => one.cmd === "get_project")).toBe(true));

    cleanup();

    expect(clone.asked.some((one) => one.cmd === "cancel_get_project")).toBe(true);
  });

  it("says each refusal in the core's sentence, and Retry clones again", async () => {
    const clone = core();
    const { person } = page();
    await person.type(screen.getByLabelText("Project URL"), URL);
    await person.click(screen.getByRole("button", { name: "Get project" }));
    await vi.waitFor(() => expect(clone.asked.some((one) => one.cmd === "get_project")).toBe(true));

    const sentence = "There is no project at https://github.com/acme/widget.git. Check the owner.";
    clone.fail(sentence);

    expect(await screen.findByRole("alert")).toHaveTextContent(sentence);
    await person.click(screen.getByRole("button", { name: "Retry" }));
    await vi.waitFor(() =>
      expect(clone.asked.filter((one) => one.cmd === "get_project")).toHaveLength(2),
    );
  });

  it("says a bad URL before anything is cloned, with Retry", async () => {
    const said = "purlis gets a project over HTTPS or SSH. Paste a URL like https://…";
    const clone = core({
      project_url_folder: () => {
        throw said;
      },
    });
    const { person } = page();
    await person.type(screen.getByLabelText("Project URL"), "http://github.com/acme/widget");

    await person.click(screen.getByRole("button", { name: "Get project" }));

    expect(await screen.findByRole("alert")).toHaveTextContent(said);
    expect(screen.getByRole("button", { name: "Retry" })).toBeInTheDocument();
    expect(clone.asked.some((one) => one.cmd === "get_project")).toBe(false);
  });

  it("puts it where the person chose, and remembers that place for the next project", async () => {
    const clone = core({ pick_project: () => "/Volumes/work/code" });
    const { person } = page();
    await person.type(screen.getByLabelText("Project URL"), URL);

    await person.click(screen.getByRole("button", { name: "Choose a folder…" }));

    expect(await screen.findByText("/Volumes/work/code/widget")).toBeInTheDocument();
    expect(globalThis.localStorage.getItem(REMEMBERED_PARENT)).toBe("/Volumes/work/code");
    cleanup();

    // The next time the page opens, the place chosen is where a project goes.
    page();
    await person.type(screen.getByLabelText("Project URL"), URL);
    expect(await screen.findByText("/Volumes/work/code/widget")).toBeInTheDocument();
    expect(clone.asked.filter((one) => one.cmd === "project_url_folder").at(-1)?.args.parent).toBe(
      "/Volumes/work/code",
    );
  });

  it("reaches every act from the keyboard", async () => {
    core();
    page();

    for (const name of ["Back", "Get project", "Choose a folder…", "Open a folder…"])
      expect(screen.getByRole("button", { name })).toHaveAttribute("tabindex", "0");
    expect(screen.getByLabelText("Project URL")).toBeInTheDocument();
  });
});
