// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { cleanup, render, renderHook } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import { ChatMark } from "../ui/chat-mark/ChatMark";
import { useArrived } from "./arrived";

describe("telling a change from a first draw", () => {
  it("is false for the value a component mounted with", () => {
    const { result } = renderHook(({ value }) => useArrived(value), {
      initialProps: { value: "running" },
    });
    expect(result.current).toBe(false);
  });

  it("stays false across renders that change nothing", () => {
    const { result, rerender } = renderHook(({ value }) => useArrived(value), {
      initialProps: { value: "running" },
    });
    rerender({ value: "running" });
    expect(result.current).toBe(false);
  });

  it("is true once the value has changed, and for every value after", () => {
    const { result, rerender } = renderHook(({ value }) => useArrived(value), {
      initialProps: { value: "running" },
    });
    rerender({ value: "waiting" });
    expect(result.current).toBe(true);
    // Back to where it started is still an arrival: the operator watched it happen.
    rerender({ value: "running" });
    expect(result.current).toBe(true);
    rerender({ value: "running" });
    expect(result.current).toBe(true);
  });
});

describe("a chat's state mark", () => {
  afterEach(cleanup);

  const badge = (holder: HTMLElement) => holder.querySelector('[data-part="badge"]');

  it("does not move for the state it was drawn in", () => {
    // A workspace's tabs coming back into view must not all knock at once.
    const { container } = render(<ChatMark state="needs" />);
    expect(badge(container)).not.toHaveAttribute("data-arrived");
  });

  it("is marked as arrived when the chat changes state while it is on screen", () => {
    const { container, rerender } = render(<ChatMark state="working" />);
    rerender(<ChatMark state="needs" />);
    expect(container.firstElementChild).toHaveAttribute("data-variant", "needs");
    expect(badge(container)).toHaveAttribute("data-arrived");
  });
});
