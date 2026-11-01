// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { BrowserLoginNotice } from "./BrowserLoginNotice";
import { grants } from "./noticeSettle";
import type { Shown } from "./bindings";

/**
 * A login a chat suggests is one notice (#4595): the site large and editable, the vault and key
 * names, what the person is warned of, the line Always writes, and a press names what was drawn.
 */

const login = (more: Partial<NonNullable<Shown["login"]>> = {}) => ({
  domain: "shop.example.test",
  path: "/login",
  vault: "qa",
  user_key: "username",
  key: "password",
  warnings: [],
  line: 'shop-login = { vault = "qa" }',
  under: "[sandbox.personas.qa] tools",
  no_always: null,
  vault_too: false,
  ...more,
});

const ask = (more: Partial<NonNullable<Shown["login"]>> = {}): Shown => ({
  session: 7,
  ask: "browser-login:1",
  says: "Wants to log its browser in",
  options: [
    { id: "once", label: "Allow once", allows: true, preselected: null },
    { id: "chat", label: "Allow for this chat", allows: true, preselected: null },
    { id: "you", label: "Always allow", allows: true, preselected: null },
    { id: "keep", label: "Keep blocked", allows: false, preselected: null },
  ],
  source: "browser-login",
  chain: [],
  answer: { via: "browser-login", id: 1, shown: "digest" },
  since: null,
  login: login(more),
});

afterEach(() => {
  cleanup();
  clearMocks();
});

describe("BrowserLoginNotice", () => {
  it("shows the site, the names and the line, and never a value", () => {
    render(<BrowserLoginNotice plane="p" session={7} asks={[ask()]} />);
    expect(screen.getByRole("textbox")).toHaveValue("shop.example.test/login");
    expect(screen.getByTestId("login-site")).toHaveTextContent("password");
    expect(screen.getByTestId("login-site")).toHaveTextContent('shop-login = { vault = "qa" }');
  });

  it("says each warning, and why Always isn't offered", () => {
    render(
      <BrowserLoginNotice
        plane="p"
        session={7}
        asks={[ask({ warnings: ["This password is also used on https://a.test."], no_always: "Not now" })]}
      />,
    );
    expect(screen.getByRole("alert")).toHaveTextContent("also used on https://a.test");
    expect(screen.getByTestId("login-site")).toHaveTextContent("Not now.");
  });

  it("draws nothing for another chat", () => {
    render(<BrowserLoginNotice plane="p" session={8} asks={[ask()]} />);
    expect(screen.queryByTestId("login-site")).toBeNull();
  });

  it("is a Notice that grants, so a press waits for it to settle", () => {
    expect(grants("browser-login:browser-login:1")).toBe(true);
  });

  it("sends what the person typed with the digest of what was drawn", async () => {
    const sent = vi.fn();
    mockIPC((command, args) => {
      if (command === "answer_browser_login") {
        sent(args);
        return { kind: "done", said: "Allowed." };
      }
      return null;
    });
    const user = userEvent.setup();
    render(<BrowserLoginNotice plane="p" session={7} asks={[ask()]} />);
    const field = screen.getByRole("textbox");
    await user.clear(field);
    await user.type(field, "other.example.test");
    await waitFor(
      async () => {
        await user.click(screen.getByRole("button", { name: "Allow once" }));
        expect(sent).toHaveBeenCalled();
      },
      { timeout: 4000 },
    );
    expect(sent.mock.calls[0]?.[0]).toMatchObject({
      session: 7,
      id: 1,
      press: "once",
      shown: "digest",
      typed: "other.example.test",
    });
  });
});
