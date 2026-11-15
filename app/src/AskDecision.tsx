// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useState, type ReactNode } from "react";
import type { Offered } from "./bindings";
import { FixWay, type NoticeAction } from "./Notice";
import { TextField } from "./ui/field/Field";
import { ButtonRow, type ButtonVariant } from "./ui/button/Button";
import { MessageForm, MessageLinks } from "./ui/message/Message";

/**
 * **A decision, answered on its row** (#5998, ADR 0121 amended 2026-11-09): a Blocking ask that is
 * a decision shows its options on its row in Asks, laid out as a chat pane's band lays out a
 * sandbox ask (`SandboxBlockNotice`): **the narrowest grant**, then **Other scopes…**, which opens
 * the wider grants under the line, then the refusals. Each is the band's own way out
 * (`FixWay`), and each grant asks the row's settle guard (`mayAct`, ADR 0097) before it does
 * anything, as an Allow in Asks always has; a refusal is never held (#1695), but on an ask whose
 * every answer is held (`holdsEvery`, a block loop's pause, T1-36).
 *
 * An ask that offers a reason with its refusal (a sandbox's Keep blocked) opens its box only once
 * Keep blocked is pressed, and the keyboard stays on Keep blocked: a second press sends the
 * refusal with what was typed, or with nothing.
 *
 * **A high-risk ask needs a deliberate press** (ADR 0121 decision 8, #6011): its grants ask once
 * more under the line ("Allow a push to main?"), and only the Allow there answers, held by the
 * settle guard as every grant is, since what it stands in was just drawn. Cancel takes it back.
 */

/** What the disclosure of the wider grants says, on a pane's band and a row in Asks alike. */
export const OTHER_SCOPES = "Other scopes…";

/**
 * **How narrow a grant is, by its option's id**: one run, then this chat. Every other grant is
 * wider, in the order its source offers it. A permission's Allow is one call.
 */
const NARROWEST: readonly string[] = ["once", "allow-once", "allow", "chat", "allow-chat"];

const narrowness = (option: Offered) => {
  const at = NARROWEST.indexOf(option.id);
  return at < 0 ? NARROWEST.length : at;
};

/**
 * **An ask's options as its row draws them**: the narrowest grant, the wider ones and the rest,
 * each in the order the source offers it. Only grants that are scopes of one another fold under
 * Other scopes…, where the narrowest is a scope this knows (one run, this chat); a harness
 * question's choices, or a browser's sites, are each an answer of its own, all on the row.
 */
export function waysOf(offers: readonly Offered[]): {
  main: Offered | undefined;
  wider: Offered[];
  rest: Offered[];
} {
  const grants = offers.filter((one) => one.allows);
  const main = grants.reduce<Offered | undefined>(
    (best, one) => (best === undefined || narrowness(one) < narrowness(best) ? one : best),
    undefined,
  );
  if (main === undefined || narrowness(main) === NARROWEST.length)
    return { main: undefined, wider: [], rest: [...offers] };
  return {
    main,
    wider: grants.filter((one) => one !== main),
    rest: offers.filter((one) => !one.allows),
  };
}

export function AskDecision({
  id,
  who,
  offers,
  grants,
  holdsEvery = false,
  ready = true,
  busy,
  reason,
  mayAct,
  confirm,
  onOpen,
  onAnswer,
  children,
}: {
  /** The id of what the row opens under its line: the wider grants, and the reason box. */
  id: string;
  /** Whose ask it is, as the row names it. */
  who: string;
  offers: readonly Offered[];
  /** Its Allows are grants (ADR 0097 §6): an ask of Allow and Keep blocked. Any other ask's
   *  answers are plain, the one its source suggests the primary (#2241). */
  grants: boolean;
  /** **Every answer waits for the row to settle** (T1-36), its refusals too: a block loop's pause. */
  holdsEvery?: boolean;
  /** **What its answers let run is on screen** (#1146): an edit's change is drawn. Until then no
   *  answer is drawn, only the row's other links. */
  ready?: boolean;
  /** An answer is on its way: every way says so, and a press sends nothing. */
  busy: boolean;
  /** Its Keep blocked takes a reason, which its chat is told (#1987). */
  reason: boolean;
  /** **The settle guard** (ADR 0097, #1695): whether a press of a grant on `way` may act now. */
  mayAct: (way: Element) => boolean;
  /** **What a grant asks once more before it acts**, on a high-risk ask (#6011): "Allow a push
   *  to main?". Absent on every other ask, whose grants act on their press. */
  confirm?: string;
  /** Something opened under its line, or closed: what stands below it moved, and settles. */
  onOpen: (open: boolean) => void;
  onAnswer: (option: Offered, reason?: string) => void;
  /** The row's other links, after its answers (Go to chat). */
  children?: ReactNode;
}) {
  const { main, wider, rest } = waysOf(offers);
  const [scopesOpen, setScopesOpen] = useState(false);
  const [why, setWhy] = useState<string>();
  /** The grant pressed on a high-risk ask, asking once more under the line (#6011). */
  const [asking, setAsking] = useState<Offered>();
  /** Opens or closes what stands under its line, and says whether anything does now. */
  const underIs = (scopes: boolean, reasoning: boolean, confirming = asking !== undefined) => {
    setScopesOpen(scopes);
    onOpen(scopes || reasoning || confirming);
  };
  /** Asks once more for `option`, or takes the question back. */
  const askAgain = (option: Offered | undefined) => {
    setAsking(option);
    underIs(scopesOpen, why !== undefined, option !== undefined);
  };
  const keepsWithReason = (option: Offered) => reason && option.id === "keep";
  /** Its look: an Allow of a grant's (ADR 0097 §6), the answer its source suggests (#2241), or
   *  plain. */
  const variantOf = (one: NoticeAction): ButtonVariant =>
    one.allows === true && grants ? "grants" : one.primary === true ? "primary" : "secondary";
  const way = (option: Offered): NoticeAction => ({
    label: option.label,
    allows: option.allows,
    primary: option.preselected === true,
    busy,
    onPress: () => {
      if (confirm !== undefined && option.allows) {
        askAgain(asking === option ? undefined : option);
        return;
      }
      if (keepsWithReason(option) && why === undefined) {
        setWhy("");
        underIs(scopesOpen, true);
        return;
      }
      onAnswer(option, keepsWithReason(option) ? why : undefined);
    },
    opens:
      confirm !== undefined && option.allows
        ? { id, open: asking === option }
        : keepsWithReason(option)
          ? { id, open: why !== undefined }
          : undefined,
  });
  const ways: NoticeAction[] = [
    ...(main === undefined ? [] : [way(main)]),
    ...(wider.length > 0
      ? [
          {
            label: OTHER_SCOPES,
            onPress: () => underIs(!scopesOpen, why !== undefined),
            opens: { id, open: scopesOpen },
          },
        ]
      : []),
    ...rest.map(way),
  ];
  const under = scopesOpen || why !== undefined || asking !== undefined;
  return (
    <>
      <MessageLinks>
        {ready && (
          <ButtonRow size="sm" role="group" aria-label={`Answers to ${who}`}>
            {ways.map((one) => (
              <FixWay
                key={one.label}
                fix={one}
                mayAct={one.allows === true || holdsEvery ? mayAct : always}
                variant={variantOf(one)}
              />
            ))}
          </ButtonRow>
        )}
        {children}
      </MessageLinks>
      {ready && under && (
        <MessageForm id={id}>
          {scopesOpen && (
            <ButtonRow size="sm" role="group" aria-label={`Other scopes for ${who}`}>
              {wider.map((one) => (
                <FixWay key={one.id} fix={way(one)} mayAct={mayAct} variant={variantOf(way(one))} />
              ))}
            </ButtonRow>
          )}
          {confirm !== undefined && asking !== undefined && (
            <ButtonRow size="sm" role="group" aria-label={confirm}>
              <span className="ask-confirm ui-text-strong">{confirm}</span>
              <FixWay
                fix={{
                  label: asking.label,
                  allows: true,
                  busy,
                  onPress: () => onAnswer(asking),
                }}
                mayAct={mayAct}
                variant={variantOf({ label: asking.label, allows: true, onPress: always })}
              />
              <FixWay
                fix={{ label: "Cancel", onPress: () => askAgain(undefined) }}
                mayAct={always}
              />
            </ButtonRow>
          )}
          {why !== undefined && (
            <TextField
              className="inbox-reason"
              aria-label={`Why you keep it blocked, told to ${who}`}
              value={why}
              placeholder="Why keep it blocked? (optional, the chat is told)"
              maxLength={REASON_TYPED_MOST}
              spellCheck
              onChange={setWhy}
            />
          )}
        </MessageForm>
      )}
    </>
  );
}

/** What a way that grants nothing asks of the settle guard: nothing. */
const always = () => true;

/** The most characters the reason box takes; the chat is told at most 200 of them. */
export const REASON_TYPED_MOST = 400;
