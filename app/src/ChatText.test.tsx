// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render, screen, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { ChatText, FOLD_LINES, linesIn } from "./ChatText";

/**
 * **A chat's text is markdown under a safe subset** (#5214, the person's ruling of 2026-11-04):
 * what is drawn, what is dropped, what never loads and what a press alone opens.
 */

afterEach(() => {
  cleanup();
  clearMocks();
});

/** The addresses the opener plugin was asked to open, from now on. */
function opened(): string[] {
  const urls: string[] = [];
  mockIPC((cmd, args) => {
    if (cmd === "plugin:opener|open_url") urls.push((args as { url: string }).url);
    return null;
  });
  return urls;
}

describe("ChatText draws the subset", () => {
  it("draws paragraphs, lists, bold, italics and code as themselves", () => {
    const { container } = render(
      <ChatText
        text={"One **bold** and *slanted* `code`.\n\n- first\n- second\n\n```\nblock\n```"}
      />,
    );

    expect(screen.getByText("bold").tagName).toBe("STRONG");
    expect(screen.getByText("slanted").tagName).toBe("EM");
    expect(screen.getByText("code").tagName).toBe("CODE");
    expect(container.querySelectorAll("li")).toHaveLength(2);
    expect(container.querySelector("pre")?.textContent).toBe("block\n");
    expect(container.textContent).not.toContain("**");
  });

  it("keeps a line break inside a paragraph", () => {
    const { container } = render(<ChatText text={"first line\nsecond line"} />);

    expect(container.querySelector("p")?.querySelector("br")).not.toBeNull();
  });

  it("draws a table", () => {
    render(<ChatText text={"| leg | p95 |\n| --- | --- |\n| claude | 302 ms |"} />);

    const table = screen.getByRole("table");
    expect(within(table).getByRole("columnheader", { name: "leg" })).toBeInTheDocument();
    expect(within(table).getByRole("cell", { name: "302 ms" })).toBeInTheDocument();
  });

  it("draws a heading no larger than bold words, and never as a heading of the page", () => {
    render(<ChatText text={"# Big news\n\nThe rest."} />);

    expect(screen.queryByRole("heading")).toBeNull();
    expect(screen.getByText("Big news").tagName).toBe("STRONG");
  });
});

describe("ChatText drops and never loads", () => {
  it("drops raw HTML rather than drawing it or printing it", () => {
    const { container } = render(
      <ChatText text={'Before <b onclick="x()">tag</b> after.\n\n<script>alert(1)</script>'} />,
    );

    expect(container.querySelector("b, script")).toBeNull();
    expect(container.textContent).not.toContain("<b");
    expect(container.textContent).not.toContain("alert");
  });

  it("never draws a picture, so none is fetched, and shows its alt text in its place", () => {
    const { container } = render(
      <ChatText text={"See ![the graph](https://tracker.example/pixel.png?who=me) here."} />,
    );

    expect(container.querySelector("img, picture, source, [src], [srcset]")).toBeNull();
    expect(container.innerHTML).not.toContain("tracker.example");
    expect(screen.getByText("the graph")).toBeInTheDocument();
  });

  it("never draws a picture written as raw HTML", () => {
    const { container } = render(<ChatText text={'<img src="https://tracker.example/x.png">'} />);

    expect(container.querySelector("img, [src]")).toBeNull();
  });

  it("draws a picture inside a link as the link, with its alt text for words", () => {
    render(<ChatText text={"[![the docs](https://x.example/a.png)](https://docs.example/page)"} />);

    expect(screen.getByRole("link", { name: "the docs" })).toBeInTheDocument();
  });

  it("says a picture with no alt text was not shown, rather than drawing nothing", () => {
    const { container } = render(<ChatText text={"![](https://x.example/a.png)"} />);

    expect(container.querySelector("img")).toBeNull();
    expect(container.textContent).toBe("Picture not shown");
  });
});

describe("ChatText's links", () => {
  it("shows where a link goes beside its words", () => {
    render(<ChatText text={"Read [the plan](https://github.com/purlis/purlis/issues/1)."} />);

    expect(screen.getByRole("link", { name: "the plan" })).toBeInTheDocument();
    expect(screen.getByText("(github.com)")).toBeInTheDocument();
  });

  it("names the host once where the link's words are its address", () => {
    const { container } = render(<ChatText text={"See https://example.com/a for it."} />);

    expect(screen.getByRole("link", { name: "https://example.com/a" })).toBeInTheDocument();
    expect(container.textContent).toBe("See https://example.com/a for it.");
  });

  it("opens a link in the browser only when pressed, never in the window", async () => {
    const urls = opened();
    render(<ChatText text={"[the plan](https://example.com/plan)"} />);

    expect(urls).toEqual([]);
    await userEvent.click(screen.getByRole("link", { name: "the plan" }));

    expect(urls).toEqual(["https://example.com/plan"]);
  });

  it.each([
    ["javascript:", "[run me](javascript:alert(1))"],
    ["data:", "[open me](data:text/html;base64,PHNjcmlwdD4=)"],
    ["file:", "[read me](file:///etc/passwd)"],
  ])("never follows a %s link: it is drawn as its words", async (_, text) => {
    const urls = opened();
    const { container } = render(<ChatText text={text} />);

    expect(screen.queryByRole("link")).toBeNull();
    expect(container.querySelector("a, [href]")).toBeNull();
    await userEvent.click(container.querySelector("p") as HTMLElement);
    expect(urls).toEqual([]);
  });
});

describe("a long message folds", () => {
  const long = Array.from({ length: 9 }, (_, at) => `Line ${at + 1}`).join("\n");

  it("counts a message's lines, blank ones between paragraphs included", () => {
    expect(linesIn("one")).toBe(1);
    expect(linesIn("one\ntwo\n\nthree\n")).toBe(4);
    expect(linesIn("")).toBe(0);
  });

  it(`draws a message of ${FOLD_LINES} lines whole, with nothing to press`, () => {
    render(<ChatText text={long.split("\n").slice(0, FOLD_LINES).join("\n")} />);

    expect(screen.queryByRole("button")).toBeNull();
  });

  it("clamps a longer one and says how many lines it has, then opens and folds it", async () => {
    const { container } = render(<ChatText text={long} />);
    const body = container.querySelector("[data-folded]");

    const show = screen.getByRole("button", { name: "Show all · 9 lines" });
    expect(show).toHaveAttribute("aria-expanded", "false");
    expect(show.getAttribute("aria-controls")).toBe(body?.id);
    expect(body).toHaveAttribute("data-folded", "true");
    // Folded only to the eye: every line is still there to be read.
    expect(body?.textContent).toContain("Line 9");

    await userEvent.click(show);
    const less = screen.getByRole("button", { name: "Show less" });
    expect(less).toHaveAttribute("aria-expanded", "true");
    expect(body).toHaveAttribute("data-folded", "false");

    await userEvent.click(less);
    expect(screen.getByRole("button", { name: "Show all · 9 lines" })).toBeInTheDocument();
  });

  it("opens from the keyboard", async () => {
    render(<ChatText text={long} />);

    await userEvent.tab();
    expect(screen.getByRole("button", { name: "Show all · 9 lines" })).toHaveFocus();
    await userEvent.keyboard("{Enter}");
    expect(screen.getByRole("button", { name: "Show less" })).toHaveFocus();
  });

  it("never folds where the caller says it is drawn whole", () => {
    render(<ChatText text={long} fold={false} />);

    expect(screen.queryByRole("button")).toBeNull();
  });
});
