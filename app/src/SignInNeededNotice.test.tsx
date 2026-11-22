// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, render, screen } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import type { SignInNeeded } from "./bindings";
import { SignInNeededNotice } from "./SignInNeededNotice";

/** What the core last told of chat 4's sign-in, which the test sets and the pane re-reads. */
const held = vi.hoisted(() => ({ needed: undefined as SignInNeeded | undefined }));
const listeners = vi.hoisted(() => new Set<() => void>());

vi.mock("./chatState", async (importOriginal) => {
  const { useSyncExternalStore } = await import("react");
  return {
    ...(await importOriginal<typeof import("./chatState")>()),
    useChatsHere: () => ({}),
    useChatsSelect: (_chats: unknown, select: (states: unknown) => unknown) =>
      useSyncExternalStore(
        (on) => {
          listeners.add(on);
          return () => listeners.delete(on);
        },
        () => select({ noticed: held.needed ? { 4: { signIn: held.needed } } : {} }),
      ),
  };
});

const askSettingsLink = vi.hoisted(() => vi.fn());
vi.mock("./settings/links", () => ({ askSettingsLink }));

const tell = (needed: SignInNeeded | undefined) =>
  act(() => {
    held.needed = needed;
    for (const on of listeners) on();
  });

afterEach(() => {
  cleanup();
  held.needed = undefined;
  askSettingsLink.mockClear();
});

describe("SignInNeededNotice", () => {
  it("says nothing while the chat's sign-in is fine", () => {
    render(<SignInNeededNotice plane="/p" session={4} />);
    expect(screen.queryByRole("status")).toBeNull();
  });

  it("opens Settings at Accounts on Sign in", async () => {
    held.needed = { host: "github.com", ended: false };
    render(<SignInNeededNotice plane="/p" session={4} />);
    await userEvent.click(screen.getByRole("button", { name: "Sign in" }));
    expect(askSettingsLink).toHaveBeenCalledWith("/p", { group: "you.accounts" });
  });

  it("is put away by Dismiss and brought back by a new cause", async () => {
    held.needed = { host: "github.com", ended: false };
    render(<SignInNeededNotice plane="/p" session={4} />);
    await userEvent.click(screen.getByRole("button", { name: "Dismiss" }));
    expect(screen.queryByText(/github\.com/)).toBeNull();

    tell({ host: "github.com", ended: true });
    expect(await screen.findByText(/Your sign-in to github\.com has ended/)).toBeTruthy();
  });

  it("goes when the core takes the sign-in down", () => {
    held.needed = { host: "github.com", ended: false };
    render(<SignInNeededNotice plane="/p" session={4} />);
    expect(screen.getByText(/no sign-in to github\.com/)).toBeTruthy();
    tell(undefined);
    expect(screen.queryByText(/github\.com/)).toBeNull();
  });
});
