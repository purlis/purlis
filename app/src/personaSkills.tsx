// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useId, useState } from "react";
import { commands, type PersonaSkillChange, type PlaneId } from "./bindings";
import { Notice } from "./Notice";
import { Button } from "./ui/button/Button";
import { Form } from "./ui/field/Field";
import { ComboBox } from "./ui/select/ComboBox";

/**
 * **A persona's skills in the Skill studio** (SK-5, #2768; spec decision 35): the studio filtered
 * to a persona adds a skill to its skills by name, and takes out one its skills name. Every skill
 * choice is the studio's; the persona's page only counts them.
 */

/** Adds `written` to `persona`'s skills, or takes it out: the core's refusal, or nothing. What
 *  Add here and a studio row's Add and Remove do. */
export async function changePersonaSkill(
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

  const add = async () => {
    const name = written.trim();
    if (busy || name === "") return;
    setBusy(true);
    const said = await changePersonaSkill(plane, persona, name, "add");
    setBusy(false);
    setRefused(said);
    if (said === undefined) {
      setWritten("");
      onChanged();
    }
  };

  return (
    <Form
      className="persona-skill-add"
      aria-label={`Add a skill to ${persona}`}
      onSubmit={(event) => {
        event.preventDefault();
        void add();
      }}
    >
      <ComboBox
        id={input}
        label="Add a skill"
        suggestions={known}
        value={written}
        placeholder="Its name, as the Skill studio lists it"
        isDisabled={busy || readOnly !== undefined}
        tooltip={readOnly}
        onChange={setWritten}
        data-testid="persona-skill-name"
      />
      <Button type="submit" isDisabled={busy || readOnly !== undefined || written.trim() === ""}>
        {busy ? "Adding…" : "Add"}
      </Button>
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
    </Form>
  );
}

/* **Remove** (#2768) is one of a studio row's actions (`SkillRowActions`, #2770), beside Add
   for a skill the persona's list does not name: it calls `changePersonaSkill` too. */
