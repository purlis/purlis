// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **What every step of Pool setup shares** (spec #4469, Q30): its heading, its one main action and
 * what that does, Options, See what this does, and a line of what happened. Drawn from the kit, so
 * a later step only says what it says.
 */
import { useEffect, useRef, type ReactNode } from "react";
import { Loader2 } from "lucide-react";
import { Badge } from "../ui/badge/Badge";
import { Button, ButtonRow } from "../ui/button/Button";
import { CodeBlock } from "../ui/code/CodeBlock";
import { Disclosure } from "../ui/disclosure/Disclosure";
import { Reserved } from "../ui/dialog/Reserved";
import { NoticeBox, NoticeMark, NoticeMessage, NoticeTitle } from "../ui/notice/Notice";

/** A step's heading and its plain sentences. Takes the focus as a step opens. */
export function StepHead({
  title,
  lead,
  tag,
}: {
  title: string;
  lead?: ReactNode;
  tag?: ReactNode;
}) {
  const ref = useRef<HTMLHeadingElement>(null);
  useEffect(() => {
    if (document.activeElement?.closest('[data-scope="steps"]')) return;
    ref.current?.focus({ preventScroll: true });
  }, [title]);
  return (
    <header className="pool-setup-head">
      <div className="pool-setup-title-row">
        <h1 ref={ref} tabIndex={-1} className="pool-setup-title ui-text-title">
          {title}
        </h1>
        {tag}
      </div>
      {lead && <p className="pool-setup-lead">{lead}</p>}
    </header>
  );
}

/** One main action, and what it will do, said before it is pressed (Q31). */
export function Action({
  does,
  main,
  onMain,
  busy,
  icon,
  reserve,
  children,
}: {
  does?: string;
  main?: string;
  /** Every word the main button can say, so changing them moves nothing beside it. */
  reserve?: readonly string[];
  onMain?: () => void;
  busy?: boolean;
  icon?: ReactNode;
  /** What stands beside it: Skip for now. */
  children?: ReactNode;
}) {
  return (
    <section className="pool-setup-action" aria-label="What to do next">
      {does && <p className="pool-setup-does">{does}</p>}
      {(main || children) && (
        <ButtonRow size="lg">
          {main && (
            <Button
              variant="primary"
              onPress={onMain}
              isDisabled={busy}
              aria-busy={busy}
              icon={icon}
            >
              {reserve ? <Reserved texts={reserve}>{main}</Reserved> : main}
            </Button>
          )}
          {children}
        </ButtonRow>
      )}
    </section>
  );
}

/** Words that take their time, said once and politely. */
export function Busy({ children }: { children: ReactNode }) {
  return (
    <p className="pool-setup-busy" role="status">
      <Loader2 aria-hidden size={18} className="spinning" />
      <span>{children}</span>
    </p>
  );
}

/** The mark of the value purlis recommends (Q30). */
export const Recommended = () => <Badge variant="accent">Recommended</Badge>;

/** **Everything that can be set**, each with the recommended value marked (Q30). */
export function Options({ open, children }: { open?: boolean; children: ReactNode }) {
  return (
    <Disclosure label="Options" level={2} defaultExpanded={open} className="pool-setup-fold">
      <div className="pool-setup-fold-body">{children}</div>
    </Disclosure>
  );
}

/** What See what this does says of commands that do what the button does. */
export const RUN_IT_YOURSELF =
  "This is exactly what the button runs. Run it yourself if you prefer, and purlis marks this step done.";

/** **The exact commands**, and what running them means (Q30). */
export function SeeWhat({
  text,
  note = RUN_IT_YOURSELF,
  open,
}: {
  text: string;
  note?: string;
  open?: boolean;
}) {
  return (
    <Disclosure
      label="See what this does"
      level={2}
      defaultExpanded={open}
      className="pool-setup-fold"
    >
      <div className="pool-setup-fold-body">
        <p className="pool-setup-does">{note}</p>
        {text !== "" && <CodeBlock label="Commands" text={text} />}
      </div>
    </Disclosure>
  );
}

/** Words on what happened, and the one thing to try (Q33), in a step, where nothing is dismissed. */
export function Say({
  tone,
  title,
  children,
  actions,
}: {
  tone: "news" | "trouble" | "attention";
  title: string;
  children?: ReactNode;
  actions?: ReactNode;
}) {
  return (
    <NoticeBox layout="card" tone={tone} placement="alone" opened={false} asks={false}>
      <NoticeMark tone={tone} />
      <NoticeMessage>
        <NoticeTitle>{title}</NoticeTitle>
        {children && <div className="pool-setup-say">{children}</div>}
        {actions && (
          <div className="pool-setup-say-actions">
            <ButtonRow size="md">{actions}</ButtonRow>
          </div>
        )}
      </NoticeMessage>
    </NoticeBox>
  );
}
