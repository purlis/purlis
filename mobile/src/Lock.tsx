import { useEffect, useState } from "react";

import { biometricCheck, type Checker } from "./lock.ts";

/**
 * The lock screen (iOS design §6.2): the check runs as soon as the screen shows; a failed or
 * cancelled check keeps the screen, with Try again as its way out.
 */
export function Lock({
  onUnlocked,
  check = biometricCheck,
}: {
  onUnlocked: () => void;
  check?: Checker;
}) {
  const [failed, setFailed] = useState(false);
  const [attempt, setAttempt] = useState(0);

  useEffect(() => {
    let stale = false;
    setFailed(false);
    check().then((ok) => {
      if (stale) return;
      if (ok) onUnlocked();
      else setFailed(true);
    });
    return () => {
      stale = true;
    };
  }, [attempt, check, onUnlocked]);

  return (
    <main className="screen lock">
      <h1>purlis</h1>
      {failed ? (
        <>
          <p>Unlock to see your chats.</p>
          <button type="button" className="primary" onClick={() => setAttempt((n) => n + 1)}>
            Try again
          </button>
        </>
      ) : (
        <p>Unlocking…</p>
      )}
    </main>
  );
}
