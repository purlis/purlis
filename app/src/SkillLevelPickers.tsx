// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/** **The pickers over a project's skill levels and grants** (#2770): what New skill, Move and a
 *  project skill's Reaches fact choose with, by the core's words. */
import type { SkillGrant, SkillLevel, StudioLevel } from "./bindings";
import { grantChoices, grantLabel, labelOf, picking, sameGrant, sameLevel } from "./skillChanges";
import { Select } from "./ui/select/Select";

/** A picker over the project's levels, by the core's words. */
export function LevelPicker({
  label,
  levels,
  now,
  disabled,
  onPick,
}: {
  label: string;
  levels: readonly StudioLevel[];
  /** The level it holds, or none for a picker that only says what to do (Move to…). */
  now?: SkillLevel;
  disabled?: boolean;
  onPick: (level: SkillLevel) => void;
}) {
  const picker = picking(
    levels.map((one) => one.level),
    (level) => labelOf(levels, level),
    sameLevel,
    now,
  );
  return (
    <Select
      aria-label={label}
      value={picker.value}
      placeholder={now === undefined ? "Move to…" : undefined}
      isDisabled={disabled}
      options={picker.options}
      onValueChange={(to) => {
        const level = picker.chosen(to);
        if (level !== undefined) onPick(level);
      }}
    />
  );
}

/** A picker over the grants a project skill can have. */
export function GrantPicker({
  label,
  workspaces,
  now,
  disabled,
  onPick,
}: {
  label: string;
  workspaces: readonly string[];
  now: SkillGrant;
  disabled?: boolean;
  onPick: (grant: SkillGrant) => void;
}) {
  const picker = picking(grantChoices(workspaces, now), grantLabel, sameGrant, now);
  return (
    <Select
      aria-label={label}
      value={picker.value}
      isDisabled={disabled}
      options={picker.options}
      onValueChange={(to) => {
        const grant = picker.chosen(to);
        if (grant !== undefined) onPick(grant);
      }}
    />
  );
}
