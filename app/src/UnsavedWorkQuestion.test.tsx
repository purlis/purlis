// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The window's question about the unsaved work a close would lose** (#3929): Keep editing,
 * Discard and leave, or Save and leave, about every work the close names.
 */
import { afterEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, render, screen, waitFor, within } from "@testing-library/react";
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

  it("Discard and leave goes on, and leaves the work to go with what holds it", async () => {
    const deploy = work("deploy");
    const review = work("review");
    const { asked, question } = ask([deploy, review]);
    expect(question()).toHaveAccessibleName("Save your changes to deploy and review?");

    await userEvent.click(within(question()).getByRole("button", { name: "Discard and leave" }));
    expect(asked.then).toHaveBeenCalledOnce();
    // A close whose own question is then cancelled loses nothing.
    expect(deploy.discard).not.toHaveBeenCalled();
    expect(unsavedNow()).toEqual([deploy, review]);
  });

  it("offers no Save and leave while a view finds its work can't be saved as it is", async () => {
    let checked = (_can: boolean) => {};
    const deploy = work("deploy");
    deploy.check = () => new Promise<boolean>((answer) => (checked = answer));
    const { question } = ask([deploy]);
    const saveAndLeave = within(question()).getByRole("button", { name: "Save and leave" });
    // Still checking: it waits.
    expect(saveAndLeave).toBeDisabled();

    act(() => checked(false));
    await waitFor(() => expect(question()).toHaveTextContent("it can't be saved as it is"));
    expect(saveAndLeave).toBeDisabled();
    expect(within(question()).getByRole("button", { name: "Discard and leave" })).toBeEnabled();
  });

  it("waits for a save under way: Keep editing can't be pressed meanwhile", async () => {
    let saved = (_said: string | undefined) => {};
    const deploy = work("deploy");
    deploy.save = () => new Promise((answer) => (saved = answer));
    const { asked, question } = ask([deploy]);

    await userEvent.click(within(question()).getByRole("button", { name: "Save and leave" }));
    expect(within(question()).getByRole("button", { name: "Keep editing" })).toBeDisabled();
    await userEvent.keyboard("{Escape}");
    expect(asked.kept).not.toHaveBeenCalled();

    act(() => {
      keepUnsaved("deploy", undefined);
      saved(undefined);
    });
    await waitFor(() => expect(asked.then).toHaveBeenCalledOnce());
    expect(asked.kept).not.toHaveBeenCalled();
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
