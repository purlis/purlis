// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useState } from "react";
import { commands, type TemplateChoice, type TemplateRow } from "./bindings";
import { DoorPage } from "./DoorPage";
import { type ForgeAsk, ForgeQuestion } from "./ForgeQuestion";
import { Choice, SettingActions, SettingRow } from "./settings/components";
import { Button } from "./ui/button/Button";

/** The radio values that are not a template's id, as the Create page has them. */
const FITS = ":fits";
const NONE = ":none";

function choiceOf(value: string): TemplateChoice {
  if (value === FITS) return { kind: "fits" };
  if (value === NONE) return { kind: "no-template" };
  return { kind: "named", id: value };
}

/**
 * **"This folder isn't a purlis project yet. Make it one?"** (#1822, spec #1810 story 28).
 *
 * The welcome draws this in its own place when the folder the Open door or the path box named is
 * in no project: no manifest at it or in any folder above it. Opening it as it is could only be
 * refused, so the page offers what the person most likely meant.
 *
 * **Make it a project** is Create a project's own act on this folder (FR-4, FR-17): purlis keeps
 * the project on this machine and opens the folder in a workspace of its own, laid out from the
 * template picked here. It opens through the same trust question every project does. **Nothing is
 * written into the folder**, so its code stays where it is (ADR 0035: never into a repo).
 *
 * **Not now** comes back to the welcome and leaves the folder untouched: nothing was written,
 * and nothing was opened.
 */
export function MakeItAProject({
  path,
  onMake,
  onBack,
  making,
  trouble,
  forgeAsk,
}: {
  /** The folder, as it was picked or typed. */
  path: string;
  /** Makes the project: the folder opened into this machine's project with `template`. */
  onMake: (path: string, template: TemplateChoice) => void;
  /** Not now: back to the welcome, with nothing done. */
  onBack: () => void;
  /** Whether the project is being made right now, so it is not asked twice. */
  making: boolean;
  /** Why the last attempt opened nothing, in the core's words. */
  trouble?: string;
  /** Which forge the folder's repos are on, when its remote did not say (#839). */
  forgeAsk?: ForgeAsk;
}) {
  const [templates, setTemplates] = useState<TemplateRow[]>();
  const [template, setTemplate] = useState(FITS);
  /** The template that fits this folder, by id; `null` when none does. */
  const [fits, setFits] = useState<string | null>();
  // The window keeps one refusal for every way a repo is opened: until Make it a project is
  // pressed here, the one it holds is another act's, and not said here.
  const [tried, setTried] = useState(false);
  const said = tried ? trouble : undefined;

  useEffect(() => {
    let gone = false;
    void commands
      .firstRunFound()
      .then((answer) => {
        if (!gone && answer.status === "ok") setTemplates(answer.data.templates);
      })
      .catch(() => undefined);
    void commands
      .templateThatFits(path)
      .then((answer) => {
        if (!gone && answer.status === "ok") setFits(answer.data);
      })
      .catch(() => undefined);
    return () => {
      gone = true;
    };
  }, [path]);

  const fitsTitle =
    fits === undefined
      ? "purlis picks by the files at the folder's top level."
      : fits === null
        ? "None fits this folder, so it opens with no template."
        : `${templates?.find((one) => one.id === fits)?.title ?? fits}, by the files at the folder's top level.`;

  return (
    <DoorPage title="Make it a project?" className="make-it-a-project" onBack={onBack}>
      <p className="welcome-lede">
        This folder isn&apos;t a purlis project yet. Make it one? purlis keeps the project on this
        machine and opens the folder in a workspace of its own. Nothing is written into the folder,
        so its code stays where it is.
      </p>
      <p>
        <code className="where">{path}</code>
      </p>

      {templates && templates.length > 0 && (
        <SettingRow
          label="Project template"
          help={
            "Personas, a review checklist and the commands purlis asks you about before a chat " +
            "runs them, for the stack you work in. Nothing is written into your folder."
          }
          grouped
          control={(ids) => (
            <Choice
              ids={ids}
              kind="radio"
              options={[
                { value: FITS, label: "Fits the folder", says: fitsTitle },
                ...templates.map((one) => ({
                  value: one.id,
                  label: one.title,
                  says: one.summary,
                })),
                { value: NONE, label: "None", says: "Only the steward persona." },
              ]}
              value={template}
              onValueChange={setTemplate}
              disabled={making}
            />
          )}
        />
      )}

      <SettingActions>
        <Button isDisabled={making} onPress={onBack}>
          Not now
        </Button>
        <Button
          isDisabled={making}
          onPress={() => {
            setTried(true);
            onMake(path, choiceOf(template));
          }}
        >
          Make it a project
        </Button>
      </SettingActions>

      {making && (
        <p className="came-back" role="status">
          Making the project and opening the folder in it…
        </p>
      )}

      {forgeAsk && <ForgeQuestion ask={forgeAsk} />}

      {/* Verbatim: the sentence names the folder and what was wrong with it. */}
      {said && (
        <p className="trouble said-in-full" role="alert">
          {said}
        </p>
      )}
    </DoorPage>
  );
}
