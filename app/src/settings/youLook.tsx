// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useState } from "react";
import { setColours, setMinimumContrast, setReduceMotion, useAppearance } from "../appearance";
import { Choice, Field } from "./components";
import type { Setting } from "./groups";

/**
 * **Reduce motion, minimum contrast and the person's own colours** (#2925, ruling #2814·7·8):
 * settings every theme honours, kept on this machine in the layout file's `appearance`
 * (`appearance.ts`). Each is layered on the system's own: purlis's reduce motion and more
 * contrast add to the system's, and never take it away.
 */

/** Whether every motion is off, whatever the system says. */
export const reduceMotion: Setting = {
  id: "you.motion.reduce",
  label: "Reduce motion",
  help: "Turns every motion in the window off: transitions, slides, fades and spinners are drawn at rest. Your system's reduce motion turns it off too.",
  useControl: function useReduceMotion() {
    const reduced = useAppearance().motion === "reduced";
    return {
      control: (ids) => (
        <Choice kind="switch" ids={ids} checked={reduced} onCheckedChange={setReduceMotion} />
      ),
    };
  },
};

/** The contrast purlis holds, at least, in every theme. */
export const minimumContrast: Setting = {
  id: "you.motion.contrast",
  label: "Minimum contrast",
  help: "More draws a theme's high-contrast appearance if it has one, and raises the contrast purlis holds: words to 7:1 and marks to 4.5:1, by drawing their colours lighter or darker. Your system's increase contrast draws the high-contrast appearance too.",
  useControl: function useMinimumContrast() {
    const chosen = useAppearance().contrast ?? "standard";
    return {
      control: (ids) => (
        <Choice
          kind="radio"
          ids={ids}
          options={[
            { value: "standard", label: "The theme's", says: "Words at 4.5:1, marks at 3:1." },
            { value: "more", label: "More", says: "Words at 7:1, marks at 4.5:1." },
          ]}
          value={chosen}
          onValueChange={(to) => setMinimumContrast(to === "more" ? "more" : "standard")}
        />
      ),
      grouped: true,
    };
  },
};

/** The person's colours as the box shows them: one `token #colour` a line, in token order. */
export function colourLines(colours: Readonly<Record<string, string>> | undefined): string {
  return Object.entries(colours ?? {})
    .sort(([a], [b]) => a.localeCompare(b))
    .map(([token, colour]) => `${token} ${colour}`)
    .join("\n");
}

/** What the box holds, as the person's colours: each line a token and a colour; a line with a
 *  token alone, or nothing, sets nothing. The engine judges each token and colour. */
export function coloursOf(text: string): Record<string, string> {
  const colours: Record<string, string> = {};
  for (const line of text.split("\n")) {
    const [token, colour] = line.trim().split(/\s+/);
    if (token && colour) colours[token] = colour;
  }
  return colours;
}

/** Each colour the person sets over every theme. */
export const yourColours: Setting = {
  id: "you.colours.tokens",
  label: "Colours",
  help: "One a line: a theme token and its colour, such as accent.base #8f7bff. Each is drawn over every theme. A colour that would make words or marks hard to read in a theme is left out there, and a Notice says which.",
  useControl: function useYourColours() {
    const set = useAppearance().colours;
    const shown = colourLines(set);
    const [typed, setTyped] = useState<string | undefined>(undefined);
    const commit = () => {
      if (typed === undefined) return;
      const next = coloursOf(typed);
      if (colourLines(next) !== shown) setColours(next);
      setTyped(undefined);
    };
    return {
      control: (ids) => (
        <Field kind="list" ids={ids} value={typed ?? shown} onChange={setTyped} onCommit={commit} />
      ),
      reset: {
        label: "Use the theme's",
        disabled: shown === "",
        onReset: () => setColours({}),
      },
    };
  },
};
