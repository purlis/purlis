import { useState } from "react";

import { closedBy, type Closed, type OpenAsk } from "./timeline.ts";

/**
 * The ask sheet (iOS design §6.1, D8): the summary, then the choices in the source's order
 * and words, none that lasts. Disabled while the desktop is away (§6.3). An ask that closed
 * under the sheet says who answered, and a tap dismisses it.
 */
export function AskSheet({
  ask,
  closed,
  connected,
  onAnswer,
  onDismiss,
}: {
  ask: OpenAsk | null;
  closed: Closed | null;
  connected: boolean;
  onAnswer: (ask: string, choice: string) => Promise<void>;
  onDismiss: () => void;
}) {
  const [sending, setSending] = useState(false);
  const [refused, setRefused] = useState<string | null>(null);

  if (closed && (!ask || ask.ask === closed.ask)) {
    return (
      <div className="sheet" role="dialog" aria-label="Ask closed">
        <p>{closedBy(closed.by)}</p>
        <button type="button" className="primary" onClick={onDismiss}>
          OK
        </button>
      </div>
    );
  }
  if (!ask) return null;

  async function choose(choice: string) {
    if (!ask) return;
    setSending(true);
    setRefused(null);
    try {
      await onAnswer(ask.ask, choice);
    } catch (err) {
      setRefused(err instanceof Error ? err.message : String(err));
    } finally {
      setSending(false);
    }
  }

  return (
    <div className="sheet" role="dialog" aria-label="Ask">
      <p className="summary">{ask.summary}</p>
      {ask.elicitsSecret ? (
        <p className="secret">This ask wants a secret. Answer it on the desktop.</p>
      ) : (
        <div className="choices">
          {ask.choices.map((choice) => (
            <button
              key={choice.id}
              type="button"
              disabled={!connected || sending}
              onClick={() => choose(choice.id)}
            >
              {choice.label}
            </button>
          ))}
        </div>
      )}
      {!connected && <p className="standing">Reconnecting… answer when the desktop is back.</p>}
      {refused && (
        <p className="refused" role="alert">
          {refused}
        </p>
      )}
    </div>
  );
}
