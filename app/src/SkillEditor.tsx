// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useState } from "react";
import { Button, ButtonRow } from "./ui/button/Button";
import { commands } from "./bindings";
import { TextEditor } from "./editor/TextEditor";

/**
 * **A skill in the light editor** (SK-7, #2770, decision 14): its `SKILL.md`, typed into, with a
 * live lint under it, the core's own (`skill_lint`), asked again a moment after each edit. Save is
 * offered only while the lint finds nothing; the save itself is taken only while the file is
 * still what the editor opened, and is one commit.
 *
 * Its own chunk, as every editor of a file is (`tools/main-chunk-has-no-codemirror.mjs`): the
 * studio loads it the first time a skill is opened.
 */
export default function SkillEditor({
  name,
  was,
  saving,
  onSave,
  onClose,
}: {
  name: string;
  /** The text the editor opened. */
  was: string;
  saving: boolean;
  onSave: (now: string) => void;
  onClose: () => void;
}) {
  const [text, setText] = useState(was);
  const [lint, setLint] = useState<readonly string[]>([]);
  useEffect(() => {
    let live = true;
    const asked = setTimeout(() => {
      void commands
        .skillLint(name, text)
        .then((lines) => {
          if (live) setLint(lines);
        })
        .catch(() => undefined);
    }, 200);
    return () => {
      live = false;
      clearTimeout(asked);
    };
  }, [name, text]);
  const unchanged = text === was;
  return (
    <section className="skill-editor" aria-label={`Edit ${name}`} data-testid="skill-editor">
      <TextEditor path="SKILL.md" label={`${name}'s instructions`} text={text} onChange={setText} />
      {lint.length === 0 ? (
        <p className="honest" data-testid="skill-lint">
          It is a whole skill.
        </p>
      ) : (
        <ul className="skill-lint" data-testid="skill-lint" aria-label="What the lint found">
          {lint.map((line) => (
            <li key={line}>{line}</li>
          ))}
        </ul>
      )}
      <ButtonRow>
        <Button
          variant="primary"
          isDisabled={saving || unchanged || lint.length > 0}
          onClick={() => onSave(text)}
        >
          Save
        </Button>
        <Button onPress={onClose}>Close</Button>
      </ButtonRow>
    </section>
  );
}
