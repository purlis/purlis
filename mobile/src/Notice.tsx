import type { DesktopRow } from "./ipc.ts";

/**
 * The connection's standing line (iOS design §6.3): Connected, Reconnecting…, or the desktop
 * unreachable with why, each with Retry as its way out.
 */
export function Notice({ desktop, onRetry }: { desktop: DesktopRow; onRetry: () => void }) {
  const line =
    desktop.standing === "connected"
      ? "Connected"
      : desktop.standing === "reconnecting"
        ? "Reconnecting…"
        : desktop.standing === "unreachable"
          ? `${desktop.name} is unreachable${desktop.why ? `: ${desktop.why}` : ""}`
          : `${desktop.name} is not connected`;
  return (
    <div className={`notice notice-${desktop.standing}`} role="status">
      <span>{line}</span>
      {desktop.standing !== "connected" && (
        <button type="button" onClick={onRetry}>
          Retry
        </button>
      )}
    </div>
  );
}
