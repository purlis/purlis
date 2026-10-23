// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useRef } from "react";
import * as Dialog from "./overlays/dialog";
import type { Ask, FontTheme } from "./bindings";
import { AnswerBar } from "./AnswerBar";
import { Button } from "./ui/button/Button";
import { Tooltip } from "./ui/tooltip/Tooltip";

/**
 * What opening this project will put in force, and the question about it.
 *
 * **The prompt IS the prompt** (ADR 0035). charter's CLI asks by printing a second
 * command to type, because `util.py` has nothing that reads stdin and a hook blocked on stdin
 * hangs a turn — a constraint about the CLI and about nothing else. Here there is a window and
 * a person looking at it, so the question is asked where the answer is given, and nothing is
 * copied from the CLI's printed-command shape.
 *
 * What it shows is what charter can enumerate, and no more: the plugins the project's
 * committed settings enable, the environment they set, the programs its reopen record would
 * start, the tools each persona may run without a prompt, and the font files its theme folders
 * bring (ADR 0098 as amended: a face from the project is drawn only as approved). It does not and cannot summarise
 * the project's persona charters, its memory or its todos, which are text a model will read and
 * act on — charter has no model and makes no judgements about the content of work. Said on screen, in the last line, rather than left for
 * whoever first assumes the dialog covered everything.
 *
 * A project that has changed what it contributes asks again, and the changes are charter's own
 * words for them (`machine::Change`) rather than a second description written here. Which
 * changes re-ask and which are only reported is `machine::Consent`'s decision and is taken
 * before this is drawn: a dialog that is up is a dialog that has to be answered.
 *
 * A Radix dialog (`docs/ui-primitives.md`). "Has to be answered" was a claim about how it was
 * drawn; with the primitive it is a property of the surface — the window behind it is inert,
 * and the keyboard cannot reach past it to open the project some other way.
 *
 * **Escape answers it now, and did not before.** A modal with no way out on the keyboard is
 * the one thing a modal must not be, and the answer Escape gives is the same as Cancel's:
 * nothing is approved, nothing is opened, and the next ask is a first ask again. A click
 * outside is not an answer and does not close it — missing a dialog is not a decision.
 */
export function ApprovePlane({
  ask,
  onApprove,
  onCancel,
  yes,
  fonts,
}: {
  ask: Ask;
  /** The theme folders an ask about font files alone is about (#2647): the dialog names them,
   *  and says what a font file is read by. Absent for an open's ask. */
  fonts?: readonly FontTheme[];
  /** What the yes is called, where it opens nothing: a project already open, whose refused
   *  theme the ask can let be drawn (#2647). An open's own words by default. */
  yes?: string;
  /** The operator's yes, carrying back the contribution they were shown — so the approval is
   *  for what was on screen and not for whatever the project says by the time it is clicked. */
  onApprove: (ask: Ask) => void;
  onCancel: () => void;
}) {
  const { contributes } = ask;
  const nothing =
    contributes.plugins.length === 0 &&
    contributes.env.length === 0 &&
    contributes.starts.length === 0 &&
    contributes.profiles.length === 0 &&
    contributes.grants.length === 0 &&
    contributes.fonts.length === 0;
  // Cancel, focused by the dialog itself rather than by tab order: opening a project puts
  // what this lists in force, and it is never what a stray Return key finds.
  const cancel = useRef<HTMLButtonElement>(null);
  return (
    <Dialog.Root
      open
      onOpenChange={(open) => {
        if (!open) onCancel();
      }}
    >
      <Dialog.Portal>
        <Dialog.Overlay className="ui-scrim" />
        <Dialog.Content
          className="ui-dialog warning"
          aria-labelledby="approve-plane"
          // Described by what purlis cannot judge in a project (#630), as the extension's
          // question is by what purlis cannot stop: the list above it is read in turn.
          aria-describedby="approve-plane-unjudged"
          // A click outside answers nothing: a dialog that is up is a dialog that has to be
          // answered, and dismissing it by missing it is not an answer.
          onInteractOutside={(e) => e.preventDefault()}
          onOpenAutoFocus={(e) => {
            e.preventDefault();
            cancel.current?.focus();
          }}
        >
          <Dialog.Title id="approve-plane">
            {ask.first ? "Open this project?" : "This project has changed since you approved it"}
          </Dialog.Title>
          {/* The root charter resolved, not the directory that was handed in: a picker pointed
            at a subfolder opens the project above it, and approving a directory you did not
            choose is the failure this dialog exists to prevent, arrived at from the friendly
            end. */}
          <p className="where">
            <code>{ask.path}</code>
          </p>

          {ask.changes.length > 0 && (
            <>
              <h3>What changed</h3>
              <ul className="changes">
                {ask.changes.map((change) => (
                  <li key={change}>{change}</li>
                ))}
              </ul>
            </>
          )}

          <h3>{ask.first ? "What this project contributes" : "What it contributes now"}</h3>
          {nothing && (
            <p className="came-back">
              Nothing purlis can enumerate: it enables no plugins, sets no environment, grants no
              persona a tool, brings no font files, and its record names no chat to start.
            </p>
          )}
          {contributes.plugins.length > 0 && (
            <section>
              <h4>Plugins it enables in every chat</h4>
              <ul className="contributes">
                {contributes.plugins.map(([name, how]) => (
                  <li key={name}>
                    <code>{name}</code>
                    {how && <span className="value"> {how}</span>}
                  </li>
                ))}
              </ul>
            </section>
          )}
          {contributes.env.length > 0 && (
            <section>
              {/* Values and not only names: an `env` whose PATH gains a directory is a different
                grant from the one that was approved, and a list of names could not show it. */}
              <h4>Environment it sets on every harness</h4>
              <ul className="contributes">
                {contributes.env.map(([name, value]) => (
                  <li key={name}>
                    <code>
                      {name}={value}
                    </code>
                  </li>
                ))}
              </ul>
            </section>
          )}
          {contributes.grants.length > 0 && (
            <section>
              {/* Each persona's grant as the trust record keeps it: the tools, and the digest
                of any script of its own a tool runs. Only what is approved here runs without a
                prompt, so a grant that changes later asks again. */}
              <h4>Tools its personas may run without a prompt</h4>
              <ul className="contributes">
                {contributes.grants.map(([persona, grant]) => (
                  <li key={persona}>
                    <code>{persona}</code>
                    <span className="value"> {grant}</span>
                  </li>
                ))}
              </ul>
            </section>
          )}
          {contributes.fonts.length > 0 && (
            <section>
              {/* Each font file by its path in the project, its size and its sha-256 (the full
                digest on hover): the window draws a face from the project only when it is
                approved here with that digest, so a face that changes later asks again. */}
              <h4>Font files its themes bring</h4>
              <FontFiles fonts={contributes.fonts} />
            </section>
          )}
          {contributes.starts.length > 0 && (
            <section>
              <h4>Programs opening it would start</h4>
              <ul className="contributes">
                {contributes.starts.map(([what]) => (
                  <li key={what}>
                    <code>{what}</code>
                  </li>
                ))}
              </ul>
            </section>
          )}
          {contributes.profiles.length > 0 && (
            <section>
              {/* Drawn, and deliberately drawn apart from the list above. The record chooses
                which of THIS machine's own harness profiles runs and never what it runs, and
                `profiletrust` shows a new or changed command line before it runs. */}
              <h4>Chats it would start on your own harness profiles</h4>
              <ul className="contributes">
                {contributes.profiles.map(([what]) => (
                  <li key={what}>
                    <code>{what}</code>
                  </li>
                ))}
              </ul>
            </section>
          )}

          <p className="came-back" id="approve-plane-unjudged">
            purlis can only list what it can read. A project&rsquo;s persona charters, memory and
            todos are text a model reads and acts on. purlis makes no judgement about them.
          </p>

          {/* `tabIndex={0}` on both, per `docs/ui-primitives.md` (charter-app#186): WebKit
              leaves a `<button>` out of the tab sequence unless its `tabindex` is written
              down, and a dialog should not depend on having exactly two answers to be
              reachable. */}
          {fonts !== undefined && (
            <>
              {fonts.length > 0 && (
                <p className="where" id="approve-plane-fonts-of">
                  {fontsOf(fonts)}
                </p>
              )}
              <p className="came-back">
                Font files are read by your system&rsquo;s font engine; allow only fonts you or your
                team added.
              </p>
            </>
          )}

          <AnswerBar>
            <Button ref={cancel} onPress={onCancel}>
              Cancel
            </Button>
            <Button onPress={() => onApprove(ask)}>
              {yes ?? (ask.first ? "Open project" : "Open it anyway")}
            </Button>
          </AnswerBar>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}

/** The sentence naming the theme folders an ask about font files is about. */
function fontsOf(fonts: readonly FontTheme[]): string {
  const each = fonts.map(({ theme, workspace }) =>
    workspace === null
      ? `the theme ${theme} in the project`
      : `the theme ${theme} in workspace ${workspace}`,
  );
  return `These font files belong to ${each.join(", ")}.`;
}

/** How many font files the ask draws before it says how many more there are. */
const FONT_FILES_SHOWN = 12;

/** One font file as the contribution records it: its path, its sha-256 and its size. */
type FontFile = { folder: string; name: string; digest: string; bytes: number };

function fontFile([path, listed]: [string, string]): FontFile {
  const at = path.lastIndexOf("/fonts/");
  const [digest = "", bytes = "0"] = listed.split(" ");
  return {
    folder: at < 0 ? "" : path.slice(0, at),
    name: at < 0 ? path : path.slice(at + "/fonts/".length),
    digest,
    bytes: Number(bytes) || 0,
  };
}

function size(bytes: number): string {
  return bytes < 1024 ? `${bytes} bytes` : `${Math.round(bytes / 1024)} KiB`;
}

/**
 * A project's font files, grouped by the theme folder that brings them: each file's name, its
 * size and the first twelve hex digits of its digest, the whole digest on hover. The first
 * {@link FONT_FILES_SHOWN} are drawn, and the rest are counted.
 */
function FontFiles({ fonts }: { fonts: [string, string][] }) {
  const files = fonts.map(fontFile);
  const shown = files.slice(0, FONT_FILES_SHOWN);
  const folders = [...new Set(shown.map((file) => file.folder))];
  const more = files.length - shown.length;
  return (
    <>
      {folders.map((folder) => (
        <div key={folder}>
          <p className="where">
            <code>{folder}</code>
          </p>
          <ul className="contributes">
            {shown
              .filter((file) => file.folder === folder)
              .map((file) => (
                <li key={file.name}>
                  <code>{file.name}</code>
                  <span className="value">
                    {" "}
                    {size(file.bytes)}, sha-256{" "}
                    <Tooltip tip={file.digest}>
                      <span>{file.digest.slice(0, 12)}</span>
                    </Tooltip>
                  </span>
                </li>
              ))}
          </ul>
        </div>
      ))}
      {more > 0 && (
        <p className="came-back">
          and {more} more font file{more === 1 ? "" : "s"}
        </p>
      )}
    </>
  );
}
