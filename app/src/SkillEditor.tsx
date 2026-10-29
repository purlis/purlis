// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { Button, ButtonRow } from "./ui/button/Button";
import type { SkillLintFinding } from "./bindings";
import { TextEditor } from "./editor/TextEditor";

/**
 * **A skill in the light editor** (SK-7, #2770, decision 14), its page's Text tab made the editor
 * in place (#3683, S4): its `SKILL.md`, typed into, with a live lint under it, the core's own
 * (`skill_lint`, asked by its owner a moment after each edit): its frontmatter, its description,
 * the skills it names and the programs it calls that this machine lacks (SK-8, #2771). Save is
 * offered only while the text changed and the lint finds nothing that blocks it; a warning is
 * shown and never waited on. The save itself is taken only while the file is still what the
 * editor opened, and is one commit. Discard drops what was typed.
 *
 * **The text is its owner's** (`text`, `onChange`): the studio keeps it by its tab, so it is
 * still here when the tab comes back to the front (`skillEdit.ts`, `SkillEditInPlace.tsx`).
 *
 * Its own chunk, as every editor of a file is (`tools/main-chunk-has-no-codemirror.mjs`): the
 * studio loads it the first time a skill is opened.
 */
export default function SkillEditor({
  name,
  was,
  text,
  lint,
  saving,
  onChange,
  onSave,
  onDiscard,
}: {
  name: string;
  /** The text the editor opened. */
  was: string;
  /** The text as typed now. */
  text: string;
  /** What the core's lint finds in `text`. */
  lint: readonly SkillLintFinding[];
  saving: boolean;
  onChange: (text: string) => void;
  onSave: () => void;
  onDiscard: () => void;
}) {
  const unchanged = text === was;
  const blocked = lint.some((finding) => finding.level === "blocks");
  return (
    <section className="skill-editor" aria-label={`Edit ${name}`} data-testid="skill-editor">
      <TextEditor path="SKILL.md" label={`${name}'s instructions`} text={text} onChange={onChange} />
      {lint.length === 0 ? (
        <p className="honest" data-testid="skill-lint">
          It is a whole skill.
        </p>
      ) : (
        <ul className="skill-lint" data-testid="skill-lint" aria-label="What the lint found">
          {lint.map((finding) => (
            <li key={finding.says} data-level={finding.level}>
              {finding.level === "warns" ? `Worth knowing: ${finding.says}` : finding.says}
            </li>
          ))}
        </ul>
      )}
      <ButtonRow>
        <Button variant="primary" isDisabled={saving || unchanged || blocked} onPress={onSave}>
          Save
        </Button>
        <Button isDisabled={saving || unchanged} onPress={onDiscard}>
          Discard
        </Button>
      </ButtonRow>
      <p className="honest">Saving makes a new version, which chats started after it get.</p>
    </section>
  );
}
