// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **A persona's removal asks about the unsaved work a save writes into it first** (#4029), as a
 * workspace's does (#3929): the window's one question, then the removal's own. The core is faked
 * at its command boundary, and the work is put in the registry directly, as the Skill studio's
 * editor puts a persona-level skill's.
 */
import { useState } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { usePlaneEdits, type PlaneEditing } from "./PlaneEdits";
import { UnsavedWorkQuestion, type UnsavedAsk } from "./UnsavedWorkQuestion";
import { keepUnsaved, unsavedNow, type UnsavedWork } from "./unsavedWork";

const PLANE = "/home/dev/one";

afterEach(() => {
  cleanup();
  clearMocks();
  for (const one of unsavedNow()) keepUnsaved(one.key, undefined);
});

/** The hook in a window of its own, drawing the question it asks as the window does. */
function Window({ doing }: { doing: (edits: PlaneEditing) => void }) {
  const [asked, setAsked] = useState<UnsavedAsk>();
  const edits = usePlaneEdits({
    plane: PLANE,
    showView: () => undefined,
    closeView: () => undefined,
    reread: () => undefined,
    reloadVaults: () => undefined,
    askUnsaved: setAsked,
  });
  doing(edits.doing);
  return (
    <>
      {edits.dialogs}
      {asked && <UnsavedWorkQuestion asked={asked} onAnswered={() => setAsked(undefined)} />}
    </>
  );
}

/** A persona-level skill being edited, as the studio registers it. */
function work(persona: string): UnsavedWork {
  const one: UnsavedWork = {
    key: `deploy\u0000${persona}`,
    plane: PLANE,
    view: { from: null, view: "skill-studio", key: "" },
    persona,
    name: "deploy",
    save: vi.fn(() => Promise.resolve(undefined)),
    discard: vi.fn(() => keepUnsaved(one.key, undefined)),
  };
  keepUnsaved(one.key, one);
  return one;
}

function drawn() {
  const sent: string[] = [];
  mockIPC((cmd) => {
    sent.push(cmd);
    return null;
  });
  let doing: PlaneEditing | undefined;
  render(<Window doing={(now) => (doing = now)} />);
  return { sent, remove: (persona: string) => doing?.removePersona(persona) };
}

const removal = (persona: string) =>
  screen.findByRole("alertdialog", { name: `Delete persona ${persona}?` });

describe("a persona's removal, with unsaved work a save writes into it", () => {
  it("asks about the work first: Keep editing removes nothing", async () => {
    const { remove } = drawn();
    const deploy = work("steward");

    remove("steward");
    const asked = await screen.findByRole("alertdialog", { name: "Save your changes to deploy?" });
    await userEvent.click(within(asked).getByRole("button", { name: "Keep editing" }));

    expect(screen.queryByRole("alertdialog")).toBeNull();
    expect(deploy.discard).not.toHaveBeenCalled();
  });

  it("goes on to the removal's own question on Discard and leave, and the work goes with it", async () => {
    const { remove, sent } = drawn();
    const deploy = work("steward");

    remove("steward");
    const asked = await screen.findByRole("alertdialog", { name: "Save your changes to deploy?" });
    await userEvent.click(within(asked).getByRole("button", { name: "Discard and leave" }));

    // Nothing goes until the removal is answered.
    const dialog = await removal("steward");
    expect(deploy.discard).not.toHaveBeenCalled();
    await userEvent.click(within(dialog).getByRole("button", { name: "Delete persona" }));

    await vi.waitFor(() => expect(deploy.discard).toHaveBeenCalledOnce());
    expect(sent).toContain("persona_remove");
    expect(unsavedNow()).toEqual([]);
  });

  it("asks nothing about another persona's work", async () => {
    const { remove } = drawn();
    const other = work("reviewer");

    remove("steward");
    await removal("steward");
    expect(screen.queryByRole("alertdialog", { name: /Save your changes/ })).toBeNull();
    expect(other.discard).not.toHaveBeenCalled();
  });
});
