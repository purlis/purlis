// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { ExtensionView } from "./bindings";
import { forgetOffers, heardOffers, useOffers } from "./offerStore";
import { OFFERS } from "./offersPref";
import { PANE } from "./paneDrag";
import type { ViewRef } from "./tabs";
import {
  ChatOffers,
  OfferNotices,
  type OfferOpening,
  OfferOpeningContext,
  ViewOpenButton,
} from "./ViewOpen";

/**
 * **Opening an extension's view, and its offer** (#4769; prototype `view-open-beside`): a press
 * opens a tab, ⌥ on the press or the beside icon opens it beside, the press's menu has both,
 * and a beside that would fall under the minimum is greyed with its reason before the press.
 * An offer is one quiet note with Open, Open beside, Stop offers and Dismiss.
 */

const STATS: ExtensionView = {
  extension: "statistics",
  id: "stats",
  title: "Statistics",
  about: "personas",
};
const VIEW: ViewRef = { from: "statistics", view: "stats", key: "steward" };
const PLANE = "/plane";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
  document.body.innerHTML = "";
  forgetOffers();
  OFFERS.forget();
});

/** A pane drawn at `width` × `height`, marked as the window marks one, holding `children`. */
function inPane(pane: number, size: { width: number; height: number }) {
  const element = document.createElement("div");
  element.setAttribute(PANE, String(pane));
  element.setAttribute("data-focused", "");
  element.getBoundingClientRect = () =>
    ({ ...size, left: 0, top: 0, right: size.width, bottom: size.height, x: 0, y: 0 }) as DOMRect;
  document.body.appendChild(element);
  return element;
}

function button(onOpen = vi.fn(), pane = 7) {
  render(
    <ViewOpenButton view={VIEW} name="Statistics · steward" pane={pane} onOpen={onOpen}>
      Statistics
    </ViewOpenButton>,
    { container: document.querySelector(`[${PANE}="${pane}"]`) as HTMLElement },
  );
  return onOpen;
}

describe("a view's button (#4769)", () => {
  it("opens the view in a tab of its own on a press", async () => {
    inPane(7, { width: 1000, height: 600 });
    const onOpen = button();
    await userEvent.click(screen.getByRole("button", { name: "Statistics" }));
    expect(onOpen).toHaveBeenCalledWith("tab");
  });

  it("opens it beside on an ⌥-press", () => {
    inPane(7, { width: 1000, height: 600 });
    const onOpen = button();
    const pressed = screen.getByRole("button", { name: "Statistics" });
    fireEvent.pointerDown(pressed, { altKey: true, pointerType: "mouse", button: 0 });
    fireEvent.pointerUp(pressed, { altKey: true, pointerType: "mouse", button: 0 });
    fireEvent.click(pressed, { altKey: true });
    expect(onOpen).toHaveBeenCalledWith("beside");
  });

  it("opens it beside this pane from the beside icon after it", async () => {
    inPane(7, { width: 1000, height: 600 });
    const onOpen = button();
    await userEvent.click(
      screen.getByRole("button", { name: "Open Statistics · steward beside this pane (⌥-press)" }),
    );
    expect(onOpen).toHaveBeenCalledWith("beside");
  });

  it("greys the beside icon with its reason, before the press, in a pane too narrow", async () => {
    inPane(7, { width: 500, height: 300 });
    const onOpen = button();
    const icon = await screen.findByRole("button", {
      name: "Too narrow to open beside: each side needs 40 columns",
    });
    expect(icon).toHaveAttribute("aria-disabled", "true");
    await userEvent.click(icon);
    expect(onOpen).not.toHaveBeenCalled();
  });

  it("offers Open in a tab and Open beside on its menu, headed by the view's name", async () => {
    inPane(7, { width: 1000, height: 600 });
    const onOpen = button();
    fireEvent.contextMenu(screen.getByRole("button", { name: "Statistics" }));
    const menu = await screen.findByRole("menu", { name: "Statistics · steward" });
    expect(menu).toHaveTextContent("Open in a tab");
    await userEvent.click(screen.getByRole("menuitem", { name: /Open beside/ }));
    expect(onOpen).toHaveBeenCalledWith("beside");
  });
});

/** The window's opening, recording what was asked. */
function opening() {
  const onOpen = vi.fn<OfferOpening["onOpen"]>();
  return { plane: PLANE, declared: [STATS], onOpen } satisfies OfferOpening;
}

const WORLD = { declared: [STATS], open: () => false };

/** How many offers stand in the plane, read as the window reads them. */
function Count() {
  return <output data-testid="standing">{useOffers(PLANE).length}</output>;
}

describe("an extension's offer, as a note (#4769)", () => {
  it("says who offers which view on the band of the chat it is about, and opens it there", async () => {
    inPane(3, { width: 1000, height: 600 });
    const given = opening();
    act(() =>
      heardOffers(
        PLANE,
        [{ extension: "statistics", view: "stats", key: "steward", chat: 4 }],
        WORLD,
      ),
    );
    render(
      <OfferOpeningContext.Provider value={given}>
        <ChatOffers session={4} pane={3} />
        <ChatOffers session={5} pane={9} />
      </OfferOpeningContext.Provider>,
    );
    expect(screen.getAllByText("statistics offers a view · Statistics · steward")).toHaveLength(1);
    await userEvent.click(screen.getByRole("button", { name: "Open beside" }));
    expect(given.onOpen).toHaveBeenCalledWith(VIEW, "Statistics · steward", "beside", 3);
  });

  it("is listed in the Notices centre, and Dismiss keeps it quiet for the session", async () => {
    const given = opening();
    act(() =>
      heardOffers(PLANE, [{ extension: "statistics", view: "stats", key: "steward" }], WORLD),
    );
    render(
      <OfferOpeningContext.Provider value={given}>
        <OfferNotices />
        <Count />
      </OfferOpeningContext.Provider>,
    );
    await userEvent.click(screen.getByRole("button", { name: "Open" }));
    expect(given.onOpen).toHaveBeenCalledWith(VIEW, "Statistics · steward", "tab");
    await userEvent.click(screen.getByRole("button", { name: "Dismiss" }));
    await waitFor(() => expect(screen.getByTestId("standing")).toHaveTextContent("0"));
    act(() =>
      heardOffers(PLANE, [{ extension: "statistics", view: "stats", key: "steward" }], WORLD),
    );
    expect(screen.getByTestId("standing")).toHaveTextContent("0");
  });

  it("stops every offer from that extension until its offers are turned back on", async () => {
    act(() =>
      heardOffers(
        PLANE,
        [
          { extension: "statistics", view: "stats", key: "steward" },
          { extension: "statistics", view: "stats", key: "scout" },
        ],
        WORLD,
      ),
    );
    render(
      <OfferOpeningContext.Provider value={opening()}>
        <OfferNotices />
        <Count />
      </OfferOpeningContext.Provider>,
    );
    await userEvent.click(
      screen.getAllByRole("button", { name: "Stop offers from statistics" })[0],
    );
    await waitFor(() => expect(screen.getByTestId("standing")).toHaveTextContent("0"));
    expect(OFFERS.value()).toEqual(["statistics"]);
    act(() => heardOffers(PLANE, [{ extension: "statistics", view: "stats", key: "lead" }], WORLD));
    expect(screen.getByTestId("standing")).toHaveTextContent("0");
  });
});
