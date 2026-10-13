import { useState } from "react";

import { commands, said, type DesktopRow } from "./ipc.ts";

/** Read one QR code with the phone's camera; the text it carried, or nothing when cancelled. */
export type Scanner = () => Promise<string | null>;

export const cameraScan: Scanner = async () => {
  const scanner = await import("@tauri-apps/plugin-barcode-scanner");
  const allowed = await scanner.checkPermissions();
  if (allowed !== "granted" && (await scanner.requestPermissions()) !== "granted") return null;
  const scanned = await scanner.scan({ windowed: false, formats: [scanner.Format.QRCode] });
  return scanned.content || null;
};

/** A refusal as a line of its own: its first letter up. */
export function sentence(said: string): string {
  return said.charAt(0).toUpperCase() + said.slice(1);
}

/**
 * Pairing (iOS design §6.1): scan the desktop's code, or paste its text, then wait while the
 * person confirms on the desktop. A refusal is said in the desktop's words.
 */
export function Pair({
  onPaired,
  onBack,
  scan = cameraScan,
}: {
  onPaired: (desktop: DesktopRow) => void;
  /** Present when a desktop is already paired and this is one more. */
  onBack?: () => void;
  scan?: Scanner;
}) {
  const [text, setText] = useState("");
  const [pairing, setPairing] = useState(false);
  const [refused, setRefused] = useState<string | null>(null);

  async function pair(code: string) {
    setPairing(true);
    setRefused(null);
    try {
      onPaired(await commands.pair(code));
    } catch (err) {
      // The core says what went wrong in a sentence of its own: a refusal reads "the desktop
      // refused the pairing: …", a bad paste "not a purlis pairing code", a dead address its
      // timeout. Said as it is, with a capital.
      setRefused(sentence(said(err)));
      setPairing(false);
    }
  }

  async function scanOne() {
    setRefused(null);
    let code: string | null;
    try {
      code = await scan();
    } catch (err) {
      setRefused(`The camera could not scan: ${said(err)}`);
      return;
    }
    if (code) await pair(code);
  }

  return (
    <main className="screen pair">
      {onBack && (
        <button type="button" className="back" onClick={onBack} disabled={pairing}>
          Back
        </button>
      )}
      <h1>Pair a desktop</h1>
      {pairing ? (
        <p className="standing" role="status">
          Confirm on your desktop…
        </p>
      ) : (
        <>
          <p>Scan the code on your desktop</p>
          <button type="button" className="primary" onClick={scanOne}>
            Scan
          </button>
          <p>Or paste the pairing text</p>
          <form
            onSubmit={(e) => {
              e.preventDefault();
              if (text.trim()) void pair(text.trim());
            }}
          >
            <input
              aria-label="Pairing text"
              value={text}
              onChange={(e) => setText(e.target.value)}
              autoCapitalize="none"
              autoCorrect="off"
              spellCheck={false}
              placeholder="purlis-pair:…"
            />
            <button type="submit" disabled={!text.trim()}>
              Pair
            </button>
          </form>
        </>
      )}
      {refused && (
        <p className="refused" role="alert">
          {refused}
        </p>
      )}
    </main>
  );
}
