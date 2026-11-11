// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useState } from "react";
import { commands, type Offered, type RuleMade, type RuleMadeShown, type Shown } from "./bindings";
import { asksMoved, onAsksMoved } from "./asks";
import { highRisk } from "./asksSame";
import { backAtSaid } from "./AsksSnooze";
import { chainSaid } from "./inboxRowParts";
import { counted } from "./dispatches";
import { FixWay } from "./Notice";
import { Button, ButtonRow } from "./ui/button/Button";
import { Dialog } from "./ui/dialog/Dialog";
import { MessageForm } from "./ui/message/Message";
import { SecretAskToolLine } from "./ui/secret-ask/SecretAsk";

/**
 * **Ask less** (ADR 0121 decision 7, #6010), as Asks draws it:
 *
 * - **A rule offered on a row** ({@link RuleOffer}): after the same Allow to the same request
 *   twice in a week, the ask offers its own wider grants as a rule, the narrowest that would have
 *   prevented the asks first (the core's `asking::with_rules`). A press answers the ask with that
 *   grant, through the path the ask names, checked by the row's settle guard as any Allow is,
 *   and keeps it in Rules I made. A rule is offered only where its source writes it and Rules I
 *   made can undo it: a sandbox's host, folder or service, and a reviewed program for its chat or,
 *   with Always allow, as its persona's tool (ADR 0112, #6177), whose line the offer shows before
 *   it is pressed; never an arbitrary command, never a high-risk ask ({@link highRisk}), and never
 *   a harness's own saved rule, which the window never writes.
 * - **Rules I made** ({@link RulesIMade}): each rule made, with what it allows and for whom, and
 *   Undo, which takes its grant away as Revoke on Settings' Granted list does, or takes the tool
 *   line an Always allow wrote out of the persona's tools again.
 * - **A cool-off** ({@link CoolingLine}): a chat that heard Don't allow twice in a row to the
 *   same request has its same ask held quietly for an hour, neither counted nor notified; the
 *   line says so, and End cool-off brings it back.
 */

/** What the link to the list, and the list's title, say. */
export const RULES_I_MADE = "Rules I made";

/** What a cool-off's way out says. */
export const END_COOL_OFF = "End cool-off";

/** **The rule `ask` offers**: its grants, the suggested one first, as its options offer them. */
export function offeredRules(ask: Shown): Offered[] {
  const offer = ask.rule?.offer;
  if (offer == null) return [];
  return offer.options.flatMap((id) => ask.options.filter((one) => one.id === id && one.allows));
}

/** The id of a run's Always allow, which writes its persona's tool line (ADR 0112). */
export const ALWAYS_ALLOW = "allow-always";

/** What the offer says above the tool line an Always allow would write. */
export const alwaysLineSaid = (persona: string) =>
  `Always allow adds this line to ${persona}'s tools in the project's committed file:`;

/** What the offer's line says. */
export const offerSaid = (times: number) =>
  `You gave this the same Allow ${times} times this week. Make it a rule, and it isn't asked again:`;

/** **The rule an ask's row offers**, under its answers. Nothing where it offers none. */
export function RuleOffer({
  ask,
  who,
  busy,
  mayAct,
  onMake,
}: {
  ask: Shown;
  /** Whose ask it is, as the row names it. */
  who: string;
  busy: boolean;
  /** The row's settle guard (ADR 0097): whether a press on `way` may act now. */
  mayAct: (way: Element) => boolean;
  /** A grant was pressed: the row answers its ask with it, as a rule. */
  onMake: (option: Offered) => void;
}) {
  const offer = ask.rule?.offer;
  const grants = highRisk(ask) ? [] : offeredRules(ask);
  if (offer == null || grants.length === 0) return null;
  // The line Always allow writes is read before it is pressed (#5116).
  const always = grants.some((one) => one.id === ALWAYS_ALLOW) ? ask.run?.always : undefined;
  return (
    <MessageForm>
      <p className="inbox-secondary ui-text-secondary">{offerSaid(offer.times)}</p>
      {always != null && ask.run != null && (
        <>
          <p className="inbox-secondary ui-text-secondary">{alwaysLineSaid(ask.run.persona)}</p>
          <SecretAskToolLine text={always.line} />
        </>
      )}
      <ButtonRow size="sm" role="group" aria-label={`Make a rule for ${who}`}>
        {grants.map((one) => (
          <FixWay
            key={one.id}
            fix={{ label: one.label, allows: true, busy, onPress: () => onMake(one) }}
            mayAct={mayAct}
            variant="grants"
          />
        ))}
      </ButtonRow>
    </MessageForm>
  );
}

/** **What the core is told of a rule made from `ask`'s offer with `option`.** */
export function ruleMadeOf(ask: Shown, option: Offered): RuleMade {
  const path = ask.answer;
  return {
    session: ask.session,
    request: ask.rule?.request ?? "",
    source: ask.source,
    says: ask.says,
    option: option.id,
    label: option.label,
    scope: option.scope ?? "chat",
    what: path.via === "sandbox-block" ? path.shown.what : null,
    target: path.via === "sandbox-block" ? path.shown.target : null,
    // The line Always allow wrote, for Undo to take out (#6177).
    declared:
      option.id === ALWAYS_ALLOW && ask.run?.always != null
        ? { persona: ask.run.persona, line: ask.run.always.line }
        : null,
  };
}

/** What a cool-off's line says: whose asks, and until when. */
export function coolingSaid(cooling: readonly Shown[], until: number | null, now: Date): string {
  const chats = [...new Set(cooling.map((one) => chainSaid(one.chain) || "A chat"))];
  const whose = chats.length === 1 ? chats[0] : `${chats.length} chats`;
  const held = counted(cooling.length, "ask", "asks");
  const when = until === null ? "" : ` until ${backAtSaid(until, now)}`;
  return `${whose} heard Don't allow twice: ${held} held quietly${when}.`;
}

/** **The cool-off line at the head of Asks**, while any ask is held in one. */
export function CoolingLine({
  plane,
  cooling,
  until,
}: {
  plane: string;
  cooling: readonly Shown[];
  until: number | null;
}) {
  const [trouble, setTrouble] = useState<string>();
  if (cooling.length === 0) return null;
  const end = async () => {
    const ended = await Promise.all(
      cooling.flatMap((one) =>
        one.rule == null ? [] : [commands.endCoolOff(plane, one.session, one.rule.request)],
      ),
    ).catch((err: unknown) => [{ status: "error" as const, error: String(err) }]);
    const failed = ended.find((one) => one.status === "error");
    setTrouble(failed?.status === "error" ? failed.error : undefined);
    asksMoved(plane);
  };
  return (
    <div className="inbox-snooze-line">
      <p className="inbox-none">{trouble ?? coolingSaid(cooling, until, new Date())}</p>
      <Button size="sm" onPress={() => void end()}>
        {END_COOL_OFF}
      </Button>
    </div>
  );
}

/** What a rule's scope says in the list. */
const SCOPE_SAID: Record<RuleMadeShown["scope"], string> = {
  chat: "this chat",
  project: "this project",
  machine: "this machine",
};

/** One rule's line in the list. */
export function ruleSaid(rule: RuleMadeShown): string {
  const saved = rule.saved === 0 ? "" : ` · saved ${counted(rule.saved, "ask", "asks")}`;
  return `${rule.label} · ${SCOPE_SAID[rule.scope]} in ${rule.project}${saved}`;
}

/** **Rules I made**: the link at the head of Asks, and the list it opens. */
export function RulesIMade() {
  const [open, setOpen] = useState(false);
  const [rules, setRules] = useState<readonly RuleMadeShown[]>();
  const [trouble, setTrouble] = useState<string>();
  /** Read on drawing, and again each time an answer moved Asks: one may have made a rule. */
  useEffect(() => {
    let gone = false;
    const read = () =>
      void commands
        .rulesMade()
        .then((listed) => {
          // A core that lists nothing (an older one, a test's stand-in) has made no rule.
          if (!gone && listed?.status === "ok" && Array.isArray(listed.data)) setRules(listed.data);
        })
        .catch(() => undefined);
    read();
    const stop = onAsksMoved(read);
    return () => {
      gone = true;
      stop();
    };
  }, []);
  const undo = async (id: string) => {
    const left = await commands.undoRule(id).catch((err: unknown) => ({
      status: "error" as const,
      error: String(err),
    }));
    if (left.status === "error") {
      setTrouble(left.error);
      return;
    }
    setTrouble(undefined);
    setRules(left.data);
  };
  // **Linked once there is a rule to review** (#6010): or while its list is open, after an Undo.
  if (!open && (rules === undefined || rules.length === 0)) return null;
  return (
    <>
      <Button size="sm" variant="quiet" onPress={() => setOpen(true)}>
        {RULES_I_MADE}
      </Button>
      <Dialog
        isOpen={open}
        onClose={() => setOpen(false)}
        title={RULES_I_MADE}
        initialFocus="cancel"
        cancel={{ children: "Close" }}
        summary={
          trouble !== undefined && (
            <span className="ui-text-secondary" role="status">
              {trouble}
            </span>
          )
        }
      >
        {rules !== undefined && <RulesList rules={rules} onUndo={(id) => void undo(id)} />}
      </Dialog>
    </>
  );
}

/** **The rules made, each with Undo**: what Rules I made draws in its dialog. */
export function RulesList({
  rules,
  onUndo,
}: {
  rules: readonly RuleMadeShown[];
  onUndo: (id: string) => void;
}) {
  if (rules.length === 0)
    return (
      <p className="ui-text-secondary">
        No rules yet. After you give the same Allow twice, Asks offers to make one.
      </p>
    );
  return (
    <ul className="ask-rules" aria-label={RULES_I_MADE}>
      {rules.map((rule) => (
        <li key={rule.id} className="ask-rule">
          <span className="ask-rule-says ui-text-body">{rule.says}</span>
          <span className="ask-rule-what ui-text-secondary">{ruleSaid(rule)}</span>
          <Button
            size="sm"
            aria-label={`Undo ${rule.label}: ${rule.says}`}
            onPress={() => onUndo(rule.id)}
          >
            Undo
          </Button>
        </li>
      ))}
    </ul>
  );
}
