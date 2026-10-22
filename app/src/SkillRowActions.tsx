// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import type { ReactNode } from "react";
import { Button, ButtonRow } from "./ui/button/Button";

/** One thing a person can do to a skill from its row: a word, and what it does. */
export type SkillRowAction = {
  /** Its key among the row's actions. */
  id: string;
  /** What its button says: `Edit`. */
  label: string;
  /** What a screen reader hears: `Edit deploy`. */
  name: string;
  onPress: () => void;
  disabled?: boolean;
  /** Drawn as the row's destructive action. */
  danger?: boolean;
};

/**
 * **The one place a Skill studio row's actions are drawn** (SK-7, #2770): each scope hands in its
 * actions in the order they read, and any control that is not a button (Move to's level picker)
 * as `extra`, drawn before the destructive one. A later control (SK-5's Add and Remove of a
 * persona's skill references) is another action here, never a second pattern.
 */
export function SkillRowActions({
  actions,
  extra,
}: {
  actions: readonly SkillRowAction[];
  extra?: ReactNode;
}) {
  if (actions.length === 0 && extra === undefined) return null;
  const plain = actions.filter((one) => one.danger !== true);
  const danger = actions.filter((one) => one.danger === true);
  const button = (one: SkillRowAction) => (
    <Button
      key={one.id}
      variant={one.danger === true ? "danger" : "secondary"}
      aria-label={one.name}
      isDisabled={one.disabled}
      onPress={one.onPress}
    >
      {one.label}
    </Button>
  );
  return (
    <ButtonRow size="sm" data-testid="skill-row-actions">
      {plain.map(button)}
      {extra}
      {danger.map(button)}
    </ButtonRow>
  );
}
