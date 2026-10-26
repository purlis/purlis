// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { openUrl } from "@tauri-apps/plugin-opener";
import { useEffect, useRef, useState, type FormEvent } from "react";
import { commands, type ProofRequest, type ShowingProof, type ShownProof } from "./bindings";
import { frameBytes, paint } from "./browserFrames";
import { counted } from "./dispatches";
import { Notice } from "./Notice";
import { Button, ButtonRow } from "./ui/button/Button";
import { Radio, RadioGroup } from "./ui/choice/Radio";
import { Form, TextField } from "./ui/field/Field";

/** The choice that names a request by its address rather than an offered one. */
const ANOTHER = "another";

/** What is said when the core could not be asked at all. */
const NOT_ASKED = "purlis could not be asked. Try again.";

/** Where Attach browser proof is: choosing the request, seeing what goes, or done. */
type Step =
  | { kind: "choosing" }
  | { kind: "showing"; showing: ShowingProof; attaching: boolean }
  | { kind: "attached"; comment: string };

/**
 * **Attach browser proof** (#2026, spec #2008 story 57): the screenshots the person picked in a
 * chat's browser timeline, attached to a pull or merge request only after the person has seen
 * exactly what goes: the request, where the pictures are kept and who can open them there, a
 * warning that they are public and permanent to the repo's readers, the comment's words and each
 * picture, drawn from the very bytes that will be sent. Nothing is attached by itself, and
 * nothing here sends a moment's words or page. The core refuses what changed since it was
 * shown, and attaches with the person's own purlis sign-in alone.
 */
export function BrowserProof({
  plane,
  chat,
  picked,
  onDone,
}: {
  plane: string;
  chat: string;
  /** The screenshots picked, by number, newest first as the timeline lists them. */
  picked: number[];
  onDone: () => void;
}) {
  const [offered, setOffered] = useState<ProofRequest[]>([]);
  const [choice, setChoice] = useState<string>(ANOTHER);
  const [typed, setTyped] = useState("");
  const [step, setStep] = useState<Step>({ kind: "choosing" });
  const [refused, setRefused] = useState<string | null>(null);
  const [asking, setAsking] = useState(false);
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
    if (request === "" || asking) return;
    setAsking(true);
    setRefused(null);
    void commands
      .showBrowserProof(plane, chat, request, picked)
      .then((answer) => {
        if (answer.status === "error") setRefused(answer.error);
        else setStep({ kind: "showing", showing: answer.data, attaching: false });
      })
      .catch(() => setRefused(NOT_ASKED))
      .finally(() => setAsking(false));
  };
  const attach = (showing: ShowingProof) => {
    setStep({ kind: "showing", showing, attaching: true });
    setRefused(null);
    const said = (why: string) => {
      setRefused(why);
      setStep({ kind: "showing", showing, attaching: false });
    };
    void commands
      .attachBrowserProof(plane, chat, showing.proof)
      .then((answer) => {
        if (answer.status === "error") said(answer.error);
        else setStep({ kind: "attached", comment: answer.data });
      })
      .catch(() => said(NOT_ASKED));
  };

  const refusal = refused !== null && (
    <Notice
      at="pane"
      tone="trouble"
      cause="browser-proof-refused"
      onDismiss={() => setRefused(null)}
    >
      {refused}
    </Notice>
  );

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
    const { showing } = step;
    const shown: ShownProof = showing.proof;
    return (
      <section
        className="browser-proof"
        aria-label="What goes to the request"
        data-testid="browser-proof-shown"
      >
        <p className="browser-proof-said">
          {counted(shown.shots.length, "screenshot goes", "screenshots go")} to {shown.number} in{" "}
          {shown.path}, as a comment by you. The pictures are kept {shown.keptAt}.
        </p>
        <blockquote className="browser-proof-words" data-testid="browser-proof-words">
          {shown.words}
        </blockquote>
        <p className="browser-proof-warning" data-testid="browser-proof-warning">
          {shown.warning}
        </p>
        <ol className="browser-proof-shots" aria-label="The screenshots that go">
          {showing.pictures.map((png, at) => (
            <li key={shown.shots[at]?.shot ?? at}>
              <Picture png={png} at={at + 1} />
            </li>
          ))}
        </ol>
        {refusal}
        <ButtonRow>
          <Button
            variant="primary"
            aria-disabled={step.attaching ? true : undefined}
            onPress={() => {
              if (!step.attaching) attach(showing);
            }}
          >
            Attach proof
          </Button>
          <Button
            onPress={() => {
              setRefused(null);
              setStep({ kind: "choosing" });
            }}
          >
            Back
          </Button>
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
        {refusal}
        <ButtonRow>
          <Button
            variant="primary"
            type="submit"
            aria-disabled={request === "" || asking ? true : undefined}
          >
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

/** One picture that goes, drawn on a canvas from the bytes that will be sent. */
function Picture({ png, at }: { png: string; at: number }) {
  const canvas = useRef<HTMLCanvasElement>(null);
  const [undrawn, setUndrawn] = useState(false);
  useEffect(() => {
    if (canvas.current === null) return;
    paint(canvas.current, frameBytes(png), undefined, "image/png").catch(() => setUndrawn(true));
  }, [png]);
  if (undrawn) return <p className="ui-text-meta">This screenshot could not be drawn.</p>;
  return (
    <canvas
      ref={canvas}
      className="browser-timeline-shot"
      role="img"
      aria-label={`Browser screenshot ${at}, as it goes`}
      data-testid="browser-proof-picture"
    />
  );
}
