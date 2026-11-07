// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { answersOf } from "./ui/dialog/dialog.testkit";
import { RenameTab, type RenameTabProps } from "./RenameTab";

afterEach(cleanup);

/**
 * **A tab's Rename says what it renames** (#5343, spec #5339): a tab with one chat renames the
 * chat unless "Only this tab" is ticked; a split names the tab only; a tab with a name of its
 * own can go back to its chat's.
 */
function show(props: Partial<RenameTabProps> = {}) {
  const onRename = vi.fn();
  const onClear = vi.fn();
  const onCancel = vi.fn();
  render(
    <RenameTab
      renames={{ kind: "chat", session: 7 }}
      name="steward 3"
      chat="steward 3"
      own={false}
      busy={false}
      onRename={onRename}
      onClear={onClear}
      onCancel={onCancel}
      {...props}
    />,
  );
  return { onRename, onClear, onCancel };
}

const box = () => screen.getByLabelText("New name");
const only = () => screen.getByRole("checkbox", { name: "Only this tab" });

describe("a tab with one chat", () => {
  it("says it renames the chat, everywhere it is named", () => {
    show();

    const dialog = screen.getByRole("dialog", { name: "Rename steward 3" });
    expect(dialog).toHaveTextContent(
      "This tab shows one chat, so this renames the chat: on its tab, in the list and on its head.",
    );
    expect(answersOf(dialog)).toEqual(["Cancel", "Rename chat"]);
  });

  it("opens on the chat's name, selected, and Enter renames the chat", async () => {
    const { onRename } = show();

    expect(box()).toHaveFocus();
    expect(box()).toHaveValue("steward 3");
    await userEvent.keyboard("planner{Enter}");

    expect(onRename).toHaveBeenCalledWith({ typed: "planner", only: false });
  });

  it("names only the tab once Only this tab is ticked, and says the chat keeps its name", async () => {
    const { onRename } = show();

    expect(only()).toHaveAccessibleDescription("The chat keeps its name, steward 3.");
    await userEvent.click(only());
    expect(answersOf(screen.getByRole("dialog"))).toEqual(["Cancel", "Rename tab"]);
    await userEvent.clear(box());
    await userEvent.type(box(), "the build{Enter}");

    expect(onRename).toHaveBeenCalledWith({ typed: "the build", only: true });
  });

  it("opens on the tab's own name, ticked, with a way back to the chat's", async () => {
    const { onClear } = show({ name: "the build", own: true });

    expect(box()).toHaveValue("the build");
    expect(only()).toBeChecked();
    expect(answersOf(screen.getByRole("dialog"))).toEqual([
      "Cancel",
      "Use the chat name again",
      "Rename tab",
    ]);
    await userEvent.click(screen.getByRole("button", { name: "Use the chat name again" }));

    expect(onClear).toHaveBeenCalled();
  });

  it("has nothing to rename until the name or the tick changes", async () => {
    show();

    expect(screen.getByRole("button", { name: "Rename chat" })).toBeDisabled();
    await userEvent.click(only());
    expect(screen.getByRole("button", { name: "Rename tab" })).toBeEnabled();
  });
});

describe("a split tab", () => {
  it("says it names the tab only, and that the chats keep their names", () => {
    show({ renames: { kind: "tab", panes: 3 } });

    const dialog = screen.getByRole("dialog", { name: "Rename tab steward 3" });
    expect(dialog).toHaveTextContent(
      "This tab shows 3 panes, so this names the tab only. The chats keep their names, and each pane's ⋯ renames its own.",
    );
    expect(screen.queryByRole("checkbox")).toBeNull();
    expect(answersOf(dialog)).toEqual(["Cancel", "Rename tab"]);
  });

  it("names the tab, and a blank names nothing", async () => {
    const { onRename } = show({ renames: { kind: "tab", panes: 2 } });

    await userEvent.clear(box());
    expect(screen.getByRole("button", { name: "Rename tab" })).toBeDisabled();
    await userEvent.type(box(), "the build{Enter}");

    expect(onRename).toHaveBeenCalledWith({ typed: "the build", only: true });
  });
});

describe("either", () => {
  it("changes nothing on Escape", async () => {
    const { onRename, onCancel } = show();

    await userEvent.type(box(), "x");
    await userEvent.keyboard("{Escape}");

    expect(onCancel).toHaveBeenCalled();
    expect(onRename).not.toHaveBeenCalled();
  });

  it("says the core's refusal beside the box, in its words", () => {
    show({ trouble: "That name is too long." });

    expect(screen.getByRole("alert")).toHaveTextContent("That name is too long.");
  });
});
