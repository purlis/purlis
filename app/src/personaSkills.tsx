// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useId, useState } from "react";
import { commands, type PersonaSkillChange, type PlaneId } from "./bindings";
import { Notice } from "./Notice";

/**
 * **A persona's skills in the Skill studio** (SK-5, #2768; spec decision 35): the studio filtered
 * to a persona adds a skill to its skills by name, and takes out one its skills name. Every skill
 * choice is the studio's; the persona's page only counts them.
 */

/** Adds `written` to `persona`'s skills, or takes it out: the core's refusal, or nothing. */
async function change(
  plane: PlaneId,
  persona: string,
  written: string,
  how: PersonaSkillChange,
): Promise<string | undefined> {
  const done = await commands
    .personaSkill(plane, persona, written, how)
    .catch((err: unknown) => ({ status: "error" as const, error: String(err) }));
  return done.status === "error" ? done.error : undefined;
}

/**
 * **Add** (#2768): a skill's name, typed or picked from the skills the studio lists, added to the
 * persona's skills. Each is offered as the studio's row writes it, so the window never spells a
 * prefix itself.
 */
export function PersonaSkillAdd({
  plane,
  persona,
  known,
  onChanged,
  readOnly,
}: {
  plane: PlaneId;
  persona: string;
  /** The skills it may name, as the studio's rows write them. */
  known: readonly string[];
  /** The persona's skills changed: the studio is asked again. */
  onChanged: () => void;
  /** Why the project is read-only, which turns Add off. */
  readOnly?: string;
}) {
  const [written, setWritten] = useState("");
  const [busy, setBusy] = useState(false);
  const [refused, setRefused] = useState<string>();
  const input = useId();
  const suggestions = useId();

  const add = async () => {
    const name = written.trim();
    if (busy || name === "") return;
    setBusy(true);
    const said = await change(plane, persona, name, "add");
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
        {[...new Set(known)].map((name) => (
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
          void change(plane, persona, written, "remove").then((said) => {
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
