// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The theme specimen** (#2330, spec #2313): a fixed page of sample content, drawn in one
 * theme, that a chat's `theme_preview` and Settings › Theme's cards photograph.
 *
 * It is built from the window's own pieces wherever one exists, so it shows what the window
 * draws and stays honest as they change: the Settings components and every kind of control
 * they have, a Notice, the dialog surface with its answer bar, the light editor's look over a
 * real grammar, and xterm drawn from the theme's terminal tokens. Each is handed the fixture
 * (`fixture.ts`) and nothing of the person's: the page is given a theme and its size, and its
 * window may invoke no command (`specimen.rs`).
 *
 * It is drawn once and never changes, so each control's callbacks do nothing.
 */
import { useLayoutEffect, useRef } from "react";
import { EditorState } from "@codemirror/state";
import { EditorView, lineNumbers } from "@codemirror/view";
import { rust } from "@codemirror/lang-rust";
import { Notice, NoticeList } from "../Notice";
import {
  Choice,
  Field,
  SettingActions,
  SettingGroup,
  SettingRow,
  SettingsLayout,
} from "../settings/components";
import { look } from "../editor/look";
import { DEFAULT_TEXT } from "../textDefaults";
import type { Theme } from "../theme/theme";
import { ReadOnlyTerminal } from "../ReadOnlyTerminal";
import { CHAT_LINES, EDITOR_FILE, EDITOR_TEXT, TERMINAL_LINES } from "./fixture";
import { Button } from "../ui/button/Button";
import { submitKeySaid } from "../ui/field/submitKey";
import { DialogInPlace } from "../ui/dialog/Dialog";

/** What a control is handed to do: nothing, the specimen never changes. */
const still = () => {};

/** What the page waits on before it says it is drawn: each terminal's lines, written. */
export type Drawing = { wait: (written: Promise<void>) => void };

const OPTIONS = [
  { value: "comfortable", label: "Comfortable", says: "More room between rows" },
  { value: "compact", label: "Compact" },
  { value: "dense", label: "Dense", disabled: true, title: "Not offered by this theme" },
];

/**
 * A terminal of the specimen's: the terminal's read-only mode (#3676), drawn by xterm from the
 * theme's terminal tokens and read as text. The specimen is a photograph of a theme, so the words
 * that say it is read-only are said and not drawn.
 */
function SpecimenTerminal({
  label,
  lines,
  theme,
  drawing,
}: {
  label: string;
  lines: readonly string[];
  theme: Theme;
  drawing: Drawing;
}) {
  return (
    <ReadOnlyTerminal
      className="specimen-terminal"
      screen={lines.join("\r\n")}
      label={label}
      noteShown={false}
      still
      cols={72}
      rows={lines.length + 1}
      theme={theme}
      fontSize={DEFAULT_TEXT.terminal}
      onWriting={(written) => drawing.wait(written)}
    />
  );
}

/** The specimen's editor: the light editor's look over a real grammar, read only. */
function SpecimenEditor() {
  const holder = useRef<HTMLDivElement>(null);
  useLayoutEffect(() => {
    const where = holder.current;
    if (!where) return;
    const view = new EditorView({
      parent: where,
      state: EditorState.create({
        doc: EDITOR_TEXT,
        extensions: [
          EditorView.contentAttributes.of({ "aria-label": EDITOR_FILE }),
          lineNumbers(),
          EditorState.readOnly.of(true),
          EditorView.editable.of(false),
          rust(),
          look,
        ],
      }),
    });
    return () => view.destroy();
  }, []);
  return <div className="specimen-editor" ref={holder} />;
}

/** A Settings page, with every kind of control the Settings components have. */
function SpecimenSettings() {
  return (
    <SettingsLayout
      levels={[
        { id: "you", label: "You" },
        { id: "project", label: "Project" },
      ]}
      level="you"
      onLevelChange={still}
      about="Your settings, on this machine."
      groups={[
        { id: "look", label: "Look" },
        { id: "keys", label: "Keys", sub: true },
      ]}
      group="look"
      onGroupChange={still}
      filter=""
      onFilterChange={still}
      foot={<p>Kept in your settings file.</p>}
    >
      <SettingGroup label="Look" help="How the window is drawn.">
        <SettingRow
          label="Name"
          help="What the window calls you."
          reset={{ label: "Reset name", disabled: false, onReset: still }}
          origin="Set in your settings file"
          badge={{ label: "This machine", says: "From your settings file, over the project's." }}
          modified
          error={["The name is longer than the title bar can show"]}
          control={(ids) => (
            <Field ids={ids} kind="text" value="Ada" placeholder="Your name" onChange={still} />
          )}
        />
        <SettingRow
          label="Text size"
          control={(ids) => (
            <Field
              ids={ids}
              kind="range"
              value={14}
              min={10}
              max={24}
              shown={(value) => `${value}px`}
              onChange={still}
            />
          )}
        />
        <SettingRow
          label="Density"
          grouped
          control={(ids) => (
            <Choice
              ids={ids}
              kind="radio"
              options={OPTIONS}
              value="comfortable"
              onValueChange={still}
            />
          )}
        />
        <SettingRow
          label="Appearance"
          control={(ids) => (
            <Choice
              ids={ids}
              kind="select"
              options={[
                { value: "system", label: "Follow the system" },
                { value: "dark", label: "Dark" },
                { value: "light", label: "Light" },
              ]}
              value="system"
              onValueChange={still}
            />
          )}
        />
        <SettingRow
          label="Reduce motion"
          control={(ids) => <Choice ids={ids} kind="switch" checked onCheckedChange={still} />}
        />
        <SettingRow
          label="Show line numbers"
          control={(ids) => (
            <Choice ids={ids} kind="toggle" checked={false} onCheckedChange={still} />
          )}
        />
        <SettingRow
          label="Panels"
          grouped
          control={(ids) => (
            <Choice
              ids={ids}
              kind="checks"
              options={OPTIONS}
              checked={new Set(["comfortable"])}
              onCheckedChange={still}
            />
          )}
        />
        <Choice
          ids={{ id: "specimen-chips", label: "Show only" }}
          kind="chips"
          options={[
            { value: "needs", label: "needs you" },
            { value: "working", label: "working" },
          ]}
          checked={new Set(["working"])}
          onCheckedChange={still}
        />
        <SettingActions>
          {/* With the key that sends it, so a theme's `button.shortcut` is seen (#3510). */}
          <Button variant="primary" shortcut={submitKeySaid()} onPress={still}>
            Save
          </Button>
          <Button onPress={still}>Cancel</Button>
        </SettingActions>
      </SettingGroup>
    </SettingsLayout>
  );
}

/** A dialog, drawn in place: the dialog family's surface, rows and footer, open, with nothing
 *  behind it to hold. */
export function SpecimenDialog() {
  return (
    <DialogInPlace
      onClose={still}
      title="Delete the branch sample/header?"
      titleId="specimen-dialog-title"
      description={<p className="came-back">Its two commits are not on any other branch.</p>}
      main={{ children: "Delete branch", variant: "danger", onPress: still }}
    >
      <SettingRow
        label="Type sample/header to confirm"
        control={(ids) => <Field ids={ids} kind="text" value="sample/he" onChange={still} />}
      />
    </DialogInPlace>
  );
}

/** **The whole specimen**, in `theme`. */
export function Specimen({ theme, drawing }: { theme: Theme; drawing: Drawing }) {
  return (
    <main className="specimen" aria-label="Theme specimen">
      <section className="specimen-chat" aria-label="Chat">
        <SpecimenTerminal label="A chat" lines={CHAT_LINES} theme={theme} drawing={drawing} />
      </section>
      <section className="specimen-shell" aria-label="Terminal">
        <SpecimenTerminal
          label="A terminal"
          lines={TERMINAL_LINES}
          theme={theme}
          drawing={drawing}
        />
      </section>
      <section className="specimen-file" aria-label="Editor">
        <SpecimenEditor />
      </section>
      <section className="specimen-settings" aria-label="Settings">
        <SpecimenSettings />
      </section>
      <section className="specimen-dialog" aria-label="Dialog">
        <SpecimenDialog />
      </section>
      <section className="specimen-notice" aria-label="Notice">
        <NoticeList>
          <Notice
            cause="specimen-notice"
            tone="trouble"
            fixes={[{ label: "Try again", onPress: still }]}
            link={{ label: "Open settings", onPress: still }}
            copy="purlis doctor"
            onDismiss={still}
          >
            The sample project could not be saved: its remote refused the push.
          </Notice>
        </NoticeList>
      </section>
    </main>
  );
}
