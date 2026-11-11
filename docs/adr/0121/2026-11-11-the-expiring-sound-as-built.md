# Amended 2026-11-11: the Expiring sound, as built

Decision 12's one sound ("an optional one for an Expiring ask about to fail") is built (#6110),
in the notifier (`app/src-tauri/src/asknotify.rs`, `Rules::expiring`), behind decision 22's
setting, Settings › You › Attention › "Sound for an expiring ask" (`notifications.expiringSound`,
#6019, off by default). These particulars are decided here.

- **"About to fail" is the last 15 seconds** of an Expiring ask's countdown (`SOUND_BEFORE`): an
  ask whose `Shown.expires` is still ahead and at most 15 seconds away. The shortest countdown a
  person can still beat is a permission prompt's, about 58 seconds, and its row counts in seconds
  below a minute; 15 is time enough to reach the window and answer. An ask first heard with less
  than that left (a run grant on Codex, 7.5 s) sounds as it is heard. The notifier wakes by
  itself at the moment an ask comes within 15 seconds, and reads the project again then.
- **Once per ask**, by its key, kept with the notifier's other per-ask memory and let go of once
  the ask is no longer listed; one sound per chat, however many of its asks are nearly out.
- **Never while Quiet** (decision 13), and **never while the person is looking at its chat**
  (decision 11): the sound rides a notification, and neither sends one. An ask Quiet holds open
  has no countdown (this ADR's amendment on timed-out asks), so it is not Expiring and cannot
  sound. A snoozed chat's asks are not counted, so they do not sound either (decision 14).
- **The sound is the system's own notification sound, on the chat's notification**: its own, or
  the burst's that counts it, shown again in place with the sound; a chat no notification stands
  for is told now, with it. No audio file is bundled and no crate is added.
  - **macOS** (an app bundle): `UNNotificationSound.defaultSound` on the notification's content,
    through `objc2-user-notifications`, already a dependency (one more feature of it). purlis
    already asked the system for the sound authorisation, so the person's notification settings
    for purlis can still turn it off.
  - **Linux and the BSDs**: the notification plugin's sound, which notify-rust sends as the
    freedesktop `sound-name` hint, `dialog-warning` from the freedesktop sound theme. A
    notification server that plays no sounds ignores it; nothing else plays one.
  - **macOS without an app bundle (`tauri dev`) and Windows**: the plugin's sound, by the
    platform's default name (`NSUserNotificationDefaultSoundName`, `Default`).
- **Nothing new is stored**: the memory of what sounded is in the notifier's thread only, so no
  tier is named (ADR 0069).
