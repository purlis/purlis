// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useState } from "react";
import type { WindowOption } from "../bindings";
import { themeOptionsInForce } from "../Extensions";
import { onDrawn } from "../theme/theme";
import { Choice, type Badge, type RowIds } from "./components";
import { Tooltip } from "../ui/tooltip/Tooltip";
import { Badge as LevelBadge } from "../ui/badge/Badge";

/**
 * **A theme's options** (#2325), as Settings › Theme shows them (#2331): each option, the value
 * it is drawn with, and a badge saying where that value comes from — the theme's default, or the
 * level whose choice made it (#2527). The core's theme engine answers (`theme_in_force` for the
 * theme drawn, the gallery for the chosen theme), and the list shows it; handed `onChoose`, each
 * option's values are the person's to pick.
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

/** The level whose choice made a value, as its badge names it, and where that choice is kept:
 *  the person's on this machine, or a committed default the team shares (#2707). */
const LEVELS: Record<string, { label: string; where: string }> = {
  "workspace-choice": {
    label: "Workspace",
    where: "your choice for this workspace on this machine",
  },
  "workspace-default": { label: "Team", where: "this workspace's committed default" },
  "project-choice": { label: "Project", where: "your choice for this project on this machine" },
  "project-default": { label: "Team", where: "this project's committed default" },
  "machine-default": { label: "This machine", where: "your machine default on this machine" },
};

/** Where an option's value comes from, as its badge says it: the level whose choice made it,
 *  or the theme's default. */
export function optionBadge(option: WindowOption): Badge {
  if (!option.chosen)
    return {
      label: "Theme default",
      says: `Nothing chose a value for ${option.label}, so the theme's default is drawn.`,
    };
  const level = option.level === null ? undefined : LEVELS[option.level];
  return level === undefined
    ? { label: "Chosen", says: `${option.label} is drawn as it was chosen for this theme.` }
    : {
        label: level.label,
        says: `${option.label} is drawn as ${level.where} has it.`,
      };
}

/** An option's badge: the level's word, with its sentence as the badge's title and its
 *  accessible description. */
function OptionBadge({ badge }: { badge: Badge }) {
  return (
    <Tooltip tip={badge.says}>
      <LevelBadge variant="accent" aria-description={badge.says}>
        {badge.label}
      </LevelBadge>
    </Tooltip>
  );
}

/** What an option's default is called. */
function defaultLabel(option: WindowOption): string {
  return option.values.find((one) => one.id === option.default)?.label ?? option.default;
}

/** What an option's value is called: its value's label, or its id when the theme gives none. */
function valueLabel(option: WindowOption): string {
  return option.values.find((one) => one.id === option.value)?.label ?? option.value;
}

export function OptionsList({
  options,
  ids,
  none = "The theme drawn here has no options.",
  onChoose,
  disabled,
}: {
  options: readonly WindowOption[] | undefined;
  ids: RowIds;
  /** What the list says of a theme with none. */
  none?: string;
  /** The person picked `value` of `option`, or the theme's default (`null`): each option is
   *  then a choice of its values. */
  onChoose?: (option: WindowOption, value: string | null) => void;
  /** The choices are out of reach while what was picked is kept. */
  disabled?: boolean;
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
      {options?.map((option) =>
        onChoose === undefined ? (
          <Tooltip key={option.id} tip={option.description || undefined}>
            <p className="ui-setting-help">
              {option.label}: {valueLabel(option)} <OptionBadge badge={optionBadge(option)} />
            </p>
          </Tooltip>
        ) : (
          <div key={option.id} className="flex flex-col gap-1">
            <p className="ui-setting-help">
              {/* The group's name is the option's alone; its badge is said as its own. */}
              <span id={`${ids.id}-${option.id}-name`}>{option.label}</span>{" "}
              <OptionBadge badge={optionBadge(option)} />
            </p>
            {option.description && (
              <p id={`${ids.id}-${option.id}-says`} className="ui-setting-help">
                {option.description}
              </p>
            )}
            <Choice
              kind="radio"
              ids={{
                id: `${ids.id}-${option.id}-values`,
                labelledBy: `${ids.id}-${option.id}-name`,
                describedBy: option.description ? `${ids.id}-${option.id}-says` : undefined,
              }}
              disabled={disabled}
              options={[
                // Back to the theme's own: no value chosen here.
                { value: "", label: `Theme default (${defaultLabel(option)})` },
                ...option.values.map((one) => ({ value: one.id, label: one.label })),
              ]}
              value={option.chosen ? option.value : ""}
              onValueChange={(to) => onChoose(option, to === "" ? null : to)}
            />
          </div>
        ),
      )}
    </div>
  );
}
