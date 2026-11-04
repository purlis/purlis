// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { SecretFillNotice } from "./SecretFillNotice";
import { grants } from "./noticeSettle";
import type { FillShown, Shown } from "./bindings";

/**
 * A secret a chat asks purlis to fill is one notice (#5115): the key and its vault by name, the
 * exact site large, the page and the field's label, never a value; Allow once and Keep blocked;
 * and a press names what was drawn.
 */

const fill = (more: Partial<FillShown> = {}): FillShown => ({
  key: "dash-api-key",
  vault: "qa",
  site: "dash.example.test",
  page: "https://dash.example.test/settings",
  label: "API key",
  browser: "b1",
  warnings: [],
  code: false,
  ...more,
});

const ask = (more: Partial<FillShown> = {}): Shown => ({
  session: 7,
  ask: "secret-fill:1",
  says: "Wants to put dash-api-key from vault qa into a field on dash.example.test",
  options: [
    { id: "once", label: "Allow once", allows: true, preselected: null },
    { id: "keep", label: "Keep blocked", allows: false, preselected: null },
  ],
  source: "secret-fill",
  chain: [],
  answer: { via: "secret-fill", id: 1, shown: "digest" },
  since: null,
  fill: fill(more),
});

afterEach(() => {
  cleanup();
  clearMocks();
});

describe("SecretFillNotice", () => {
  it("says in plain words which key goes into which field on which site", () => {
    render(<SecretFillNotice plane="p" session={7} asks={[ask()]} />);
    const site = screen.getByTestId("fill-site");
    expect(site).toHaveTextContent(
      "Put dash-api-key from vault qa into the field the page labels API key on",
    );
    expect(screen.getByTestId("fill-site-site")).toHaveTextContent(/^dash\.example\.test$/);
    expect(site).toHaveTextContent("https://dash.example.test/settings");
    expect(site).toHaveTextContent("the chat never sees the value");
  });

  it("says a 2FA code is made from the seed, and that the seed reaches neither page nor chat", () => {
    render(
      <SecretFillNotice plane="p" session={7} asks={[ask({ key: "dash-2fa", code: true })]} />,
    );
    const site = screen.getByTestId("fill-site");
    expect(site).toHaveTextContent(
      "Put a 2FA code from dash-2fa in vault qa into the field the page labels API key on",
    );
    expect(site).toHaveTextContent(
      "purlis works the code out from the seed as it fills, and the page and the chat never see the seed.",
    );
    expect(site).not.toHaveTextContent("the chat never sees the value");
  });

  it("offers Allow once and Keep blocked, and nothing more", () => {
    render(<SecretFillNotice plane="p" session={7} asks={[ask()]} />);
    expect(screen.getByRole("button", { name: "Allow once" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Keep blocked" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /Always/ })).toBeNull();
    expect(screen.queryByRole("button", { name: /this chat/ })).toBeNull();
  });

  it("says each warning, and a field the page labels nothing", () => {
    render(
      <SecretFillNotice
        plane="p"
        session={7}
        asks={[ask({ label: " ", warnings: ["Vault prod is ops's, not qa's."] })]}
      />,
    );
    expect(screen.getByTestId("fill-site")).toHaveTextContent("into a field on");
    expect(screen.getByTestId("fill-site")).toHaveTextContent("Vault prod is ops's, not qa's.");
  });

  it("draws nothing for another chat", () => {
    render(<SecretFillNotice plane="p" session={8} asks={[ask()]} />);
    expect(screen.queryByTestId("fill-site")).toBeNull();
  });

  it("is a Notice that grants, so a press waits for it to settle", () => {
    expect(grants("secret-fill:secret-fill:1:digest")).toBe(true);
  });

  it("sends the press with the digest of what was drawn", async () => {
    const sent = vi.fn();
    mockIPC((command, args) => {
      if (command === "answer_secret_fill") {
        sent(args);
        return { kind: "done", said: "Allowed once." };
      }
      return null;
    });
    const user = userEvent.setup();
    render(<SecretFillNotice plane="p" session={7} asks={[ask()]} />);
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
      reason: null,
    });
  });

  it("is reached and answered from the keyboard", async () => {
    const sent = vi.fn();
    mockIPC((command, args) => {
      if (command === "answer_secret_fill") {
        sent(args);
        return { kind: "done", said: "Kept blocked. Nothing was filled." };
      }
      return null;
    });
    const user = userEvent.setup();
    render(<SecretFillNotice plane="p" session={7} asks={[ask()]} />);
    await waitFor(
      async () => {
        screen.getByRole("button", { name: "Keep blocked" }).focus();
        await user.keyboard("{Enter}");
        expect(sent).toHaveBeenCalled();
      },
      { timeout: 4000 },
    );
    expect(sent.mock.calls[0]?.[0]).toMatchObject({ press: "keep" });
  });
});
