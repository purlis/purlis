// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useCallback, useEffect, useId, useLayoutEffect, useRef, useState } from "react";
import { commands, type PlaneId } from "./bindings";
import { Button } from "./ui/button/Button";
import { Card } from "./ui/card/Card";
import { Form, TextArea, useFormRefusal, type Refused } from "./ui/field/Field";

/** What the form says when it is sent with nothing in it. The core says the same of blanks. */
export const EMPTY_ANSWER = "The answer is empty, so nothing was sent.";

/** What the form shows where the task has no question to answer any more. */
export function noQuestionSaid(task: string): string {
  return `${task} has no question waiting for an answer now.`;
}

/** What follows the reason a question closed while the person was typing an answer to it. */
export const TEXT_IS_KEPT = "What you typed is still in the box, to copy.";

/** The question a form answers: the number the core gave it, and its words. */
type Asked = { number: number; question: string };

/**
 * **The person answers a question a task put to its asking chat** (#1496, V100-46): a small
 * inline form with the question, a text area, Send and Cancel.
 *
 * The task is handed the answer marked as the person's and carries on; the asking chat is told
 * the person answered, and its own answer to that question is refused from then on.
 *
 * **Mounted wherever a task's question is shown**: on the question's line in a chat's Activity
 * tab, and by any other surface that names a task (its row in the Chats list, its tab's menu).
 * A surface that has the question passes its `number` and its words as `question`; one that
 * does not leaves both out, and the form asks the core which question the task is paused on.
 *
 * - **It answers one question, by its number.** The core numbers each question as it is asked.
 *   The number of the question shown goes to the core with the answer, and an answer for a
 *   question the task is no longer paused on is refused there, though the task has asked
 *   another in the very same words. It is never delivered as the answer to another.
 * - **The question is drawn as text.** It is a chat's own words: a text node, with its line
 *   breaks, never read as Markdown or markup.
 * - **Enter sends, Shift+Enter starts a new line**, as in a chat's own pane. An Enter that only
 *   picks a candidate while composing a character is not a send. **One send is final**: a
 *   question is answered once, and more is said in the task's own tab.
 * - **Empty is refused**, in a sentence, and nothing is asked of the core.
 * - **What the person typed is never lost or cut.** A refusal from the core (the asking chat
 *   answered first, the task has moved on, the text is too long or holds a character purlis
 *   hands to no chat) is shown as the core says it, and the text stays in the box.
 * - **It stays while they type, whatever happens to the question.** Where the question closes
 *   under them (`closed`: the asking chat answered first, or the task reported or ended), the
 *   form says why, keeps their text in the box where it can be selected and copied, and has
 *   nothing left to send.
 */
export function AnswerQuestion(props: AnswerProps & { onCancel: () => void }) {
  const { task, onCancel } = props;
  const answering = useAnswerQuestion(props);
  const { asked, refused, shut, sending, send, ids } = answering;
  if (asked === undefined) return <ReadingTheQuestion />;
  if (asked === null) {
    return (
      <Card className="answer-question" data-testid="answer-question">
        <NoQuestion task={task} refused={refused} />
        <div className="answer-actions">
          <Button size="sm" onPress={onCancel}>
            Close
          </Button>
        </div>
      </Card>
    );
  }
  return (
    <Card className="answer-question">
      <Form
        className="answer-form"
        // The form is what is named and what a test finds: the card only stands it off the line.
        data-testid="answer-question"
        aria-label={`Answer ${task}'s question`}
        onSubmit={(event) => {
          event.preventDefault();
          void send();
        }}
      >
        <AnswerBox answering={answering} />
        <div className="answer-actions">
          <Button size="sm" type="submit" isDisabled={sending || shut !== undefined}>
            Send
          </Button>
          <Button size="sm" onPress={onCancel}>
            {shut === undefined ? "Cancel" : "Close"}
          </Button>
          {shut === undefined && (
            <span className="answer-how" id={`${ids}-how`}>
              {answerHowSaid(task)}
            </span>
          )}
        </div>
      </Form>
    </Card>
  );
}

/** What a form for a task's question is handed: which task, and the question if it is known. */
export type AnswerProps = {
  plane: PlaneId;
  /** The task's chat, by the app's number for it now. */
  session: number;
  /** The task, by the name the person sees it under. */
  task: string;
  /** The question's number, as the core gave it; left out with `question`, the core is asked. */
  number?: number;
  /** The question's words as the surface shows them. */
  question?: string;
  /** Why the question can no longer be answered, in one sentence, once that is so. */
  closed?: string;
  /** The answer was taken: the surface puts the form away. */
  onDone: () => void;
};

/** What the form says under the box, while there is something to send. */
export function answerHowSaid(task: string): string {
  return `${task} is told this is your answer. Enter sends, Shift+Enter starts a new line.`;
}

/**
 * **The answering itself**, for whatever draws the form: the inline form above, and the Answer
 * dialog, whose Send and Cancel are its footer's (`AnswerPanel.tsx`). The rules are
 * {@link AnswerQuestion}'s.
 */
export function useAnswerQuestion({
  plane,
  session,
  number,
  question,
  closed,
  onDone,
}: AnswerProps) {
  const given: Asked | undefined =
    number !== undefined && question !== undefined ? { number, question } : undefined;
  /** The question the core says the task is paused on, where the surface gave none. */
  const [read, setRead] = useState<Asked | null>();
  /** The question to answer; `null` where the task asks none, nothing while it is read. */
  const asked = given ?? read;
  const [text, setText] = useState("");
  const [sending, setSending] = useState(false);
  const [refused, setRefused] = useState<string>();
  const box = useRef<HTMLTextAreaElement>(null);
  /** The form the box is drawn in, which says a refused send once (#3291): set by the box. */
  const sayTo = useRef<(refused: Refused) => void>(undefined);
  const heardBy = useCallback((say: (refused: Refused) => void) => {
    sayTo.current = say;
  }, []);
  /** A send refused, in the box's error line and, once, by its form. */
  const refuse = (why: string) => {
    setRefused(why);
    sayTo.current?.({ fields: [{ name: "Your answer", why }] });
  };
  const ids = useId();

  const has = given !== undefined;
  useEffect(() => {
    if (has) return;
    let left = false;
    void commands
      .taskQuestion(plane, session)
      .then((answer) => {
        if (left) return;
        if (answer.status === "error") {
          setRead(null);
          setRefused(answer.error);
        } else if (answer.data === null) setRead(null);
        else setRead({ number: answer.data.number, question: answer.data.question });
      })
      .catch((err: unknown) => {
        if (left) return;
        setRead(null);
        setRefused(`purlis could not read the question: ${String(err)}`);
      });
    return () => {
      left = true;
    };
  }, [plane, session, has]);

  // The box is where the person types next, once there is a question to answer.
  const ready = asked !== undefined && asked !== null;
  useEffect(() => {
    if (ready) box.current?.focus();
  }, [ready]);

  /* Closed under the person: not while their own send is on its way, whose answer is what
     closes it. */
  const shut = closed !== undefined && !sending ? `${closed} ${TEXT_IS_KEPT}` : undefined;

  const send = async () => {
    if (sending || shut !== undefined || asked === undefined || asked === null) return;
    if (text.trim() === "") {
      refuse(EMPTY_ANSWER);
      box.current?.focus();
      return;
    }
    setSending(true);
    setRefused(undefined);
    const answer = await commands
      .answerTaskQuestion(plane, session, asked.number, asked.question, text)
      .catch((err: unknown) => ({ status: "error" as const, error: String(err) }));
    setSending(false);
    if (answer.status === "error") {
      // Said as the core says it, with what was typed still in the box.
      refuse(answer.error);
      box.current?.focus();
      return;
    }
    onDone();
  };

  return { asked, text, setText, sending, refused, setRefused, shut, send, box, ids, heardBy };
}

/** What {@link useAnswerQuestion} hands what draws the form. */
export type Answering = ReturnType<typeof useAnswerQuestion>;

/** The form while the question is read. */
export function ReadingTheQuestion() {
  return (
    <p className="answer-pending" aria-busy="true">
      Reading the question…
    </p>
  );
}

/** Where the task asks nothing now: the core's refusal, or that it has no question. */
export function NoQuestion({ task, refused }: { task: string; refused?: string }) {
  return (
    <p className="answer-refused" role="alert">
      {refused ?? noQuestionSaid(task)}
    </p>
  );
}

/** The question, the box the answer is typed in, and what is wrong, if anything is. */
export function AnswerBox({ answering }: { answering: Answering }) {
  const { asked, text, setText, sending, refused, setRefused, shut, send, box, ids, heardBy } =
    answering;
  // The form the box is in says the answer's refusal once (#3291).
  const say = useFormRefusal();
  useLayoutEffect(() => heardBy(say), [say, heardBy]);
  if (asked === undefined || asked === null) return null;
  return (
    <>
      <p className="answer-asked" id={`${ids}-asked`}>
        {asked.question}
      </p>
      {/* The last refusal is the field's error, under the box; a question that closed is not
          the answer's fault, and is said on its own line. */}
      <TextArea
        inputRef={box}
        className="answer-text"
        aria-label="Your answer"
        aria-describedby={
          shut === undefined ? `${ids}-asked ${ids}-how` : `${ids}-asked ${ids}-refused`
        }
        error={shut === undefined ? refused : undefined}
        rows={Math.min(Math.max(2, text.split("\n").length), 10)}
        value={text}
        isDisabled={sending}
        // Closed: nothing more is typed, and what was typed can still be selected and copied.
        isReadOnly={shut !== undefined}
        onChange={(to) => {
          setText(to);
          setRefused(undefined);
        }}
        onKeyDown={(event) => {
          // An Enter that picks a candidate while composing a character is the input
          // method's, not a send.
          if (event.key !== "Enter" || event.nativeEvent.isComposing) return;
          if (event.shiftKey || event.altKey) return;
          event.preventDefault();
          void send();
        }}
      />
      {shut !== undefined && (
        <p className="answer-refused" id={`${ids}-refused`} role="alert">
          {shut}
        </p>
      )}
    </>
  );
}
