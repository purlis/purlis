//! One phone (iOS design §3): its stores, its identity, and a session per paired desktop,
//! with what each screen has open. Everything of substance is `purlis_mobile_core`'s; this
//! holds the handles the commands share.

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use purlis_mobile_core::Error;
use purlis_mobile_core::hosts;
use purlis_mobile_core::identity::{self, Device};
use purlis_mobile_core::session::{Session, Standing, Tcp};
use purlis_mobile_core::store::{FileStore, Store};
use purlis_session_protocol::noise::Identity;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

/// How long a dial waits on one address before trying the next.
const DIAL_TIMEOUT: Duration = Duration::from_secs(10);

/// A view the webview has open: the task feeding it, and where its acknowledgements go.
pub struct ViewOpen {
    pub view: u32,
    pub task: JoinHandle<()>,
    pub acks: mpsc::UnboundedSender<usize>,
}

/// A timeline the webview has open.
pub struct TimelineOpen {
    pub timeline: u32,
    pub task: JoinHandle<()>,
}

/// What runs for one desktop beside its session: the events feed and the standing watcher.
#[derive(Default)]
pub struct Following {
    pub events: Option<JoinHandle<()>>,
    pub standing: Option<JoinHandle<()>>,
}

pub struct Phone {
    secrets: Arc<dyn Store>,
    records: Arc<dyn Store>,
    /// The link key in hand: replaced in place when it is forgotten, so the next pairing is
    /// made with the new one and not with one the next launch will not find.
    me: Mutex<Identity>,
    device: Mutex<Device>,
    sessions: Mutex<HashMap<String, Arc<Session>>>,
    following: Mutex<HashMap<String, Following>>,
    views: Mutex<HashMap<(String, String), ViewOpen>>,
    timelines: Mutex<HashMap<(String, String), TimelineOpen>>,
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The link key's store: the Keychain on the phone, a private file in a desktop build (for
/// development: an unsigned Mac build cannot open the data-protection Keychain).
fn secrets_store(data_dir: &Path) -> Result<Arc<dyn Store>, Error> {
    #[cfg(target_os = "ios")]
    {
        let _ = data_dir;
        Ok(Arc::new(purlis_mobile_core::store::KeychainStore::new()?))
    }
    #[cfg(not(target_os = "ios"))]
    {
        Ok(Arc::new(FileStore::new(data_dir.join("purlis-secrets"))))
    }
}

impl Phone {
    /// The phone as its stores hold it, minting its key and id on the first launch.
    pub fn open(data_dir: &Path) -> Result<Phone, Error> {
        let records: Arc<dyn Store> = Arc::new(FileStore::new(data_dir.join("purlis")));
        let secrets = secrets_store(data_dir)?;
        let me = identity::link_key(&*secrets)?;
        let device = identity::device(&*records, None)?;
        Ok(Phone {
            secrets,
            records,
            me: Mutex::new(me),
            device: Mutex::new(device),
            sessions: Mutex::new(HashMap::new()),
            following: Mutex::new(HashMap::new()),
            views: Mutex::new(HashMap::new()),
            timelines: Mutex::new(HashMap::new()),
        })
    }

    pub fn me(&self) -> Identity {
        lock(&self.me).clone()
    }

    pub fn device(&self) -> Device {
        lock(&self.device).clone()
    }

    pub fn rename(&self, name: &str) -> Result<Device, Error> {
        let renamed = identity::rename(&*self.records, name)?;
        *lock(&self.device) = renamed.clone();
        Ok(renamed)
    }

    pub fn records(&self) -> &Arc<dyn Store> {
        &self.records
    }

    /// Forget the link key and mint the next one now: every pairing made with the old one is
    /// void, and a pairing made before the next launch is made with the key that launch
    /// will find.
    pub fn forget_link_key(&self) -> Result<(), Error> {
        identity::forget_link_key(&*self.secrets)?;
        let minted = identity::link_key(&*self.secrets)?;
        *lock(&self.me) = minted;
        Ok(())
    }

    /// Which connect made a desktop's link: 0 for one never dialled.
    pub fn generation_of(&self, device: &str) -> u64 {
        self.session_if_any(device)
            .map(|session| session.generation())
            .unwrap_or(0)
    }

    /// The session for a paired desktop, made on the first ask.
    pub fn session(&self, device: &str) -> Result<Arc<Session>, Error> {
        if let Some(session) = lock(&self.sessions).get(device) {
            return Ok(Arc::clone(session));
        }
        let host = hosts::find(&*self.records, device)?
            .ok_or_else(|| Error::Store(format!("no desktop {device} is paired")))?;
        let session = Arc::new(Session::new(
            Arc::clone(&self.records),
            self.me(),
            host,
            Arc::new(Tcp {
                timeout: DIAL_TIMEOUT,
            }),
        ));
        lock(&self.sessions).insert(device.to_owned(), Arc::clone(&session));
        Ok(session)
    }

    /// The session, if one was made: for a read that must not dial.
    pub fn session_if_any(&self, device: &str) -> Option<Arc<Session>> {
        lock(&self.sessions).get(device).cloned()
    }

    /// Where a desktop stands, as a word for the screen, and why when unreachable.
    pub fn standing_of(&self, device: &str) -> (&'static str, Option<String>) {
        match self
            .session_if_any(device)
            .map(|s| s.standing().borrow().clone())
        {
            None => ("disconnected", None),
            Some(Standing::Connected) => ("connected", None),
            Some(Standing::Reconnecting) => ("reconnecting", None),
            Some(Standing::Unreachable(why)) => ("unreachable", Some(why)),
        }
    }

    /// The desktop is unpaired, or its session is to be started over: everything open on it
    /// ends.
    pub fn forget_session(&self, device: &str) {
        lock(&self.sessions).remove(device);
        if let Some(following) = lock(&self.following).remove(device) {
            if let Some(task) = following.events {
                task.abort();
            }
            if let Some(task) = following.standing {
                task.abort();
            }
        }
        lock(&self.views).retain(|(of, _), open| {
            if of == device {
                open.task.abort();
                false
            } else {
                true
            }
        });
        lock(&self.timelines).retain(|(of, _), open| {
            if of == device {
                open.task.abort();
                false
            } else {
                true
            }
        });
    }

    pub fn following(&self) -> MutexGuard<'_, HashMap<String, Following>> {
        lock(&self.following)
    }

    pub fn views(&self) -> MutexGuard<'_, HashMap<(String, String), ViewOpen>> {
        lock(&self.views)
    }

    pub fn timelines(&self) -> MutexGuard<'_, HashMap<(String, String), TimelineOpen>> {
        lock(&self.timelines)
    }
}
