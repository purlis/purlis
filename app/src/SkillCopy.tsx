// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useState } from "react";
import type { SkillLevel, StudioLevel } from "./bindings";
import { Field, SettingRow } from "./settings/components";
import { labelOf, picking, sameLevel } from "./skillChanges";
import { Dialog } from "./ui/dialog/Dialog";
import { Select } from "./ui/select/Select";

/** What Make my own copy is called, wherever it is offered (#3686). */
export const MAKE_MY_OWN_COPY = "Make my own copy";

/** What the folded built-in skills' closing line says of them (S12): the studio's and the
 *  Skills view's. */
export const BUILT_IN_FIXED =
  "They ship with purlis: they can't be edited, turned off or replaced.";

/** What a refused name is told, the core's own sentence (`skills::NAME_RESERVED`), said here
 *  before the copy is asked for. */
export const NAME_RESERVED = "That name belongs to a built-in skill; pick another, or make a copy.";

/** What the copy's name starts as: the built-in's, made the person's. */
export function copyNameOf(name: string): string {
  return `my-${name}`;
}

/**
 * **Make my own copy** (#3686, spec #3679 S12): a built-in skill copied to a skill level the
 * person picks, under a new name, as a skill of their own to edit. The built-in skill stays as it
 * is, and chats keep getting it beside the copy: a built-in skill is never replaced.
 *
 * A dialog, as a short choice is (S2): which built-in (where it was opened from the folded line,
 * not from one built-in's page), the new name, and where it lands. A name a built-in skill has is
 * refused as it is typed, in the core's words, and the core refuses it again when asked; what
 * the core refuses is said in place, beside the fields the person is still filling.
 */
export function SkillCopy({
  builtIns,
  from,
  levels,
  start,
  busy,
  refused,
  onEdit,
  onCopy,
  onCancel,
}: {
  /** Every built-in skill's name, in its order: what a copy can be made of, and the names it
   *  cannot take. */
  builtIns: readonly string[];
  /** The built-in it copies, where it was opened on one; else the person picks. */
  from?: string;
  /** The levels the copy can land at, by the core's words. */
  levels: readonly StudioLevel[];
  /** Where it lands unless the person picks another: the scope's own level. */
  start: SkillLevel;
  busy: boolean;
  /** Why the core did not make the last copy asked for: its sentence, unchanged. */
  refused?: string;
  /** A field changed: a refusal of what was asked before no longer stands beside it. */
  onEdit: () => void;
  onCopy: (name: string, level: SkillLevel, to: string) => void;
  onCancel: () => void;
}) {
  const [name, setName] = useState(from ?? builtIns[0] ?? "");
  const [to, setTo] = useState(copyNameOf(from ?? builtIns[0] ?? ""));
  const [toTyped, setToTyped] = useState(false);
  const [level, setLevel] = useState<SkillLevel>(start);
  const wanted = to.trim();
  const reserved = builtIns.includes(wanted);
  const ready = !busy && name !== "" && wanted !== "" && !reserved;
  const levelPicker = picking(
    levels.map((one) => one.level),
    (one) => labelOf(levels, one),
    sameLevel,
    level,
  );
  return (
    <Dialog
      onClose={onCancel}
      title={
        from === undefined
          ? `${MAKE_MY_OWN_COPY} of a built-in skill`
          : `${MAKE_MY_OWN_COPY} of ${from}`
      }
      titleId="skill-copy"
      description={`Your copy is a skill of your own, to edit as you like. ${name === "" ? "The built-in skill" : name} stays as it is, and chats keep getting it beside your copy.`}
      summary={
        name === "" || wanted === ""
          ? undefined
          : `${wanted} is made in the ${labelOf(levels, level)} skills, in one commit.`
      }
      main={{
        children: busy ? "Copying…" : "Make the copy",
        reserve: ["Make the copy", "Copying…"],
        isDisabled: !ready,
        onPress: () => {
          if (ready) onCopy(name, level, wanted);
        },
      }}
      data-testid="skill-copy"
    >
      {from === undefined && (
        <SettingRow
          label="Copy of"
          control={(ids) => (
            <Select
              id={ids.id}
              aria-labelledby={ids.labelledBy}
              value={name}
              options={builtIns.map((one) => ({ value: one, label: one }))}
              onValueChange={(next) => {
                setName(next);
                if (!toTyped) setTo(copyNameOf(next));
                onEdit();
              }}
            />
          )}
        />
      )}
      <SettingRow
        label="New name"
        help="Lowercase letters, digits and hyphens: not a built-in skill's name."
        control={(ids) => (
          <Field
            kind="text"
            ids={ids}
            value={to}
            onChange={(next) => {
              setTo(next);
              setToTyped(true);
              onEdit();
            }}
          />
        )}
      />
      {reserved && (
        <p className="trouble" role="alert" data-testid="skill-copy-reserved">
          {NAME_RESERVED}
        </p>
      )}
      <SettingRow
        label="Lands in"
        control={(ids) => (
          <Select
            id={ids.id}
            aria-labelledby={ids.labelledBy}
            value={levelPicker.value}
            options={levelPicker.options}
            onValueChange={(next) => {
              const picked = levelPicker.chosen(next);
              if (picked !== undefined) setLevel(picked);
              onEdit();
            }}
          />
        )}
      />
      {/* Verbatim, beside the fields: the person is still answering. */}
      {refused !== undefined && (
        <p className="trouble" role="alert">
          {refused}
        </p>
      )}
    </Dialog>
  );
}
