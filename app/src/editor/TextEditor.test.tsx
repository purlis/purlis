// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { createRef, useState } from "react";
import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { afterEach, describe, expect, it } from "vitest";
import { onAMac } from "../tabKeys";
import { ConflictMerge, TAKE_FROM_DISK, TextEditor, type TextEditorHandle } from "./TextEditor";
import { backspace, textOf, typeIn, typeable } from "./typing.testkit";

/**
 * **The text editor** (DS-10, #1842): Settings' Edit as TOML and Edit as JSON type into the
 * light editor's CodeMirror. What a person sees and does: a named text with line numbers,
 * colours from the file's grammar, the find bar, typing handed to the owner, and — when the file
 * changed on disk — the file beside the edit, with nothing dropped.
 */

afterEach(cleanup);

const TOML = '# the team\'s\n[plane]\nmode = "push"\n';

/** An owner that keeps what is typed, as Settings keeps a draft. */
function Owned({ start, path = "charter.toml" }: { start: string; path?: string }) {
  const [text, setText] = useState(start);
  return (
    <>
      <TextEditor path={path} label="charter.toml, as TOML" text={text} onChange={setText} />
      <output data-testid="owned">{text}</output>
    </>
  );
}

const named = () => screen.findByRole("textbox", { name: "charter.toml, as TOML" });

describe("the text editor", () => {
  it("is a named text with the file's lines, numbered", async () => {
    const { container } = render(<Owned start={TOML} />);

    expect(textOf(await named())).toBe(TOML);
    expect(
      // CodeMirror's hidden spacer, which sizes the gutter, is not a line's number.
      [...container.querySelectorAll<HTMLElement>(".cm-lineNumbers .cm-gutterElement")]
        .filter((one) => one.style.visibility !== "hidden")
        .map((one) => one.textContent),
    ).toEqual(["1", "2", "3", "4"]);
  });

  it("colours TOML from its grammar", async () => {
    const { container } = render(<Owned start={TOML} />);

    // The grammar is its own chunk, so the colours arrive after the text.
    await waitFor(() =>
      expect(container.querySelectorAll(".cm-line span").length).toBeGreaterThan(0),
    );
  });

  it("colours JSON too, for a workspace's manifest", async () => {
    const { container } = render(<Owned path="workspace.json" start={'{\n  "live": true\n}\n'} />);

    await waitFor(() =>
      expect(container.querySelectorAll(".cm-line span").length).toBeGreaterThan(0),
    );
  });

  it("hands every typed change to its owner", async () => {
    render(<Owned start={TOML} />);
    const text = await named();

    typeIn(text, "[update]\n");
    backspace(text);

    expect(screen.getByTestId("owned").textContent).toBe(`${TOML}[update]`);
    expect(textOf(text)).toBe(`${TOML}[update]`);
  });

  it("undoes what was typed with the history keys", async () => {
    render(<Owned start={TOML} />);
    const text = await named();
    typeIn(text, "# mine");

    const mac = onAMac();
    fireEvent.keyDown(text, { key: "z", metaKey: mac, ctrlKey: !mac });

    await waitFor(() => expect(screen.getByTestId("owned").textContent).toBe(TOML));
  });

  it("shows a text its owner put back, and hands nothing back for it", async () => {
    const heard: string[] = [];
    const { rerender } = render(
      <TextEditor
        path="charter.toml"
        label="charter.toml, as TOML"
        text={TOML}
        onChange={(to) => heard.push(to)}
      />,
    );
    const text = await named();

    rerender(
      <TextEditor
        path="charter.toml"
        label="charter.toml, as TOML"
        text={"[harness]\n"}
        onChange={(to) => heard.push(to)}
      />,
    );

    expect(textOf(text)).toBe("[harness]\n");
    expect(heard).toEqual([]);
  });

  it("is read only while it is off, and typed into again once it is on", async () => {
    const { rerender } = render(
      <TextEditor
        path="charter.toml"
        label="charter.toml, as TOML"
        text={TOML}
        onChange={() => {}}
        off
      />,
    );
    const text = await named();
    expect(text).toHaveAttribute("aria-readonly", "true");
    expect(typeable(text)).toBe(false);

    rerender(
      <TextEditor
        path="charter.toml"
        label="charter.toml, as TOML"
        text={TOML}
        onChange={() => {}}
      />,
    );

    expect(typeable(text)).toBe(true);
  });

  it("leaves Tab to the form: it does not indent", async () => {
    render(<Owned start={TOML} />);
    const text = await named();
    text.focus();

    fireEvent.keyDown(text, { key: "Tab" });

    expect(screen.getByTestId("owned").textContent).toBe(TOML);
  });

  it("takes the keyboard when its owner asks", async () => {
    const handle = createRef<TextEditorHandle>();
    render(
      <TextEditor
        path="charter.toml"
        label="charter.toml, as TOML"
        text={TOML}
        onChange={() => {}}
        ref={handle}
      />,
    );
    const text = await named();

    act(() => handle.current?.focus());

    expect(text).toHaveFocus();
  });

  it("finds in the text on ⌘F or Ctrl+F, marking every match", async () => {
    const { container } = render(<Owned start={TOML} />);
    const text = await named();
    const mac = onAMac();

    fireEvent.keyDown(text, { key: "f", metaKey: mac, ctrlKey: !mac });
    const field = await screen.findByRole("searchbox", { name: "Find" });
    expect(screen.getByRole("search", { name: "Find in charter.toml, as TOML" })).toBeVisible();
    await userEvent.type(field, "p");

    const bar = screen.getByRole("search", { name: "Find in charter.toml, as TOML" });
    expect(within(bar).getByRole("status")).toHaveTextContent("1 of 2");
    expect(container.querySelectorAll(".cm-find-match, .cm-find-match-active")).toHaveLength(2);
  });
});

describe("the merge view of a file changed on disk", () => {
  const DISK = `${TOML}# edited by hand\n`;
  const MINE = `${TOML}# mine\n`;

  function Conflict() {
    const [mine, setMine] = useState(MINE);
    return (
      <>
        <ConflictMerge
          path="charter.toml"
          label="charter.toml, as TOML"
          diskLabel="charter.toml, on disk now"
          disk={DISK}
          mine={mine}
          onChange={setMine}
        />
        <output data-testid="owned">{mine}</output>
      </>
    );
  }

  it("shows the file as it now is beside the edit, the difference marked on both", async () => {
    const { container } = render(<Conflict />);

    const disk = await screen.findByRole("textbox", { name: "charter.toml, on disk now" });
    const mine = screen.getByRole("textbox", { name: "charter.toml, as TOML" });
    expect(textOf(disk)).toBe(DISK);
    expect(textOf(mine)).toBe(MINE);
    expect(
      screen.getByRole("group", {
        name: "charter.toml, on disk now, beside charter.toml, as TOML",
      }),
    ).toBeInTheDocument();
    const [a, b] = [...container.querySelectorAll(".cm-editor")];
    const changed = (side: Element) =>
      [...side.querySelectorAll(".cm-changedLine")].map((line) => line.textContent);
    expect(changed(a)).toEqual(["# edited by hand"]);
    expect(changed(b)).toEqual(["# mine"]);
  });

  it("keeps the file's side read only, and the edit typed into", async () => {
    render(<Conflict />);
    const disk = await screen.findByRole("textbox", { name: "charter.toml, on disk now" });
    const mine = screen.getByRole("textbox", { name: "charter.toml, as TOML" });

    expect(typeable(disk)).toBe(false);
    typeIn(mine, "# more\n");

    expect(screen.getByTestId("owned").textContent).toBe(`${MINE}# more\n`);
    expect(textOf(disk)).toBe(DISK);
  });

  it("takes the file's side of a difference into the edit with its arrow", async () => {
    render(<Conflict />);

    // Drawn once the merge view has measured where each difference is.
    const arrow = await screen.findByRole("button", { name: TAKE_FROM_DISK });
    fireEvent.mouseDown(arrow);

    await waitFor(() => expect(screen.getByTestId("owned").textContent).toBe(DISK));
  });

  it("takes them from the keyboard too: Enter or Space on the arrow", async () => {
    render(<Conflict />);
    const arrow = await screen.findByRole("button", { name: TAKE_FROM_DISK });
    arrow.focus();

    // A key on a button is a click with no pointer behind it (`detail` 0).
    fireEvent.click(arrow, { detail: 0 });

    await waitFor(() => expect(screen.getByTestId("owned").textContent).toBe(DISK));
  });
});
