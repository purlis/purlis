// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { cleanup, render, screen, within } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import type { SecretFilled } from "./bindings";
import { SecretMarks, marksOn, secretMarkSaid } from "./SecretMarks";

/**
 * A vault secret purlis filled is marked in the person's view of the browser (#5119, SF-8):
 * "secret: <key>", named by the key and the site, never by a value, and only while the page is
 * on the site it went to.
 */

const DASH: SecretFilled = {
  key: "dash-api-key",
  origin: "https://dash.example.test",
  page: "https://dash.example.test",
  label: "API key",
  code: false,
};
const LOCAL: SecretFilled = {
  key: "dev-token",
  origin: "http://localhost:3000",
  page: null,
  label: "",
  code: false,
};
/** A card number in a part of a shop's page from its payment site. */
const PAY: SecretFilled = {
  key: "card",
  origin: "https://pay.example.test",
  page: "https://shop.example.test",
  label: "Card number",
  code: false,
};

afterEach(cleanup);

describe("SecretMarks", () => {
  it("marks each secret by its key, and is heard by the key and the site", () => {
    render(<SecretMarks secrets={[DASH, LOCAL]} />);
    const group = screen.getByRole("group", { name: "Secrets purlis filled on this site" });
    expect(
      within(group).getByRole("img", { name: "Secret dash-api-key, filled on dash.example.test" }),
    ).toHaveTextContent(/^secret: dash-api-key$/);
    expect(
      within(group).getByRole("img", { name: "Secret dev-token, filled on http://localhost:3000" }),
    ).toHaveTextContent(/^secret: dev-token$/);
  });

  it("says in its hint which field the page labels, and that the chat never saw the value", () => {
    expect(secretMarkSaid(DASH)).toBe(
      'purlis filled dash-api-key from your vault into the field the page labels "API key" on dash.example.test. The chat never saw the value.',
    );
    expect(secretMarkSaid(LOCAL)).toContain("into a field on http://localhost:3000");
    expect(secretMarkSaid(PAY)).toContain(
      "on pay.example.test, in a part of a page from shop.example.test",
    );
    expect(secretMarkSaid({ ...DASH, key: "dash-2fa", code: true })).toContain(
      "purlis filled a 2FA code from dash-2fa into",
    );
  });

  it("draws nothing where nothing was filled", () => {
    render(<SecretMarks secrets={[]} />);
    expect(screen.queryByRole("group")).toBeNull();
  });
});

describe("marksOn", () => {
  it("keeps the marks of the site the page is on, and none for a page on no site", () => {
    expect(marksOn([DASH, LOCAL], "https://dash.example.test")).toEqual([DASH]);
    expect(marksOn([DASH, LOCAL], "https://other.example.test")).toEqual([]);
    expect(marksOn([DASH], null)).toEqual([]);
    // A field in a part of the page from another site is marked on the page's site too.
    expect(marksOn([PAY], "https://shop.example.test")).toEqual([PAY]);
  });
});
