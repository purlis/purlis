// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/** PROTOTYPE, throwaway (spec #4469). The shape every step shares, on the real UI kit. */
import { useEffect, useId, useRef, useState, type ReactNode } from "react";
import {
  AlertTriangle,
  Check,
  Circle,
  Clock,
  Copy,
  ExternalLink,
  HelpCircle,
  Loader2,
  Minus,
  X,
} from "lucide-react";
import { Button, ButtonRow } from "../../ui/button/Button";
import { Disclosure } from "../../ui/disclosure/Disclosure";
import { NoticeBox, NoticeMark, NoticeMessage, NoticeTitle } from "../../ui/notice/Notice";

export const Out = () => <ExternalLink aria-hidden size={14} />;

/** The page's heading and its one or two plain sentences. Takes the focus as a step opens. */
export function StepHead({ title, lead, tag }: { title: string; lead?: ReactNode; tag?: ReactNode }) {
  const ref = useRef<HTMLHeadingElement>(null);
  useEffect(() => {
    if (document.activeElement?.closest("[data-ps-keep]")) return;
    ref.current?.focus({ preventScroll: true });
  }, [title]);
  return (
    <header className="ps-head">
      <div className="ps-title-row">
        <h1 ref={ref} tabIndex={-1} className="ps-title">
          {title}
        </h1>
        {tag}
      </div>
      {lead && <p className="ps-lead">{lead}</p>}
    </header>
  );
}

/** A word explained where it first appears (Q24): a sentence on the page, never a tooltip. */
export function Terms({ children }: { children: ReactNode }) {
  return <div className="ps-terms">{children}</div>;
}
export function Term({ word, children }: { word: string; children: ReactNode }) {
  return (
    <p className="ps-term">
      <strong>{word}</strong> {children}
    </p>
  );
}

/** One main action, and what it will do, said before it is pressed (Q31). */
export function Action({
  does,
  main,
  onMain,
  busy,
  disabled,
  children,
  mainIcon,
}: {
  does?: ReactNode;
  main?: string;
  onMain?: () => void;
  busy?: boolean;
  disabled?: boolean;
  children?: ReactNode;
  mainIcon?: ReactNode;
}) {
  return (
    <section className="ps-action" aria-label="What to do next">
      {does && <p className="ps-does">{does}</p>}
      <ButtonRow size="lg">
        {main && (
          <Button variant="primary" onPress={onMain} isDisabled={disabled} aria-busy={busy} icon={mainIcon}>
            {main}
          </Button>
        )}
        {children}
      </ButtonRow>
    </section>
  );
}

/** Everything you can set, the recommended value marked (Q30). */
export function Options({ children, open }: { children: ReactNode; open?: boolean }) {
  return (
    <Disclosure label="Options" level={2} defaultExpanded={open} className="ps-fold">
      <div className="ps-fold-body">{children}</div>
    </Disclosure>
  );
}

export const Recommended = () => <span className="ps-rec">Recommended</span>;

function CopyButton({ text, what = "Copy" }: { text: string; what?: string }) {
  const [said, setSaid] = useState(false);
  useEffect(() => {
    if (!said) return;
    const later = setTimeout(() => setSaid(false), 1800);
    return () => clearTimeout(later);
  }, [said]);
  return (
    <Button
      size="sm"
      variant="ghost"
      icon={said ? <Check aria-hidden size={14} /> : <Copy aria-hidden size={14} />}
      onPress={() => {
        void navigator.clipboard?.writeText(text).catch(() => undefined);
        setSaid(true);
      }}
    >
      {said ? "Copied" : what}
    </Button>
  );
}

/** Text to copy: a command, YAML, a tool line. */
export function Code({ text, label, copy, wraps }: { text: string; label?: string; copy?: string; wraps?: boolean }) {
  const id = useId();
  return (
    <figure className="ps-code" aria-labelledby={label ? id : undefined}>
      <figcaption className="ps-code-head">
        <span id={id}>{label ?? ""}</span>
        <CopyButton text={text} what={copy} />
      </figcaption>
      <pre tabIndex={0} aria-label={label ?? "Commands"} data-wraps={wraps ? "" : undefined}>
        <code>{text}</code>
      </pre>
    </figure>
  );
}

/** The exact commands, and that running them yourself counts (Q30). */
export function SeeWhat({
  text,
  note,
  open,
  label,
}: {
  text: string;
  note?: string;
  open?: boolean;
  label?: string;
}) {
  return (
    <Disclosure label="See what this does" level={2} defaultExpanded={open} className="ps-fold">
      <div className="ps-fold-body">
        <p className="ps-note">
          {note ?? "This is exactly what the button runs. Run it yourself if you prefer, and purlis marks this step done."}
        </p>
        <Code text={text} label={label ?? "Commands"} />
      </div>
    </Disclosure>
  );
}

/** Words on what happened and the one thing to try (Q33). */
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
        {children && <div className="ps-say">{children}</div>}
        {actions && (
          <div className="ps-say-actions">
            <ButtonRow size="md">{actions}</ButtonRow>
          </div>
        )}
      </NoticeMessage>
    </NoticeBox>
  );
}

/** A line of a check: its mark, which is never colour alone, and its words. */
export type Verdict = "ok" | "missing" | "running" | "todo" | "warn" | "skip";

const MARK: Record<Verdict, { icon: ReactNode; said: string }> = {
  ok: { icon: <Check aria-hidden size={16} />, said: "Passed" },
  missing: { icon: <X aria-hidden size={16} />, said: "Missing" },
  running: { icon: <Loader2 aria-hidden size={16} className="ps-spin" />, said: "Running" },
  todo: { icon: <Circle aria-hidden size={10} />, said: "Not yet" },
  warn: { icon: <AlertTriangle aria-hidden size={16} />, said: "Needs a look" },
  skip: { icon: <Minus aria-hidden size={16} />, said: "Skipped" },
};

export function Checks({ label, children }: { label: string; children: ReactNode }) {
  return (
    <ul className="ps-checks" aria-label={label}>
      {children}
    </ul>
  );
}
export function CheckLine({
  verdict,
  title,
  children,
  trail,
}: {
  verdict: Verdict;
  title: ReactNode;
  children?: ReactNode;
  trail?: ReactNode;
}) {
  const mark = MARK[verdict];
  return (
    <li className="ps-check" data-verdict={verdict}>
      <span className="ps-check-mark">
        {mark.icon}
        <span className="ps-vh">{mark.said}: </span>
      </span>
      <div className="ps-check-words">
        <div className="ps-check-title">{title}</div>
        {children && <div className="ps-check-more">{children}</div>}
      </div>
      {trail && <div className="ps-check-trail">{trail}</div>}
    </li>
  );
}

/** A recheck that runs by itself (Q28, Q38), said once and politely. */
export function Rechecking({ children }: { children: ReactNode }) {
  return (
    <p className="ps-recheck" role="status">
      <Loader2 aria-hidden size={14} className="ps-spin" />
      <span>{children}</span>
    </p>
  );
}

export function Busy({ children }: { children: ReactNode }) {
  return (
    <p className="ps-busy" role="status">
      <Loader2 aria-hidden size={18} className="ps-spin" />
      <span>{children}</span>
    </p>
  );
}

export const StateIcon = {
  done: <Check aria-hidden size={14} strokeWidth={3} />,
  attention: <AlertTriangle aria-hidden size={14} />,
  skipped: <Minus aria-hidden size={14} strokeWidth={3} />,
  waiting: <Clock aria-hidden size={14} />,
  help: <HelpCircle aria-hidden size={14} />,
};
