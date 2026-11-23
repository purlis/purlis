// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useState } from "react";
import { commands, type RuleMadeShown, type Shown } from "./bindings";
import { asksMoved, onAsksMoved } from "./asks";
import { backAtSaid } from "./AsksSnooze";
import { chainSaid } from "./inboxRowParts";
import { counted } from "./dispatches";
import { Button } from "./ui/button/Button";
import { Dialog } from "./ui/dialog/Dialog";

/**
 * **Ask less** (ADR 0121 decision 7, #6010), as Asks draws it:
 *
 * - **No rule is offered** (M75 slice 19, Q314.15): a chat is no longer offered a rule after the
 *   same Allow twice. What stays of a rule made before is listed in Rules I made.
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
