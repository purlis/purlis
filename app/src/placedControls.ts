// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { askSavingTab } from "./saving";
import { AT_THE_ROOT, askSkillStudio } from "./skillStudio";
import type { SettingsLink } from "./settings/links";
import { ASK_WAITS_MS } from "./windowControls";

/**
 * **A message that names a control in a view or in Settings links to it, focused** (R-357,
 * T1-48; Q234). `windowControls.ts` holds the controls that stand on a Notice; these stand in a
 * view (the Saving view, the Skill studio's Needs you) or in a row of Settings. Where the window
 * shows a result whose words name one, it carries **Go to** it: the view or the Settings group
 * opens, with the keyboard on the control.
 *
 * `windowControls.test.ts` counts these as linked, so the core messages that name them are no
 * longer held in its list of those that link nowhere.
 */
export type PlacedControl = {
  /** The control's label, as it is drawn and as the core names it. */
  words: string;
  /** How a message names it: the label where it alone would match ordinary prose ("Remove"). */
  names: RegExp;
  /** What its Go to says it goes to. */
  goTo: string;
  /** How its accessible name starts, where that is more than its label ("Remove pool"). */
  spoken?: string;
  /** Which one of several it is, from the message's words: the profile or pool it names. */
  about?: RegExp;
  /** Where it stands. */
  at: { settings: SettingsLink } | { view: "saving" } | { view: "skill-studio"; need: string };
};

/** Settings › You › This machine › Pools' list, where each pool and its profiles' lines stand. */
const POOLS_LIST: SettingsLink = { group: "you.machine.pools", setting: "you.machine.pools.list" };

export const PLACED_CONTROLS: readonly PlacedControl[] = [
  {
    // `planegit/update.rs`'s TO_RESOLVE and `planegit/bringin.rs`. Not a resolve's "Resolve
    // again": that is said while the resolve is open, where Abort stands in Resolve's place.
    words: "Resolve",
    names: /\bResolve in the Saving view\b/,
    goTo: "Resolve",
    at: { view: "saving" },
  },
  {
    // `profilestore/pin.rs`: a chat that does not start on the profile's pinned version.
    words: "Update the pin",
    names: /\bUpdate the pin\b/,
    goTo: "Update the pin",
    about: /\bprofile '([^']+)'/,
    at: { settings: { group: "you.profiles", setting: "you.profiles.list" } },
  },
  {
    // `skills/levels.rs`'s UNCONFIRMED: Needs you's item for the project's folders
    // (`skillstudio/needs.rs`).
    words: "Accept the folders",
    names: /\bAccept the folders\b/,
    goTo: "Accept the folders",
    at: { view: "skill-studio", need: "review:project-folders" },
  },
  {
    // `poolsetup/signins.rs`: a profile a pool that names none keeps.
    words: "Remove",
    names: /\bRemove on the pool\b/,
    goTo: "Remove on the pool",
    spoken: "Remove pool",
    about: /\b[Pp]ool (\S+) names none\b/,
    at: { settings: POOLS_LIST },
  },
];

/** The words of the way to `control` on a message that names it. */
export const goToPlacedSays = (control: PlacedControl): string => `Go to ${control.goTo}`;

/** The controls `message` names, in the list's order. */
export const placedControlsNamedIn = (message: string): PlacedControl[] =>
  PLACED_CONTROLS.filter((control) => control.names.test(message));

/**
 * **Goes to `control`, as the message `message` named it**: Settings through `onSettings`,
 * landing on the control itself (of the profile or pool the message names); a view through its
 * window's ask, then the keyboard on the control once the view draws it.
 */
export function goToPlaced(
  control: PlacedControl,
  project: string | undefined,
  message: string,
  onSettings: (link: SettingsLink) => void,
): void {
  const at = control.at;
  if ("settings" in at) {
    const about = control.about?.exec(message)?.[1];
    onSettings({ ...at.settings, control: { words: control.spoken ?? control.words, about } });
    return;
  }
  if (project === undefined) return;
  if (at.view === "saving") askSavingTab(project);
  else askSkillStudio(project, AT_THE_ROOT, "needs", at.need);
  reach(control.words, project);
}

/** The element the project `project` draws its view into (`projectShown.useProjectHost`). */
function viewOf(project: string): HTMLElement | undefined {
  return [...document.querySelectorAll<HTMLElement>(".project-view")].find(
    (one) => one.dataset.plane === project,
  );
}

/** How many frames in a row the control must keep the keyboard before the reach is done: the
 *  window gives a view it opened the keyboard a frame after it draws it. */
const HELD_FOR = 3;

/**
 * **Puts the keyboard on the button that says `words`** once the project `project`'s view draws
 * it (another project's same button is not it), and back on it
 * while the view that opened takes it, until it has held it for {@link HELD_FOR} frames. The
 * person's own click or key, or {@link ASK_WAITS_MS} without it, ends it.
 */
export function reach(words: string, project: string): void {
  const until = Date.now() + ASK_WAITS_MS;
  let held = 0;
  let frame = 0;
  const end = () => {
    cancelAnimationFrame(frame);
    for (const type of ["pointerdown", "keydown"]) window.removeEventListener(type, end, true);
  };
  const step = () => {
    if (Date.now() > until) return end();
    const view = viewOf(project);
    const buttons = view?.querySelectorAll<HTMLElement>('button, [role="button"]') ?? [];
    const control = [...buttons].find(
      (one) => one.textContent?.trim() === words && one.getAttribute("aria-disabled") !== "true",
    );
    if (control === undefined) held = 0;
    else if (document.activeElement === control) held += 1;
    else {
      held = 0;
      control.focus();
    }
    if (held >= HELD_FOR) return end();
    frame = requestAnimationFrame(step);
  };
  // Added in the press that asked, past its own capture: that press does not end it.
  for (const type of ["pointerdown", "keydown"]) window.addEventListener(type, end, true);
  frame = requestAnimationFrame(step);
}
