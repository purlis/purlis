// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The Skill studio at the window seam** (#3681): drawn as its tab draws it, at a place its
 * history was opened at, the core faked at its command boundary by each test's `mockIPC`.
 */
import type { ComponentProps, ReactElement } from "react";
import { render, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { SkillStudioTab } from "./SkillStudioTab";
import {
  AT_THE_ROOT,
  SKILL_STUDIO_VIEW,
  studioHistoryOf,
  type StudioPage,
  type StudioPlace,
  type StudioScope,
} from "./skillStudio";
import { forgetViewHistory, openViewAt } from "./ui/panel/viewHistory";

/**
 * **The handle between the list and the item is kept off where a test presses** (jsdom lays
 * nothing out): `react-resizable-panels` takes any press whose point is within its handle's box
 * as a drag of it, and in jsdom every box is at 0,0, so every press would land on the handle and
 * take the keyboard there. Its box is placed far from every point a test presses.
 */
const drawnAt = HTMLElement.prototype.getBoundingClientRect;
HTMLElement.prototype.getBoundingClientRect = function (this: HTMLElement) {
  if (this.dataset.scope === "panel-separator") return new DOMRect(-10_000, -10_000, 1, 1);
  return drawnAt.call(this);
};

/** The project every studio test draws. */
export const PLANE = "p";

/** The history of the project's studio tab, which a test opens at a place. */
export const STUDIO_HISTORY = studioHistoryOf(PLANE, SKILL_STUDIO_VIEW);

/** **The studio tab, opened at `page` on `scope`'s chats**, as an entry point opens it. */
export function renderStudio(
  scope: StudioScope = AT_THE_ROOT,
  page: StudioPage = "library",
  { narrow }: { narrow?: boolean } = {},
) {
  openViewAt(STUDIO_HISTORY, { page, scope });
  return render(<SkillStudioTab plane={PLANE} changed={0} narrow={narrow} />);
}

/** **The studio tab, opened at `place`** (a skill's page, one of its tabs), as an entry point
 *  opens it; `wrap` draws it inside what the window hands its panes. */
export function renderStudioAt(
  place: StudioPlace,
  {
    narrow,
    wrap = (tab) => tab,
    ...more
  }: {
    narrow?: boolean;
    wrap?: (tab: ReactElement) => ReactElement;
  } & Partial<
    Pick<ComponentProps<typeof SkillStudioTab>, "onOpenView" | "offerFor" | "onPress">
  > = {},
) {
  openViewAt(STUDIO_HISTORY, place);
  return render(wrap(<SkillStudioTab plane={PLANE} changed={0} narrow={narrow} {...more} />));
}

/** Its history goes, as its tab's does when it closes: each test starts the tab afresh. */
export function forgetStudio(): void {
  forgetViewHistory(STUDIO_HISTORY);
}

/**
 * **Chooses the Library's row `testid`**, as a press on it does, and answers the skill's item
 * beside the list once it is drawn.
 */
export async function openSkill(
  testid: string,
  user: ReturnType<typeof userEvent.setup> = userEvent.setup(),
): Promise<HTMLElement> {
  const row = await screen.findByTestId(testid);
  await user.click(row);
  const id = row.getAttribute("data-row");
  return waitFor(() => {
    const item = screen.getByTestId("skill-studio-skill");
    if (item.getAttribute("data-row") !== id) throw new Error(`${testid} is not open yet`);
    return item;
  });
}

/** **Chooses `said` from the "…" menu of the skill `name`'s page** (#3682), as a press on the
 *  menu and then on its row does. */
export async function chooseMore(
  name: string,
  said: string | RegExp,
  user: ReturnType<typeof userEvent.setup> = userEvent.setup(),
): Promise<void> {
  await user.click(screen.getByRole("button", { name: `More for ${name}` }));
  const menu = await screen.findByRole("menu", { name: `More for ${name}` });
  await user.click(within(menu).getByRole("menuitem", { name: said }));
}
