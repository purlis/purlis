// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { openUrl } from "@tauri-apps/plugin-opener";
import { useEffect, useState, type FormEvent, type ReactNode } from "react";
import { commands, type ProofRequest, type ShownProof } from "./bindings";
import { Button, ButtonRow } from "./ui/button/Button";
import { Radio, RadioGroup } from "./ui/choice/Radio";
import { Form, TextField } from "./ui/field/Field";

/** The choice that names a request by its address rather than an offered one. */
const ANOTHER = "another";

/** Where Attach browser proof is: choosing the request, seeing what goes, or done. */
type Step =
  | { kind: "choosing"; refused: string | null }
  | { kind: "showing"; shown: ShownProof; refused: string | null; attaching: boolean }
  | { kind: "attached"; comment: string };

/**
 * **Attach browser proof** (#2026, spec #2008 story 57): the screenshots the person picked in a
 * chat's browser timeline, attached to a pull or merge request only after the person has seen
 * exactly what goes: the request, where the pictures are kept and who can read them there, the
 * comment's words and each picture. Nothing is attached by itself, and nothing here sends a
 * moment's words or page. The core refuses what changed since it was shown, and attaches with
 * the person's own purlis sign-in alone.
 */
export function BrowserProof({
  plane,
  chat,
  picked,
  drawn,
  onDone,
}: {
  plane: string;
  chat: string;
  /** The screenshots picked, by number, newest first as the timeline lists them. */
  picked: number[];
  /** Draws screenshot `shot` as the timeline draws it. */
  drawn: (shot: number) => ReactNode;
  onDone: () => void;
}) {
  const [offered, setOffered] = useState<ProofRequest[]>([]);
  const [choice, setChoice] = useState<string>(ANOTHER);
  const [typed, setTyped] = useState("");
  const [step, setStep] = useState<Step>({ kind: "choosing", refused: null });
  useEffect(() => {
    let gone = false;
    void commands
      .browserProofRequests(plane, chat)
      .then((answer) => {
        if (gone || answer.status === "error") return;
        setOffered(answer.data);
        if (answer.data.length > 0) setChoice(answer.data[0].url);
      })
      .catch(() => undefined);
    return () => {
      gone = true;
    };
  }, [plane, chat]);

  const request = choice === ANOTHER ? typed.trim() : choice;
  const show = (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    if (request === "") return;
    void commands
      .showBrowserProof(plane, chat, request, picked)
      .then((answer) =>
        setStep(
          answer.status === "error"
            ? { kind: "choosing", refused: answer.error }
            : { kind: "showing", shown: answer.data, refused: null, attaching: false },
        ),
      )
      .catch(() => undefined);
  };
  const attach = (shown: ShownProof) => {
    setStep({ kind: "showing", shown, refused: null, attaching: true });
    void commands
      .attachBrowserProof(plane, chat, shown)
      .then((answer) =>
        setStep(
          answer.status === "error"
            ? { kind: "showing", shown, refused: answer.error, attaching: false }
            : { kind: "attached", comment: answer.data },
        ),
      )
      .catch(() => undefined);
  };

  if (step.kind === "attached")
    return (
      <section className="browser-proof" aria-label="Browser proof" data-testid="browser-proof">
        <p className="browser-proof-said">The proof is attached.</p>
        <ButtonRow>
          <Button variant="link" onPress={() => void openUrl(step.comment).catch(() => undefined)}>
            Open the comment
          </Button>
          <Button onPress={onDone}>Done</Button>
        </ButtonRow>
      </section>
    );
  if (step.kind === "showing") {
    const { shown } = step;
    const count = shown.shots.length === 1 ? "1 screenshot" : `${shown.shots.length} screenshots`;
    return (
      <section
        className="browser-proof"
        aria-label="What goes to the request"
        data-testid="browser-proof-shown"
      >
        <p className="browser-proof-said">
          {count} go to {shown.number} in {shown.path}, as a comment by you. The pictures are kept{" "}
          {shown.keptAt}.
        </p>
        <blockquote className="browser-proof-words" data-testid="browser-proof-words">
          {shown.words}
        </blockquote>
        <ol className="browser-proof-shots" aria-label="The screenshots that go">
          {shown.shots.map((one) => (
            <li key={one.shot}>{drawn(one.shot)}</li>
          ))}
        </ol>
        {step.refused !== null && (
          <p className="browser-proof-refused" role="alert">
            {step.refused}
          </p>
        )}
        <ButtonRow>
          <Button
            variant="primary"
            aria-disabled={step.attaching ? true : undefined}
            onPress={() => {
              if (!step.attaching) attach(shown);
            }}
          >
            Attach
          </Button>
          <Button onPress={() => setStep({ kind: "choosing", refused: null })}>Back</Button>
          <Button variant="ghost" onPress={onDone}>
            Cancel
          </Button>
        </ButtonRow>
      </section>
    );
  }
  return (
    <section
      className="browser-proof"
      aria-label="Attach browser proof"
      data-testid="browser-proof"
    >
      <Form className="browser-proof-choose" aria-label="Attach browser proof" onSubmit={show}>
        {offered.length > 0 && (
          <RadioGroup aria-label="The request" value={choice} onChange={setChoice}>
            {offered.map((one) => (
              <Radio key={one.url} value={one.url} description={one.url}>
                The {one.request} of {one.repo}
              </Radio>
            ))}
            <Radio value={ANOTHER}>Another request</Radio>
          </RadioGroup>
        )}
        {choice === ANOTHER && (
          <TextField
            label="The request's address"
            placeholder="https://github.com/owner/repo/pull/12"
            value={typed}
            onChange={setTyped}
          />
        )}
        {step.refused !== null && (
          <p className="browser-proof-refused" role="alert">
            {step.refused}
          </p>
        )}
        <ButtonRow>
          <Button variant="primary" type="submit" aria-disabled={request === "" ? true : undefined}>
            Show what goes
          </Button>
          <Button variant="ghost" onPress={onDone}>
            Cancel
          </Button>
        </ButtonRow>
      </Form>
    </section>
  );
}
