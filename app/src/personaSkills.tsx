// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useId, useState } from "react";
import { commands, type PanelBlock, type PanelRow, type PlaneId } from "./bindings";
import { Notice } from "./Notice";
import { skillStudioAs } from "./skillStudio";

/**
 * **A persona's Skills section, as the window lends it its controls** (SK-5, #2768). The core
 * draws the section as panel blocks (`app/src-tauri/src/personaskills.rs`): a count, and a list
 * whose every row opens the Skill studio on the persona's chats. The window adds what a block
 * cannot carry, as it adds a change's Land: an **Add** above the list, which names one more skill
 * in the persona's `skills:` list, and a **Remove** beside each row that list names. A persona's
 * own skill has no Remove: deleting one is the studio's.
 */

/** The key prefix of a row the persona's `skills:` list names; the rest is the entry. */
export const NAMED_SKILL_ROW = "named:";

/** Whether `block` is `persona`'s Skills list: the list whose empty state offers its studio. */
export function isPersonaSkills(block: PanelBlock, persona: string): boolean {
  return block.kind === "list" && block.empty.offer === skillStudioAs(persona);
}

/** The entry of the persona's `skills:` list that `row` is, or `undefined` for its own skill. */
export function namedOf(row: PanelRow): string | undefined {
  return row.key.startsWith(NAMED_SKILL_ROW) ? row.key.slice(NAMED_SKILL_ROW.length) : undefined;
}

/** Adds `written` to `persona`'s skills, or takes it out: the core's refusal, or nothing. */
async function change(
  plane: PlaneId,
  persona: string,
  written: string,
  add: boolean,
): Promise<string | undefined> {
  const done = await commands
    .personaSkill(plane, persona, written, add)
    .catch((err: unknown) => ({ status: "error" as const, error: String(err) }));
  return done.status === "error" ? done.error : undefined;
}

/**
 * **Add** (#2768): a skill's name, typed or picked from the skills the studio lists for a chat as
 * the persona, added to its `skills:` list. `purlis:name` names a built-in skill and
 * `plugin:name` one an installed plugin brings.
 */
export function PersonaSkillAdd({
  plane,
  persona,
  changed,
  onChanged,
  readOnly,
}: {
  plane: PlaneId;
  persona: string;
  changed: number;
  /** The list changed: the persona's view is asked again. */
  onChanged: () => void;
  /** Why the project is read-only, which turns Add off. */
  readOnly?: string;
}) {
  const [written, setWritten] = useState("");
  const [busy, setBusy] = useState(false);
  const [refused, setRefused] = useState<string>();
  const [known, setKnown] = useState<string[]>([]);
  const input = useId();
  const suggestions = useId();
  useEffect(() => {
    let gone = false;
    void commands
      .skillStudio(plane, null, persona)
      .catch(() => undefined)
      .then((answer) => {
        if (gone || answer?.status !== "ok") return;
        setKnown([
          ...new Set(
            answer.data.skills.map((skill) =>
              skill.level === "built-in" ? `purlis:${skill.name}` : skill.name,
            ),
          ),
        ]);
      });
    return () => {
      gone = true;
    };
  }, [plane, persona, changed]);

  const add = async () => {
    const name = written.trim();
    if (busy || name === "") return;
    setBusy(true);
    const said = await change(plane, persona, name, true);
    setBusy(false);
    setRefused(said);
    if (said === undefined) {
      setWritten("");
      onChanged();
    }
  };

  return (
    <form
      className="persona-skill-add"
      aria-label={`Add a skill to ${persona}`}
      onSubmit={(event) => {
        event.preventDefault();
        void add();
      }}
    >
      <label htmlFor={input}>Add a skill</label>
      <input
        id={input}
        className="ui-input"
        list={suggestions}
        value={written}
        placeholder="Its name, as the Skill studio lists it"
        disabled={busy || readOnly !== undefined}
        title={readOnly}
        onChange={(event) => setWritten(event.target.value)}
        data-testid="persona-skill-name"
      />
      <datalist id={suggestions}>
        {known.map((name) => (
          <option key={name} value={name} />
        ))}
      </datalist>
      <button
        type="submit"
        className="ui-button"
        tabIndex={0}
        disabled={busy || readOnly !== undefined || written.trim() === ""}
      >
        {busy ? "Adding…" : "Add"}
      </button>
      {refused !== undefined && (
        <Notice
          cause={`persona-skill-add:${persona}`}
          tone="trouble"
          label="The skill was not added"
          onDismiss={() => setRefused(undefined)}
        >
          {refused}
        </Notice>
      )}
    </form>
  );
}

/** **Remove** (#2768): takes `written` out of `persona`'s `skills:` list. */
export function PersonaSkillRemove({
  plane,
  persona,
  written,
  onChanged,
  readOnly,
}: {
  plane: PlaneId;
  persona: string;
  written: string;
  onChanged: () => void;
  readOnly?: string;
}) {
  const [busy, setBusy] = useState(false);
  const [refused, setRefused] = useState<string>();
  return (
    <>
      <button
        type="button"
        className="ui-button persona-skill-remove"
        tabIndex={0}
        aria-label={`Remove ${written} from ${persona}`}
        disabled={busy || readOnly !== undefined}
        title={readOnly}
        onClick={() => {
          setBusy(true);
          void change(plane, persona, written, false).then((said) => {
            setBusy(false);
            setRefused(said);
            if (said === undefined) onChanged();
          });
        }}
      >
        {busy ? "Removing…" : "Remove"}
      </button>
      {refused !== undefined && (
        <Notice
          cause={`persona-skill-remove:${persona}:${written}`}
          tone="trouble"
          label="The skill was not removed"
          onDismiss={() => setRefused(undefined)}
        >
          {refused}
        </Notice>
      )}
    </>
  );
}
