// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { afterEach, describe, expect, it } from "vitest";
import { onAMac } from "../tabKeys";
import { LightEditor, MergeViewer, opensFindInAFile } from "./LightEditor";
import { gitHunks } from "./gitdiff.testkit";

afterEach(cleanup);

/** The text of each line an editor draws, in order. */
function lines(root: Element): string[] {
  return [...root.querySelectorAll(".cm-line")].map((line) => line.textContent ?? "");
}

describe("the light editor", () => {
  it("draws a file's text, a line to a line", async () => {
    const { container } = render(
      <LightEditor path="src/lib.rs" text={"pub fn one() -> u8 {\n    1\n}\n"} />,
    );

    await waitFor(() => expect(lines(container)).toContain("pub fn one() -> u8 {"));
    expect(lines(container)).toEqual(["pub fn one() -> u8 {", "    1", "}", ""]);
  });

  it("is for reading: the text cannot be typed into", async () => {
    const { container } = render(<LightEditor path="notes.txt" text={"one\n"} />);

    await waitFor(() => expect(container.querySelector(".cm-content")).not.toBeNull());
    expect(container.querySelector(".cm-content")).toHaveAttribute("aria-readonly", "true");
  });

  it("colours a language it ships a grammar for", async () => {
    const { container } = render(<LightEditor path="src/lib.rs" text={"pub fn one() {}\n"} />);

    // The grammar is its own chunk, so the highlighting arrives after the text.
    await waitFor(() =>
      expect(container.querySelectorAll(".cm-line span").length).toBeGreaterThan(0),
    );
  });
});

describe("the merge view", () => {
  it("renders a diff: the base beside the head, with git's changed lines marked", async () => {
    const base = "one\ntwo\nthree\n";
    const head = "one\nTWO\nthree\nfour\n";

    const { container } = render(
      <MergeViewer path="notes.txt" base={base} head={head} hunks={gitHunks(base, head)} />,
    );

    await waitFor(() => expect(container.querySelector(".cm-mergeView")).not.toBeNull());
    const [a, b] = [...container.querySelectorAll(".cm-editor")];
    expect(lines(a)).toEqual(["one", "two", "three", ""]);
    expect(lines(b)).toEqual(["one", "TWO", "three", "four", ""]);
    const changed = (side: Element) =>
      [...side.querySelectorAll(".cm-changedLine")].map((line) => line.textContent);
    expect(changed(a)).toEqual(["two"]);
    expect(changed(b)).toEqual(["TWO", "four"]);
  });

  it("marks nothing git did not report, even where the texts differ", async () => {
    const { container } = render(
      <MergeViewer path="notes.txt" base={"one\n"} head={"two\n"} hunks={[]} />,
    );

    await waitFor(() => expect(container.querySelector(".cm-mergeView")).not.toBeNull());
    expect(container.querySelectorAll(".cm-changedLine")).toHaveLength(0);
  });
});

describe("find in a file (#946)", () => {
  /** Opens the bar from the editor's text, as the key would: ⌘F on a Mac, Ctrl+F elsewhere. */
  async function openFind(container: HTMLElement) {
    await waitFor(() => expect(container.querySelector(".cm-content")).not.toBeNull());
    const content = container.querySelector(".cm-content") as HTMLElement;
    const mac = onAMac();
    fireEvent.keyDown(content, { key: "f", metaKey: mac, ctrlKey: !mac });
    return screen.findByRole("searchbox", { name: "Find" });
  }

  const marks = (root: Element) =>
    [...root.querySelectorAll(".cm-find-match, .cm-find-match-active")].map((one) => [
      one.textContent,
      one.classList.contains("cm-find-match-active"),
    ]);

  it("opens the bar on the key, marks every match in any case, and counts them", async () => {
    const { container } = render(
      <LightEditor path="notes.txt" text={"Alpha beta\nalpha gamma\nALPHA\n"} />,
    );
    const field = await openFind(container);
    expect(screen.getByRole("search", { name: "Find in notes.txt" })).toBeInTheDocument();
    expect(field).toHaveFocus();

    await userEvent.type(field, "alpha");

    expect(screen.getByRole("status")).toHaveTextContent("1 of 3");
    expect(marks(container)).toEqual([
      ["Alpha", true],
      ["alpha", false],
      ["ALPHA", false],
    ]);
  });

  it("moves along on Enter and back on Shift+Enter, round the ends", async () => {
    const { container } = render(<LightEditor path="notes.txt" text={"a x a x a\n"} />);
    const field = await openFind(container);
    await userEvent.type(field, "a");

    await userEvent.keyboard("{Enter}");
    expect(screen.getByRole("status")).toHaveTextContent("2 of 3");
    await userEvent.keyboard("{Enter}{Enter}");
    expect(screen.getByRole("status")).toHaveTextContent("1 of 3");
    await userEvent.keyboard("{Shift>}{Enter}{/Shift}");
    expect(screen.getByRole("status")).toHaveTextContent("3 of 3");
  });

  it("says no matches, and leaves nothing marked once it is closed", async () => {
    const { container } = render(<LightEditor path="notes.txt" text={"one two\n"} />);
    const field = await openFind(container);

    await userEvent.type(field, "three");
    expect(screen.getByRole("status")).toHaveTextContent("No matches");
    await userEvent.clear(field);
    await userEvent.type(field, "one");
    expect(marks(container)).toEqual([["one", true]]);

    await userEvent.keyboard("{Escape}");

    expect(screen.queryByRole("search")).toBeNull();
    expect(marks(container)).toEqual([]);
  });

  it("finds in the file as it is drawn now, once the file changed under the bar", async () => {
    const { container, rerender } = render(<LightEditor path="notes.txt" text={"a a\n"} />);
    const field = await openFind(container);
    await userEvent.type(field, "a");
    expect(screen.getByRole("status")).toHaveTextContent("1 of 2");

    rerender(<LightEditor path="notes.txt" text={"a a a\n"} />);
    await waitFor(() => expect(container.querySelector(".cm-line")).toHaveTextContent("a a a"));
    await userEvent.click(screen.getByRole("button", { name: "Next match" }));

    expect(screen.getByRole("status")).toHaveTextContent("1 of 3");
    expect(marks(container)).toHaveLength(3);
  });

  it("finds on both sides of the merge view, the side before first", async () => {
    const base = "one\ntwo\n";
    const head = "one\nTWO two\n";
    const { container } = render(
      <MergeViewer path="notes.txt" base={base} head={head} hunks={gitHunks(base, head)} />,
    );
    const field = await openFind(container);
    expect(
      screen.getByRole("search", { name: "Find in what changed in notes.txt" }),
    ).toBeInTheDocument();

    await userEvent.type(field, "two");

    expect(screen.getByRole("status")).toHaveTextContent("1 of 3");
    const [a, b] = [...container.querySelectorAll(".cm-editor")];
    expect(marks(a)).toEqual([["two", true]]);
    expect(marks(b)).toEqual([
      ["TWO", false],
      ["two", false],
    ]);
  });

  it("is opened by ⌘F on a Mac and Ctrl+F elsewhere, never with Shift or Alt", () => {
    const key = (held: Partial<Record<"metaKey" | "ctrlKey" | "shiftKey" | "altKey", boolean>>) =>
      ({
        key: "f",
        metaKey: false,
        ctrlKey: false,
        shiftKey: false,
        altKey: false,
        ...held,
      }) as never;
    expect(opensFindInAFile(key({ metaKey: true }), true)).toBe(true);
    expect(opensFindInAFile(key({ ctrlKey: true }), true)).toBe(false);
    expect(opensFindInAFile(key({ ctrlKey: true }), false)).toBe(true);
    expect(opensFindInAFile(key({ metaKey: true }), false)).toBe(false);
    expect(opensFindInAFile(key({ metaKey: true, shiftKey: true }), true)).toBe(false);
    expect(opensFindInAFile(key({ ctrlKey: true, altKey: true }), false)).toBe(false);
  });
});
