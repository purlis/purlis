import { useState } from "react";

/**
 * The composer (iOS design §6.1): Send is `prompt`. Disabled while the desktop is away, and
 * nothing is queued to be sent later (§6.3).
 */
export function Composer({
  connected,
  onSend,
}: {
  connected: boolean;
  onSend: (text: string) => Promise<void>;
}) {
  const [text, setText] = useState("");
  const [sending, setSending] = useState(false);
  const ready = connected && !sending && text.trim().length > 0;

  async function send() {
    if (!ready) return;
    setSending(true);
    try {
      await onSend(text);
      setText("");
    } finally {
      setSending(false);
    }
  }

  return (
    <form
      className="composer"
      onSubmit={(e) => {
        e.preventDefault();
        void send();
      }}
    >
      <textarea
        aria-label="Message"
        placeholder="Message"
        rows={1}
        value={text}
        disabled={!connected}
        onChange={(e) => setText(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === "Enter" && !e.shiftKey) {
            e.preventDefault();
            void send();
          }
        }}
      />
      <button type="submit" className="primary" disabled={!ready}>
        Send
      </button>
    </form>
  );
}
