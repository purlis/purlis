// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useLayoutEffect, useRef, useState } from "react";
import type { ReviewClash } from "./bindings";
import { capitalised } from "./chatEnded";
import { Choice, SettingRow } from "./settings/components";
import { nameProblem } from "./SkillChangeDialogs";
import { TextField } from "./ui/field/Field";

/**
 * **The name-taken step** (ADR 0100, amended 2026-11-01): what an install asks of a skill whose
 * name is taken where it lands, and what Bring back asks of a deleted skill whose name belongs
 * to purlis now (#5457). Each such skill waits until the person skips it or confirms a new name,
 * which starts as the core offered it (`<source>-<name>`) and is checked as it is typed; the
 * core checks it again.
 */

/** One skill the step may ask of: `key` tells it apart in the review, and `named` is whether the
 *  person named it (an install brings along others it refers to). */
export type Taking = {
  key: string;
  name: string;
  clash: ReviewClash | null;
  named: boolean;
};

/** What the step says, for the act it is part of. */
export type TakenWords = {
  /** The choice that leaves it out: "Skip it". */
  skip: string;
  /** The choice that names it: "Install it under a new name". */
  rename: string;
  /** What a new name changes, after what holds the name. */
  changes: string;
  /** Why the act cannot be pressed when every skill named is skipped. */
  nothing: string;
};

/** What an install says (ADR 0100, amended 2026-11-01). */
export const INSTALL_WORDS: TakenWords = {
  skip: "Skip it",
  rename: "Install it under a new name",
  changes:
    "A new name changes only its name line and folder: its text stays as the source wrote it.",
  nothing: "It is skipped, so there is nothing to install.",
};

/** What Bring back says (#5457). */
export const BRING_BACK_WORDS: TakenWords = {
  skip: "Leave it in Deleted",
  rename: "Bring it back under a new name",
  changes:
    "A new name changes only its name line and folder. Nothing that names its old name changes: that name means purlis's skill now.",
  nothing: "Each is left in Deleted, so there is nothing to bring back.",
};

/** How the person settled a skill's name: unsettled until they choose. */
type Settling = { how?: "skip" | "rename"; name?: string };

/** The longest name a skill may have, as the core's `name_problem` holds it. */
const MOST_NAME = 64;

/** The new name offered, or what the person typed over it. */
function newName(one: Taking, typed: string | undefined): string {
  return typed ?? one.clash?.offered ?? "";
}

/** **Why `wanted` cannot be `one`'s new name**, as it is typed: `others` are the rest of the
 *  review's names. The core checks again when the act is pressed. */
export function newNameProblem(
  one: Pick<Taking, "name" | "clash">,
  wanted: string,
  others: readonly string[],
): string | undefined {
  const name = wanted.trim();
  if (name === "") return "Give it a name.";
  if (name.length > MOST_NAME) return `Use at most ${MOST_NAME} characters.`;
  return nameProblem(name, "", [one.name, ...(one.clash?.taken ?? []), ...others]);
}

/** **A skill's new name**, as the person confirms it: filled in as the core offered it, refused
 *  as it is typed. */
export function NewNameField({
  one,
  value,
  others,
  busy,
  onChange,
}: {
  one: Pick<Taking, "name" | "clash">;
  value: string;
  others: readonly string[];
  busy: boolean;
  onChange: (name: string) => void;
}) {
  return (
    <TextField
      label={`New name for ${one.name}`}
      value={value}
      isDisabled={busy}
      onChange={onChange}
      error={newNameProblem(one, value, others)}
      data-testid={`new-name-${one.name}`}
    />
  );
}

/**
 * **The choices a review's taken names wait for**: each unsettled until the person skips it or
 * names it, the new names the act hands the core, and why the act cannot be pressed yet.
 * `onSkip`, where given, takes a skipped skill out of the review rather than remembering it.
 */
export function useNameTaken(
  all: readonly Taking[],
  words: TakenWords,
  onSkip?: (key: string) => void,
) {
  const [settled, setSettled] = useState<Readonly<Record<string, Settling>>>({});
  const clashing = all.filter((one) => one.clash !== null);
  const skipped = clashing.filter((one) => settled[one.key]?.how === "skip");
  // What a skipped skill alone brings along is not installed, so naming none leaves nothing.
  const named = all.filter((one) => one.named);
  const nothing = named.length > 0 && named.every((one) => skipped.includes(one));
  const renaming = clashing.filter((one) => settled[one.key]?.how === "rename");
  const open = clashing.filter((one) => settled[one.key]?.how === undefined);
  const nameOf = (one: Taking) => newName(one, settled[one.key]?.name).trim();
  const others = (one: Taking) => all.filter((other) => other !== one).map((other) => other.name);
  const bad = renaming.some(
    (one) => newNameProblem(one, newName(one, settled[one.key]?.name), others(one)) !== undefined,
  );
  const wanted = renaming.map(nameOf);
  const twice = wanted.some((name, at) => wanted.indexOf(name) !== at);
  const settle = (key: string, change: Settling) =>
    setSettled((was) => ({ ...was, [key]: { ...was[key], ...change } }));
  return {
    /** The keys of the skills skipped. */
    skipped: new Set(skipped.map((one) => one.key)),
    renames: renaming.map((one) => ({ key: one.key, to: nameOf(one) })),
    why:
      open.length > 0
        ? open.length === 1
          ? `Choose what to do with ${open[0]?.name}: its name is taken.`
          : `Choose what to do with the ${open.length} skills whose names are taken.`
        : bad
          ? "Fix the new name."
          : twice
            ? "Two skills cannot have the same name."
            : nothing
              ? words.nothing
              : undefined,
    choice: (one: Taking, busy: boolean) =>
      one.clash === null ? undefined : (
        <NameTakenChoice
          one={one}
          words={words}
          settling={settled[one.key]}
          others={others(one)}
          busy={busy}
          onSettle={(change) => {
            // A skip that takes it out of the review settles nothing to remember: ticked again,
            // it asks again.
            if (change.how === "skip" && onSkip !== undefined) onSkip(one.key);
            else settle(one.key, change);
          }}
        />
      ),
  };
}

/** **A name that is taken**: what holds it, and the two ways on, in the skill's own review. */
function NameTakenChoice({
  one,
  words,
  settling,
  others,
  busy,
  onSettle,
}: {
  one: Taking;
  words: TakenWords;
  settling: Settling | undefined;
  others: readonly string[];
  busy: boolean;
  onSettle: (change: Settling) => void;
}) {
  const renaming = settling?.how === "rename";
  const field = useRef<HTMLDivElement>(null);
  // What the choice reveals comes into sight as it appears, so the next Tab, onto the field,
  // lands where it can be seen: never under a review's bar (WCAG 2.4.11), which WebKit would
  // otherwise leave it under.
  useLayoutEffect(() => {
    if (renaming) field.current?.scrollIntoView?.({ block: "nearest" });
  }, [renaming]);
  return (
    <div className="skill-clash" data-testid={`clash-${one.name}`}>
      <SettingRow
        label="This name is taken"
        grouped
        help={`${capitalised(one.clash?.held_by ?? "")}. ${words.changes}`}
        control={(ids) => (
          <Choice
            kind="radio"
            ids={ids}
            options={[
              { value: "skip", label: words.skip },
              { value: "rename", label: words.rename },
            ]}
            // Unsettled is no value at all: one no radio holds would leave every radio out of
            // the Tab order, as a group's Tab stop is the radio chosen.
            value={settling?.how}
            disabled={busy}
            onValueChange={(to) => onSettle({ how: to === "skip" ? "skip" : "rename" })}
          />
        )}
      />
      {renaming && (
        <div ref={field}>
          <NewNameField
            one={one}
            value={newName(one, settling.name)}
            others={others}
            busy={busy}
            onChange={(name) => onSettle({ name })}
          />
        </div>
      )}
    </div>
  );
}
