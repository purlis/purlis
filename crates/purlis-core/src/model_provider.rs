// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Model provider declarations (T1a-06, #6749): the stub interface, to be built.

use std::collections::BTreeSet;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum From {
    BuiltIn,
    Project { project: String },
    Yours,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Vendor,
    Router,
    Cloud,
    Local,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Required,
    Optional,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    AnthropicMessages,
    OpenaiChat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Auth {
    XApiKey,
    Bearer,
    Sigv4,
    GoogleOauth,
    Entra,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Base {
    origin: String,
    path: String,
}

impl Base {
    pub fn origin(&self) -> &str {
        &self.origin
    }
    pub fn path(&self) -> &str {
        &self.path
    }
    pub fn upstream_path(&self, _request_path: &str) -> String {
        String::new()
    }
    pub fn allows(&self, _request_path: &str, _allowlist: &[&str]) -> bool {
        false
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Endpoint {
    pub format: Format,
    pub base: Base,
    pub auth: Auth,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Price {
    pub input: Option<f64>,
    pub output: Option<f64>,
    pub cache_read: Option<f64>,
    pub cache_write: Option<f64>,
    pub cache_write_1h: Option<f64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ModelRow {
    pub id: String,
    pub context: Option<u64>,
    pub max_output: Option<u64>,
    pub tools: Option<bool>,
    pub price: Option<Price>,
    pub tier_above: Option<u64>,
    pub source: Option<String>,
    pub as_of: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Models {
    pub background: Option<String>,
    pub list: Option<Base>,
    pub table: Vec<ModelRow>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Declaration {
    pub from: From,
    pub id: String,
    pub name: String,
    pub kind: Kind,
    pub key: Key,
    pub endpoints: Vec<Endpoint>,
    pub models: Models,
    digest: String,
}

impl Declaration {
    pub fn digest(&self) -> &str {
        &self.digest
    }
    pub fn hosts(&self) -> BTreeSet<String> {
        BTreeSet::new()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectReference {
    pub project: String,
    pub id: String,
    pub digest: String,
}

impl ProjectReference {
    pub fn resolve<'a>(&self, _in_project: &str, _held: &'a [Declaration]) -> Option<&'a Declaration> {
        None
    }
}

pub fn parse(
    _text: &str,
    _from: &From,
    _known: &[Declaration],
    _file: &str,
) -> Result<Declaration, String> {
    Err(String::new())
}

#[cfg(test)]
mod tests;
