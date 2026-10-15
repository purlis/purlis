// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Point kubectl at it** (qw384b): on the person's press, a chat's own copy of one kube
//! context for a cluster on this Mac ([`purlis_core::sandbox::kubeconfig`]), its server named
//! as the chat's proxy carries it. The person's own kubeconfig is only read, by kubectl itself
//! (`kubectl config view`), and never written: the copy is the chat's, named by `KUBECONFIG`
//! for that chat alone, and the chat restarts on its conversation to take it.

use purlis_core::sandbox::kubeconfig;
use serde_json::Value;

use crate::planes::{PlaneId, Planes};

/// What the commands here name their work, off the window's thread.
const READING_KUBECONFIG: &str = "reading your kubeconfig";

/// One context of the person's kubeconfig, as Point kubectl at it lists it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct KubeContext {
    /// Its name, as kubectl knows it.
    pub name: String,
    /// Its cluster's server, as the kubeconfig names it.
    pub server: String,
    /// The port of this Mac it reaches, where purlis can point a chat at it.
    pub port: Option<u16>,
    /// Why purlis would not point a chat at it, where it would not: never pressable then.
    pub refused: Option<String>,
}

/// **The contexts of the person's kubeconfig** (qw384b), as kubectl reads it (every file
/// `KUBECONFIG` names), each with whether a chat can be pointed at it. Data a context holds
/// (keys, certificates) is left out by kubectl itself.
#[tauri::command]
#[specta::specta]
pub async fn kube_contexts() -> Result<Vec<KubeContext>, String> {
    crate::off_the_window(READING_KUBECONFIG, || {
        let view = kubectl_view(&["config", "view", "-o", "json"])?;
        Ok(listed(&view))
    })
    .await
}

/// **Points chat `session`'s kubectl at `context`** (qw384b): a cluster on this Mac only, any
/// other refused with why. What the Notice says once it is done.
#[tauri::command]
#[specta::specta]
pub async fn point_kubectl(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    session: u32,
    context: String,
) -> Result<String, String> {
    let held = planes.held(&plane)?;
    crate::off_the_window(READING_KUBECONFIG, move || {
        point(held.chats(), session, &context, &kubectl_view)
    })
    .await
}

/// [`point_kubectl`] with `view` as kubectl's.
pub(crate) fn point(
    chats: &crate::chats::Chats,
    session: u32,
    context: &str,
    view: &dyn Fn(&[&str]) -> Result<Value, String>,
) -> Result<String, String> {
    // `--context=<name>` whole, so a name can never read as another option.
    let named = format!("--context={context}");
    let read = view(&["config", "view", "--raw", "--minify", &named, "-o", "json"])?;
    let pointed = kubeconfig::for_this_chat(&read, context)?;
    let told = format!(
        "purlis: the person pointed this chat's kubectl at `{}`, a cluster on this Mac: \
         KUBECONFIG names this chat's own copy of that one context, whose server is \
         `https://{}:{}`, reached through this chat's proxy. This chat restarted on the same \
         conversation to take it. Retry the kubectl command.",
        context.replace('`', ""),
        purlis_core::sandbox::localport::THIS_MAC,
        pointed.port
    );
    chats.point_kubectl(session, pointed.config, told)?;
    Ok(format!(
        "kubectl in this chat now uses its own copy of {context}, reached as {}:{}. The chat \
         restarts on the same conversation to take it once its turn has ended. Your own \
         kubeconfig is unchanged.",
        purlis_core::sandbox::localport::THIS_MAC,
        pointed.port
    ))
}

fn listed(view: &Value) -> Vec<KubeContext> {
    kubeconfig::contexts(view)
        .into_iter()
        .map(|one| KubeContext {
            name: one.name,
            server: one.server,
            port: one.port,
            refused: one.refused,
        })
        .collect()
}

/// What `kubectl <args>` prints, read as JSON: the kubectl this Mac runs, found where a chat's
/// would be.
fn kubectl_view(args: &[&str]) -> Result<Value, String> {
    let kubectl = purlis_core::programs::find("kubectl", &purlis_core::programs::search_dirs())
        .ok_or_else(|| {
            "purlis found no kubectl on this Mac, so it read no kubeconfig.".to_owned()
        })?;
    let out = purlis_core::forklock::output(std::process::Command::new(kubectl).args(args))
        .map_err(|why| format!("purlis could not run kubectl ({why})."))?;
    if !out.status.success() {
        let said = String::from_utf8_lossy(&out.stderr);
        let said = said.lines().next().unwrap_or_default().trim();
        return Err(format!("kubectl could not read your kubeconfig: {said}"));
    }
    serde_json::from_slice(&out.stdout)
        .map_err(|why| format!("purlis could not read what kubectl printed ({why})."))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn printing(view: Value) -> impl Fn(&[&str]) -> Result<Value, String> {
        move |args: &[&str]| {
            assert!(args.contains(&"--minify"), "{args:?}");
            assert!(args.contains(&"--context=prod"), "{args:?}");
            Ok(view.clone())
        }
    }

    #[test]
    fn a_context_elsewhere_points_nothing_and_says_why() {
        let chats = crate::chats::Chats::new();
        let view = serde_json::json!({
            "clusters": [{"name": "prod", "cluster": {"server": "https://api.prod.example.com"}}],
            "users": [{"name": "u", "user": {}}],
            "contexts": [{"name": "prod", "context": {"cluster": "prod", "user": "u"}}],
        });
        let why = point(&chats, 7, "prod", &printing(view)).expect_err("refused");
        assert!(why.contains("on this Mac"), "{why}");
        assert!(chats.owed_restarts().is_empty());
    }

    #[test]
    fn every_context_is_listed_with_why_it_cannot_be_pointed_at() {
        let view = serde_json::json!({
            "clusters": [
                {"name": "kind", "cluster": {"server": "https://127.0.0.1:50795"}},
                {"name": "prod", "cluster": {"server": "https://api.prod.example.com"}},
            ],
            "contexts": [
                {"name": "kind", "context": {"cluster": "kind"}},
                {"name": "prod", "context": {"cluster": "prod"}},
            ],
        });
        let listed = listed(&view);
        assert_eq!(listed[0].port, Some(50795));
        assert_eq!(listed[0].refused, None);
        assert_eq!(listed[1].port, None);
        assert!(listed[1].refused.is_some());
    }
}
