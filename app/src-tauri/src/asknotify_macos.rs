// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **macOS's own notifications, replaced and taken away in place** (#6059):
//! `UNUserNotificationCenter`. A notification posted with an `identifier` already delivered
//! takes its place in Notification Center, and `removeDeliveredNotifications(withIdentifiers:)`
//! takes it away. The notification plugin posts through the older `NSUserNotificationCenter`
//! under an identifier of its own making, so it can do neither.
//!
//! **Only from an app bundle.** `UNUserNotificationCenter` raises when the process is no
//! `.app` (a `tauri dev` build), so there [`UserNotifications::here`] says none and the
//! plugin's path stays.
//!
//! **Asked for once.** The system asks the person whether purlis may notify the first time it
//! is asked ([`ASKED`]); after that the answer is the system's notification settings' to keep.
//! No delegate is set, so a notification posted while purlis is the frontmost app goes to
//! Notification Center without a banner, as the plugin's did.

use std::sync::Once;

use block2::RcBlock;
use objc2::rc::Retained;
use objc2::runtime::Bool;
use objc2_foundation::{NSArray, NSBundle, NSError, NSString};
use objc2_user_notifications::{
    UNAuthorizationOptions, UNMutableNotificationContent, UNNotificationRequest,
    UNNotificationSound, UNUserNotificationCenter,
};

use super::Notice;
use super::center::Center;

/// Whether purlis asked the system yet whether it may notify.
static ASKED: Once = Once::new();

/// The identifier notification `id` is posted and taken away under.
fn identifier(id: i32) -> Retained<NSString> {
    NSString::from_str(&format!("dev.purlis.ask.{id}"))
}

/// macOS's notification center, as the notifier uses it.
pub struct UserNotifications(Retained<UNUserNotificationCenter>);

impl UserNotifications {
    /// The center, where this process is an app bundle; else none.
    pub fn here() -> Option<Self> {
        let bundle = NSBundle::mainBundle();
        if !bundle.bundlePath().to_string().ends_with(".app") || bundle.bundleIdentifier().is_none()
        {
            return None;
        }
        let center = UNUserNotificationCenter::currentNotificationCenter();
        ASKED.call_once(|| {
            let answered = RcBlock::new(|_granted: Bool, _error: *mut NSError| {});
            center.requestAuthorizationWithOptions_completionHandler(
                UNAuthorizationOptions::Alert | UNAuthorizationOptions::Sound,
                &answered,
            );
        });
        Some(Self(center))
    }
}

impl Center for UserNotifications {
    fn replaces(&self) -> bool {
        true
    }

    fn show(&self, notice: &Notice) {
        let content = UNMutableNotificationContent::new();
        content.setTitle(&NSString::from_str(&notice.title));
        content.setBody(&NSString::from_str(&notice.body));
        // The Expiring sound (#6110): the system's own notification sound, which the person's
        // notification settings for purlis can still turn off.
        if notice.sound {
            content.setSound(Some(&UNNotificationSound::defaultSound()));
        }
        let request = UNNotificationRequest::requestWithIdentifier_content_trigger(
            &identifier(notice.id),
            &content,
            None,
        );
        self.0
            .addNotificationRequest_withCompletionHandler(&request, None);
    }

    fn remove(&self, id: i32) {
        self.0
            .removeDeliveredNotificationsWithIdentifiers(&NSArray::from_retained_slice(&[
                identifier(id),
            ]));
    }
}
