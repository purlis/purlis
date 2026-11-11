// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The system's notifications, as the notifier does an [`Act`] to them** (#6059): the one
//! platform layer under asknotify's rules.
//!
//! - **macOS**: its own notification center (`asknotify_macos.rs`), which replaces a
//!   notification in place by its identifier and takes one away by it. A build that is no app
//!   bundle (`tauri dev`) cannot reach it and takes the plugin's path.
//! - **Linux and the BSDs**: the notification plugin, which takes a notification away by its
//!   id, so an update is the old one taken away and the new one shown.
//! - **Windows**: the plugin, which can neither replace nor take one away, so an update that
//!   tells of no new chat is not shown ([`shown_here`]). purlis ships for macOS and Linux only;
//!   a Windows build would want the toast's `Tag` and `Group`.

use super::{Act, Notice, REPLACES_HERE, shown_here};

/// **A system's notifications**: what [`act`] tells them.
pub trait Center {
    /// Whether it replaces a notification shown, and takes one away, by its id.
    fn replaces(&self) -> bool;
    /// Shows `notice`; where it [`replaces`](Self::replaces), in place of the one of its id.
    fn show(&self, notice: &Notice);
    /// Takes notification `id` away.
    fn remove(&self, id: i32);
}

/// Does `act` to `center`, and says the notice it showed, if it showed one.
pub fn act(center: &dyn Center, act: Act) -> Option<Notice> {
    match act {
        Act::Remove(id) => {
            center.remove(id);
            None
        }
        Act::Show(notice) => shown_here(&notice, center.replaces()).then(|| {
            center.show(&notice);
            notice
        }),
    }
}

/// This system's notifications.
pub fn here(app: &tauri::AppHandle) -> Box<dyn Center + '_> {
    #[cfg(target_os = "macos")]
    if let Some(center) = super::macos::UserNotifications::here() {
        return Box::new(center);
    }
    Box::new(Plugin(app))
}

/// **The system's own notification sound, by the name the plugin passes on** (#6110): the
/// freedesktop sound theme's warning on Linux and the BSDs (the `sound-name` hint, which a
/// notification server without sounds ignores), and the default sound elsewhere: macOS's
/// `NSUserNotificationDefaultSoundName` (a build that is no app bundle) and Windows's `Default`.
const SOUND: &str = if cfg!(all(unix, not(target_os = "macos"))) {
    "dialog-warning"
} else if cfg!(target_os = "macos") {
    "NSUserNotificationDefaultSoundName"
} else {
    "Default"
};

/// The notification plugin's: it replaces and takes away by id on Linux and the BSDs only
/// ([`REPLACES_HERE`]).
struct Plugin<'a>(&'a tauri::AppHandle);

impl Center for Plugin<'_> {
    fn replaces(&self) -> bool {
        REPLACES_HERE
    }

    fn show(&self, notice: &Notice) {
        use tauri_plugin_notification::NotificationExt;
        // Best effort, always. A desktop that refuses notifications, or a person who turned
        // them off, is not a reason for anything else here to stop working.
        if notice.replaces {
            let _ = self.0.notification().remove_active(vec![notice.id]);
        }
        // No action type: a notification has no Allow or Deny of its own, which would answer
        // past the settle guard (ADR 0121 decision 12).
        let mut builder = self
            .0
            .notification()
            .builder()
            .id(notice.id)
            .title(notice.title.clone())
            .body(notice.body.clone());
        if notice.sound {
            builder = builder.sound(SOUND);
        }
        let _ = builder.show();
    }

    fn remove(&self, id: i32) {
        use tauri_plugin_notification::NotificationExt;
        let _ = self.0.notification().remove_active(vec![id]);
    }
}

#[cfg(test)]
#[path = "asknotify_center_tests.rs"]
mod tests;
