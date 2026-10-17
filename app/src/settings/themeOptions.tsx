// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useState } from "react";
import type { WindowOption } from "../bindings";
import { themeOptionsInForce } from "../Extensions";
import { onDrawn } from "../theme/theme";
import { OriginBadge, type Badge, type RowIds } from "./components";
import type { LiveSetting } from "./groups";

/**
 * **The theme's options, in either Appearance group** (#2325): each option of the theme the
 * window draws, the value it is drawn with, and a badge saying where that value comes from —
 * the theme's default, or a choice. The core's theme engine answers (`theme_in_force`), and the
 * row only shows it: the values follow the appearance's precedence (spec #2313), and the
 * person's choices are written where the appearance's are.
 */
export function themeOptionsRow(group: string): LiveSetting {
  return {
    id: `${group}.options`,
    label: "Theme options",
    writes: "nothing",
    help: "The options of the theme drawn here, each with the value it is drawn with and where that value comes from.",
    useControl: function useThemeOptions() {
      const options = useOptionsInForce();
      return {
        grouped: true,
        control: (ids) => <OptionsList options={options} ids={ids} />,
      };
    },
  };
}

/** The options of the theme in force, asked again whenever the window draws another theme. */
function useOptionsInForce(): readonly WindowOption[] | undefined {
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

/** What an option's value is called: its value's label, or its id when the theme gives none. */
function valueLabel(option: WindowOption): string {
  return option.values.find((one) => one.id === option.value)?.label ?? option.value;
}

function OptionsList({
  options,
  ids,
}: {
  options: readonly WindowOption[] | undefined;
  ids: RowIds;
}) {
  const said =
    options === undefined
      ? "Reading the theme's options…"
      : options.length === 0
        ? "The theme drawn here has no options."
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
          {option.label}: {valueLabel(option)} <OriginBadge badge={optionBadge(option)} />
        </p>
      ))}
    </div>
  );
}
