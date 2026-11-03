// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { forgetThisLaunch } from "../regions";
import { whereYouAnswer } from "../whereYouAnswer";
import { whereYouAnswerSetting } from "./you";

/** **You › Chats list › Where you answer** (#4565): the remembered choice, changed here. */

function Row() {
  const { control } = whereYouAnswerSetting.useControl();
  return <>{control({ id: "where", label: "Where you answer" })}</>;
}

beforeEach(() => {
  forgetThisLaunch();
  mockIPC(() => null);
});
afterEach(() => {
  cleanup();
  clearMocks();
});

describe("Where you answer, in Settings (#4565)", () => {
  it("is from the top until changed, and remembers from each session", async () => {
    const user = userEvent.setup();
    render(<Row />);
    expect(screen.getByRole("radio", { name: /From the top/ })).toBeChecked();
    await user.click(screen.getByRole("radio", { name: /From each session/ }));
    expect(whereYouAnswer().answer).toBe("each");
    expect(screen.getByRole("radio", { name: /From each session/ })).toBeChecked();
  });

  it("counts as the choice offered, so the line above the first question is not offered again", async () => {
    const user = userEvent.setup();
    render(<Row />);
    await user.click(screen.getByRole("radio", { name: /From the top/ }));
    await user.click(screen.getByRole("radio", { name: /From each session/ }));
    expect(whereYouAnswer().offered).toBe(true);
  });
});
