// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useState } from "react";
import type { WindowOption } from "../bindings";
import { themeOptionsInForce } from "../Extensions";
import { onDrawn } from "../theme/theme";
import type { Badge, RowIds } from "./components";

/**
 * **A theme's options** (#2325), as Settings › Theme shows them (#2331): each option, the value
 * it is drawn with, and a badge saying where that value comes from — the theme's default, or a
 * choice. The core's theme engine answers (`theme_in_force` for the theme drawn, the gallery's
 * card for another), and the list only shows it.
 */
/** The options of the theme in force, asked again whenever the window draws another theme. */
export function useOptionsInForce(): readonly WindowOption[] | undefined {
  const [options, setOptions] = useState<readonly WindowOption[]>();
  useEffect(() => {
    let live = true;
    const ask = () => void themeOptionsInForce().then((given) => live && setOptions(given));
    ask();
    const stop = onDrawn(ask);
    return () => {
      live = false;
      stop();
    };
  }, []);
  return options;
}

/** Where an option's value comes from, as its badge says it. */
export function optionBadge(option: WindowOption): Badge {
  return option.chosen
    ? { label: "Chosen", says: `${option.label} is drawn as it was chosen for this theme.` }
    : {
        label: "Theme default",
        says: `Nothing chose a value for ${option.label}, so the theme's default is drawn.`,
      };
}

/** An option's badge: the level's word, with its sentence as the badge's title and its
 *  accessible description. */
function OptionBadge({ badge }: { badge: Badge }) {
  return (
    <span className="ui-setting-badge" title={badge.says} aria-description={badge.says}>
      {badge.label}
    </span>
  );
}

/** What an option's value is called: its value's label, or its id when the theme gives none. */
function valueLabel(option: WindowOption): string {
  return option.values.find((one) => one.id === option.value)?.label ?? option.value;
}

export function OptionsList({
  options,
  ids,
  none = "The theme drawn here has no options.",
}: {
  options: readonly WindowOption[] | undefined;
  ids: RowIds;
  /** What the list says of a theme with none. */
  none?: string;
}) {
  const said =
    options === undefined
      ? "Reading the theme's options…"
      : options.length === 0
        ? none
        : undefined;
  return (
    <div
      id={ids.id}
      role="group"
      className="ui-setting-status"
      aria-labelledby={ids.labelledBy}
      aria-describedby={ids.describedBy}
    >
      {said !== undefined && <p className="ui-setting-help">{said}</p>}
      {options?.map((option) => (
        <p key={option.id} className="ui-setting-help" title={option.description || undefined}>
          {option.label}: {valueLabel(option)} <OptionBadge badge={optionBadge(option)} />
        </p>
      ))}
    </div>
  );
}
