// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **What doctor asks of an engine on another machine** (#6272, ADR 0123 §7), as the local kind
//! asks its own: each read from what Docker prints as data, never from its words.

use super::{Docker, Failed, Image, MOST_INSPECTED, Refused, refused};

/// **How the engine confines runners, as doctor reads it** (#6272).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Confinement {
    /// It gives containers its default seccomp profile.
    Confined,
    /// It answered, with another profile, or none.
    Unconfined,
    /// It didn't answer in its time, or didn't say: why.
    Unread(Refused),
}

/// How long a download of the runner image may take: it is hundreds of megabytes.
const PULL_WITHIN: std::time::Duration = std::time::Duration::from_secs(15 * 60);

impl Docker {
    /// **Whether the engine confines runners as the local kind's daemon check holds it to**
    /// ([`super::check_daemon`], [`super::check_podman`]): the daemon's built-in seccomp
    /// profile, or Podman's default one. An engine that answered with another profile doesn't;
    /// one that didn't answer, or didn't say, is unread, never taken for one that doesn't.
    pub fn confinement(&self) -> Confinement {
        match self.json_or_late(
            &["info", "--format", "{{json .SecurityOptions}}"],
            "docker info",
        ) {
            Ok(options) if options.is_array() => match super::check_daemon(&options) {
                Ok(()) => Confinement::Confined,
                Err(_) => Confinement::Unconfined,
            },
            Ok(_) => Confinement::Unread(refused(
                "docker info did not say which security options its daemon has",
            )),
            // An engine that did not answer is not asked again as a Podman.
            Err(Failed::Late(why)) => Confinement::Unread(why),
            Err(Failed::Refused(why)) => match self.json(
                &["info", "--format", "{{json .Host.Security}}"],
                "podman info",
            ) {
                Ok(security) if security.is_object() => match super::check_podman(&security) {
                    Ok(_) => Confinement::Confined,
                    Err(_) => Confinement::Unconfined,
                },
                _ => Confinement::Unread(why),
            },
        }
    }

    /// Whether this `docker` is Podman's own client (#6269), by its program's name.
    #[must_use]
    pub fn is_podman_client(&self) -> bool {
        self.program
            .file_name()
            .is_some_and(|name| name.to_string_lossy().starts_with("podman"))
    }

    /// **The engine's platform, as Docker names one** (`linux/amd64`): `docker info`'s `OSType`
    /// and `Architecture`.
    pub fn platform(&self) -> Result<String, Refused> {
        let info = self.json(&["info", "--format", "{{json .}}"], "docker info")?;
        let field = |name: &str, podman: &str| {
            info.get(name)
                .or_else(|| info.pointer(podman))
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
        };
        match (
            field("OSType", "/host/os"),
            field("Architecture", "/host/arch"),
        ) {
            (Some(os), Some(arch)) => Ok(crate::poolsetup::doctoring::platform_of(&os, &arch)),
            _ => Err(refused("docker info did not say the engine's platform")),
        }
    }

    /// **The platform `image` is built for, as the engine holds it**: `None` where it holds no
    /// such image. An engine that didn't answer is refused.
    pub fn image_platform(&self, image: &Image) -> Result<Option<String>, Refused> {
        match self.json_or_late(
            &["image", "inspect", image.as_str()],
            "docker image inspect",
        ) {
            Ok(inspected) => {
                let field = |name: &str| {
                    inspected
                        .pointer(&format!("/0/{name}"))
                        .and_then(serde_json::Value::as_str)
                        .map(str::to_owned)
                };
                match (field("Os"), field("Architecture")) {
                    (Some(os), Some(arch)) => {
                        Ok(Some(crate::poolsetup::doctoring::platform_of(&os, &arch)))
                    }
                    _ => Err(refused(
                        "docker image inspect did not say the image's platform",
                    )),
                }
            }
            Err(Failed::Late(why)) => Err(why),
            Err(Failed::Refused(_)) => Ok(None),
        }
    }

    /// **Downloads `image` by its digest**, as making a runner would.
    pub fn pull(&self, image: &Image) -> Result<(), Refused> {
        self.run_prepared(
            &self.prepared()?,
            &["pull".into(), "--quiet".into(), image.as_str().into()],
            None,
            MOST_INSPECTED,
            Some(PULL_WITHIN),
        )
        .map(|_| ())
    }
}
