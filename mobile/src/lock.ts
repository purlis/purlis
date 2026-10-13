// The lock (iOS design §6.2): on every open, and after five minutes in the background. The
// biometric check itself is one function a test replaces.

/** How long the app may be hidden before it locks again. */
export const LOCK_AFTER_MS = 5 * 60_000;

export type LockState = {
  locked: boolean;
  /** When the app went to the background, while it is there. */
  hiddenAt: number | null;
};

/** At launch: locked, until the check passes. */
export function atLaunch(): LockState {
  return { locked: true, hiddenAt: null };
}

export function hidden(state: LockState, now: number): LockState {
  return { ...state, hiddenAt: state.hiddenAt ?? now };
}

/** Back in the foreground: locked again when it was away for long enough. */
export function shown(state: LockState, now: number): LockState {
  const away = state.hiddenAt === null ? 0 : now - state.hiddenAt;
  return { locked: state.locked || away >= LOCK_AFTER_MS, hiddenAt: null };
}

export function unlocked(state: LockState): LockState {
  return { ...state, locked: false };
}

/** Whether the person is who the phone says: the biometric plugin, or the device passcode
 *  where there is no biometry. `false` is a failed or cancelled check. */
export type Checker = () => Promise<boolean>;

export const biometricCheck: Checker = async () => {
  let biometric: typeof import("@tauri-apps/plugin-biometric");
  let status: Awaited<ReturnType<typeof biometric.checkStatus>>;
  try {
    biometric = await import("@tauri-apps/plugin-biometric");
    status = await biometric.checkStatus();
  } catch {
    // No plugin: a desktop build, for development, which has no lock to pass.
    return true;
  }
  // A phone with no passcode has nothing to check against. Face ID denied or not enrolled
  // still has the passcode, and the check below falls back to it.
  if (!status.isAvailable && status.errorCode === "passcodeNotSet") return true;
  try {
    await biometric.authenticate("Unlock purlis", {
      allowDeviceCredential: true,
      cancelTitle: "Cancel",
      fallbackTitle: "Use passcode",
    });
    return true;
  } catch {
    return false;
  }
};
