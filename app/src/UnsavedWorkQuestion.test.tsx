// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The window's question about the unsaved work a close would lose** (#3929): Keep editing,
 * Discard and leave, or Save and leave, about every work the close names.
 */
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { UnsavedWorkQuestion, type UnsavedAsk } from "./UnsavedWorkQuestion";
import { keepUnsaved, unsavedNow, type UnsavedWork } from "./unsavedWork";

afterEach(() => {
  cleanup();
  for (const one of unsavedNow()) keepUnsaved(one.key, undefined);
});

/** A work registered under `name`, whose save answers `refused` (once each) before it saves. */
function work(name: string, refused: (string | undefined)[] = []): UnsavedWork {
  const one: UnsavedWork = {
    key: name,
    plane: "p",
    view: { from: null, view: "skill-studio", key: "" },
    name,
    save: vi.fn(() => {
      const said = refused.shift();
      if (said === undefined) keepUnsaved(name, undefined);
      return Promise.resolve(said);
    }),
    discard: vi.fn(() => keepUnsaved(name, undefined)),
  };
  keepUnsaved(name, one);
  return one;
}

/** The question about `work`, drawn as the window draws it, and what it was answered with. */
function ask(work: UnsavedWork[]) {
  const asked: UnsavedAsk = { work, then: vi.fn(), kept: vi.fn() };
  const answered = vi.fn();
  render(<UnsavedWorkQuestion asked={asked} onAnswered={answered} />);
  return { asked, answered, question: () => screen.getByRole("alertdialog") };
}

describe("the question about unsaved work", () => {
  it("names the work, and Keep editing keeps it and goes nowhere", async () => {
    const deploy = work("deploy");
    const { asked, answered, question } = ask([deploy]);
    expect(question()).toHaveAccessibleName("Save your changes to deploy?");

    await userEvent.click(within(question()).getByRole("button", { name: "Keep editing" }));
    expect(answered).toHaveBeenCalledOnce();
    expect(asked.kept).toHaveBeenCalledOnce();
    expect(asked.then).not.toHaveBeenCalled();
    expect(deploy.discard).not.toHaveBeenCalled();
    expect(unsavedNow()).toEqual([deploy]);
  });

  it("Discard and leave lets every work go, then goes on", async () => {
    const deploy = work("deploy");
    const review = work("review");
    const { asked, question } = ask([deploy, review]);
    expect(question()).toHaveAccessibleName("Save your changes to deploy and review?");

    await userEvent.click(within(question()).getByRole("button", { name: "Discard and leave" }));
    expect(deploy.discard).toHaveBeenCalledOnce();
    expect(review.discard).toHaveBeenCalledOnce();
    expect(asked.then).toHaveBeenCalledOnce();
    expect(unsavedNow()).toEqual([]);
  });

  it("Save and leave saves every work, then goes on", async () => {
    const deploy = work("deploy");
    const review = work("review");
    const { asked, answered, question } = ask([deploy, review]);

    await userEvent.click(within(question()).getByRole("button", { name: "Save and leave" }));
    await waitFor(() => expect(asked.then).toHaveBeenCalledOnce());
    expect(deploy.save).toHaveBeenCalledOnce();
    expect(review.save).toHaveBeenCalledOnce();
    expect(answered).toHaveBeenCalledOnce();
    expect(deploy.discard).not.toHaveBeenCalled();
  });

  it("stays asked where a save is refused, says why, and names only what is left", async () => {
    const deploy = work("deploy");
    const review = work("review", ["nothing was saved: its frontmatter names no description:"]);
    const { asked, answered, question } = ask([deploy, review]);

    await userEvent.click(within(question()).getByRole("button", { name: "Save and leave" }));
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "review: nothing was saved: its frontmatter names no description:",
    );
    expect(asked.then).not.toHaveBeenCalled();
    expect(answered).not.toHaveBeenCalled();
    expect(question()).toHaveAccessibleName("Save your changes to review?");

    // Asked again, only what is left is saved.
    await userEvent.click(within(question()).getByRole("button", { name: "Save and leave" }));
    await waitFor(() => expect(asked.then).toHaveBeenCalledOnce());
    expect(deploy.save).toHaveBeenCalledOnce();
    expect(review.save).toHaveBeenCalledTimes(2);
  });
});
