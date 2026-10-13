import { useState } from "react";

import { said, type DesktopRow, type PhoneInfo } from "./ipc.ts";

/**
 * Settings (iOS design §6.1): the paired desktops (rename, unpair, where else to reach
 * them), the lock, this phone's name, and the key every pairing rests on.
 */
export function Settings({
  phone,
  desktops,
  lockOn,
  onLock,
  onRenamePhone,
  onRenameDesktop,
  onSetAddresses,
  onUnpair,
  onForgetKey,
  onBack,
}: {
  phone: PhoneInfo | null;
  desktops: DesktopRow[];
  lockOn: boolean;
  onLock: (on: boolean) => void;
  onRenamePhone: (name: string) => Promise<void>;
  onRenameDesktop: (device: string, name: string) => Promise<void>;
  onSetAddresses: (device: string, addresses: string[]) => Promise<void>;
  onUnpair: (device: string) => Promise<void>;
  onForgetKey: () => Promise<void>;
  onBack: () => void;
}) {
  const [forgetting, setForgetting] = useState(false);
  const [trouble, setTrouble] = useState<string | null>(null);

  /** A change the Rust side may refuse: the refusal is shown, and handed back to the field. */
  const tried = <T,>(work: Promise<T>): Promise<T> =>
    work.then(
      (done) => {
        setTrouble(null);
        return done;
      },
      (err) => {
        setTrouble(said(err));
        throw err;
      },
    );

  return (
    <main className="screen settings">
      <header className="bar">
        <button type="button" className="back" onClick={onBack}>
          Back
        </button>
        <h1>Settings</h1>
      </header>
      {trouble && (
        <p className="refused" role="alert">
          {trouble}
        </p>
      )}

      <section aria-labelledby="desktops-heading">
        <h2 id="desktops-heading">Paired desktops</h2>
        {desktops.length === 0 && <p className="empty">No desktop is paired.</p>}
        {desktops.map((desktop) => (
          <Desktop
            key={desktop.device}
            desktop={desktop}
            onRename={(name) => tried(onRenameDesktop(desktop.device, name))}
            onSetAddresses={(addresses) => tried(onSetAddresses(desktop.device, addresses))}
            onUnpair={() => tried(onUnpair(desktop.device))}
          />
        ))}
      </section>

      <section aria-labelledby="lock-heading">
        <h2 id="lock-heading">Lock</h2>
        <label className="toggle">
          <input type="checkbox" checked={lockOn} onChange={(e) => onLock(e.target.checked)} />
          Lock with Face ID
        </label>
      </section>

      <section aria-labelledby="phone-heading">
        <h2 id="phone-heading">This phone</h2>
        <Named
          label="This phone's name"
          name={phone?.name ?? ""}
          onRename={(name) => tried(onRenamePhone(name))}
        />
        {phone && (
          <p className="fingerprint">
            Key <code>{phone.fingerprint}</code>
          </p>
        )}
        {forgetting ? (
          <p className="ending">
            Every desktop will have to pair again.
            <button
              type="button"
              className="danger"
              onClick={() => {
                setForgetting(false);
                tried(onForgetKey()).catch(() => {});
              }}
            >
              Forget the key
            </button>
            <button type="button" onClick={() => setForgetting(false)}>
              Keep it
            </button>
          </p>
        ) : (
          <button type="button" className="danger" onClick={() => setForgetting(true)}>
            Forget this phone's key
          </button>
        )}
      </section>
    </main>
  );
}

function Desktop({
  desktop,
  onRename,
  onSetAddresses,
  onUnpair,
}: {
  desktop: DesktopRow;
  onRename: (name: string) => Promise<void>;
  onSetAddresses: (addresses: string[]) => Promise<void>;
  onUnpair: () => Promise<void>;
}) {
  const [addresses, setAddresses] = useState(desktop.addresses.join("\n"));
  const [unpairing, setUnpairing] = useState(false);
  return (
    <div className="desktop-row" aria-label={desktop.name}>
      <Named label="Rename" name={desktop.name} onRename={onRename} />
      <label>
        Also reach it at
        <textarea
          aria-label={`Also reach ${desktop.name} at`}
          rows={2}
          value={addresses}
          autoCapitalize="none"
          autoCorrect="off"
          spellCheck={false}
          onChange={(e) => setAddresses(e.target.value)}
          onBlur={() => {
            const list = addresses
              .split(/[\n,]/)
              .map((one) => one.trim())
              .filter(Boolean);
            if (list.join("\n") !== desktop.addresses.join("\n")) {
              onSetAddresses(list).catch(() => setAddresses(desktop.addresses.join("\n")));
            }
          }}
        />
      </label>
      {unpairing ? (
        <p className="ending">
          Unpair {desktop.name}?
          <button
            type="button"
            className="danger"
            onClick={() => onUnpair().catch(() => setUnpairing(false))}
          >
            Unpair
          </button>
          <button type="button" onClick={() => setUnpairing(false)}>
            Keep
          </button>
        </p>
      ) : (
        <button type="button" className="danger" onClick={() => setUnpairing(true)}>
          Unpair
        </button>
      )}
    </div>
  );
}

/** A name that commits on blur or Enter, the way the desktop's list fields do. */
function Named({
  label,
  name,
  onRename,
}: {
  label: string;
  name: string;
  onRename: (name: string) => Promise<void>;
}) {
  const [draft, setDraft] = useState(name);
  const commit = () => {
    const next = draft.trim();
    if (next && next !== name) onRename(next).catch(() => setDraft(name));
    else setDraft(name);
  };
  return (
    <label>
      {label}
      <input
        aria-label={label}
        value={draft}
        onChange={(e) => setDraft(e.target.value)}
        onBlur={commit}
        onKeyDown={(e) => {
          if (e.key === "Enter") (e.target as HTMLInputElement).blur();
        }}
      />
    </label>
  );
}
