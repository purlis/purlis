// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { CodeBlock } from "./CodeBlock";

afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
});

describe("CodeBlock", () => {
  it("names the text for a reader and lets the keyboard reach it", () => {
    render(<CodeBlock label="Commands" text="docker version" />);
    expect(screen.getByRole("figure", { name: "Commands" })).toBeTruthy();
    expect(screen.getByText("docker version").closest("pre")?.tabIndex).toBe(0);
  });

  it("copies the whole text and says so", async () => {
    const writeText = vi.fn().mockResolvedValue(undefined);
    const user = userEvent.setup();
    Object.defineProperty(navigator, "clipboard", { value: { writeText }, configurable: true });
    render(<CodeBlock label="Commands" text={"one\ntwo"} />);
    await user.click(screen.getByRole("button", { name: /^Copy/ }));
    expect(writeText).toHaveBeenCalledWith("one\ntwo");
    await waitFor(() => expect(screen.getByRole("status").textContent).toBe("Copied"));
  });

  it("is named by the word it shows once it has copied (WCAG 2.5.3)", async () => {
    const writeText = vi.fn().mockResolvedValue(undefined);
    const user = userEvent.setup();
    Object.defineProperty(navigator, "clipboard", { value: { writeText }, configurable: true });
    render(<CodeBlock label="Image" text="img@sha256:a" copy="Copy digest" />);
    await user.click(screen.getByRole("button", { name: "Copy digest: img@sha256:a" }));
    await waitFor(() =>
      expect(screen.getByRole("button", { name: "Copied: img@sha256:a" })).toBeTruthy(),
    );
  });

  it("says when it couldn't copy", async () => {
    const writeText = vi.fn().mockRejectedValue(new Error("refused"));
    const user = userEvent.setup();
    Object.defineProperty(navigator, "clipboard", { value: { writeText }, configurable: true });
    render(<CodeBlock label="Commands" text="ls" />);
    await user.click(screen.getByRole("button", { name: /^Copy/ }));
    await waitFor(() => expect(screen.getByRole("status").textContent).toBe("Couldn't copy"));
  });
});
