// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! `purlis-cluster-operator doctor` (#2236): what a cluster lacks before purlis's cluster
//! operator is installed, one line per finding, each naming the next step. The checks and their
//! words are `purlis-cluster-doctor`'s, which Pool setup's Check the cluster runs too (#4492);
//! here is the cluster they ask, [`Kube`], with the credentials of whoever runs it.

pub use purlis_cluster_doctor::*;

use k8s_openapi::api::apps::v1::Deployment;
use k8s_openapi::api::authorization::v1::{
    ResourceAttributes, SelfSubjectAccessReview, SelfSubjectAccessReviewSpec,
};
use k8s_openapi::api::core::v1::Namespace;
use k8s_openapi::apiextensions_apiserver::pkg::apis::apiextensions::v1::CustomResourceDefinition;
use kube::api::{Api, ListParams, PostParams};
use kube::config::Kubeconfig;

/// The real cluster, through a kube client with the credentials of whoever runs doctor.
pub struct Kube {
    client: kube::Client,
    asked: String,
}

impl Kube {
    /// `client`, which asks the cluster the person knows as `asked`.
    pub fn new(client: kube::Client, asked: impl Into<String>) -> Self {
        Kube {
            client,
            asked: asked.into(),
        }
    }

    /// The cluster of the person's kubeconfig (or the pod's own, in a cluster), as kubectl
    /// picks it.
    pub async fn from_environment() -> Result<Self, String> {
        let config = kube::Config::infer()
            .await
            .map_err(|error| error.to_string())?;
        let server = config.cluster_url.to_string();
        let asked = match Kubeconfig::read().ok().and_then(|k| k.current_context) {
            Some(context) => format!("context {context}, API server {server}"),
            None => format!("API server {server}"),
        };
        let client = kube::Client::try_from(config).map_err(|error| error.to_string())?;
        Ok(Kube::new(client, asked))
    }
}

fn unanswered(error: kube::Error) -> Unanswered {
    match &error {
        kube::Error::Api(status) if status.code == 403 => Unanswered::Forbidden,
        _ => Unanswered::Failed(error.to_string()),
    }
}

impl Cluster for Kube {
    fn asked(&self) -> String {
        self.asked.clone()
    }

    async fn version(&self) -> Result<Version, Unanswered> {
        let info = self.client.apiserver_version().await.map_err(unanswered)?;
        Ok(Version {
            major: info.major,
            minor: info.minor,
            git_version: info.git_version,
        })
    }

    async fn definition(&self, name: &str) -> Result<Option<Vec<String>>, Unanswered> {
        let definitions = Api::<CustomResourceDefinition>::all(self.client.clone());
        let found = definitions.get_opt(name).await.map_err(unanswered)?;
        Ok(found.map(|definition| {
            definition
                .spec
                .versions
                .into_iter()
                .filter(|version| version.served)
                .map(|version| version.name)
                .collect()
        }))
    }

    async fn controller(&self) -> Result<Option<Controller>, Unanswered> {
        let (namespace, name) = AGENT_SANDBOX_CONTROLLER;
        let deployments = Api::<Deployment>::namespaced(self.client.clone(), namespace);
        let found = deployments.get_opt(name).await.map_err(unanswered)?;
        Ok(found.map(|deployment| Controller {
            ready: deployment
                .status
                .and_then(|status| status.ready_replicas)
                .unwrap_or(0),
            images: deployment
                .spec
                .and_then(|spec| spec.template.spec)
                .map(|pod| pod.containers.into_iter().filter_map(|c| c.image).collect())
                .unwrap_or_default(),
        }))
    }

    async fn may(&self, verb: &str, group: &str, resource: &str) -> Result<bool, Unanswered> {
        let review = SelfSubjectAccessReview {
            spec: SelfSubjectAccessReviewSpec {
                resource_attributes: Some(ResourceAttributes {
                    verb: Some(verb.to_owned()),
                    group: Some(group.to_owned()),
                    resource: Some(resource.to_owned()),
                    ..ResourceAttributes::default()
                }),
                non_resource_attributes: None,
            },
            ..SelfSubjectAccessReview::default()
        };
        let reviews = Api::<SelfSubjectAccessReview>::all(self.client.clone());
        let answered = reviews
            .create(&PostParams::default(), &review)
            .await
            .map_err(unanswered)?;
        Ok(answered.status.is_some_and(|status| status.allowed))
    }

    async fn marked_namespaces(&self) -> Result<Vec<String>, Unanswered> {
        let (key, value) = MANAGED_BY;
        let namespaces = Api::<Namespace>::all(self.client.clone());
        let marked = namespaces
            .list(&ListParams::default().labels(&format!("{key}={value}")))
            .await
            .map_err(unanswered)?;
        Ok(marked
            .items
            .into_iter()
            .filter_map(|namespace| namespace.metadata.name)
            .collect())
    }
}
