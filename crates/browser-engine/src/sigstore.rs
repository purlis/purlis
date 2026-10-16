// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Who may sign an engine manifest: one GitHub workflow, keyless, through Sigstore (#2073, ADR
//! 0042 as amended on 2026-10-16).
//!
//! A manifest that did not ship inside this build is trusted only with a Sigstore bundle that
//! [`Sigstore`] verifies **offline**, against [`TRUSTED_ROOT`], the Sigstore trusted root this
//! build ships and each purlis release refreshes. It accepts a bundle only when all of these
//! hold:
//!
//! - its certificate chains to a certificate authority of that root, valid when it signed;
//! - the certificate names exactly one workflow of one repository on one ref
//!   ([`Workflow::identity`]), issued by GitHub's token issuer ([`ISSUER`]);
//! - the signature is over exactly the manifest's bytes;
//! - the signature's entry in a transparency log of that root is in the bundle, with its
//!   inclusion proof and a checkpoint the log signed, and the entry describes this signature.
//!
//! For engine manifests that workflow is [`MANIFEST_WORKFLOW`]: `engine-manifest.yml` on `main`
//! of purlis/purlis. There is no key to keep: the identity is the workflow's own.
//!
//! The proof of the log is asked of every bundle here, before Sigstore's verifier is: a
//! bundle of the format `cosign sign-blob` makes (v0.3), whose every log entry carries its
//! inclusion proof and a checkpoint. The rest is Sigstore's own Rust verifier
//! (`sigstore-verify`), with this build's cryptography ([`Cryptography`]).

use p256::ecdsa::signature::hazmat::PrehashVerifier;
use ring::signature as ring_signature;
use sigstore_verify::crypto::{
    CryptoProvider, Error as CryptoError, HashContext, SigningScheme, install_default_provider,
};
use sigstore_verify::trust_root::TrustedRoot;
use sigstore_verify::types::{Bundle, HashAlgorithm, MediaType, SignatureContent};
use sigstore_verify::{IdentityMatcher, VerificationPolicy, Verifier};

use crate::install::ManifestSignature;

/// The Sigstore trusted root this build ships: Sigstore's public-good instance's, as its TUF
/// repository published it. Refreshed with each purlis release.
pub const TRUSTED_ROOT: &str = include_str!("../sigstore-trusted-root.json");

/// The issuer of every certificate accepted: GitHub Actions' OIDC token issuer.
pub const ISSUER: &str = "https://token.actions.githubusercontent.com";

/// A GitHub workflow, as the certificate Sigstore gives it names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Workflow<'a> {
    /// `owner/name` of its repository.
    pub repository: &'a str,
    /// Its file under `.github/workflows/`.
    pub file: &'a str,
    /// The ref it ran on, such as `refs/heads/main`.
    pub git_ref: &'a str,
}

impl Workflow<'_> {
    /// The identity its certificates carry: the workflow's address at the ref it ran on.
    pub fn identity(&self) -> String {
        format!(
            "https://github.com/{}/.github/workflows/{}@{}",
            self.repository, self.file, self.git_ref
        )
    }
}

/// The one workflow whose engine manifests are trusted.
pub const MANIFEST_WORKFLOW: Workflow<'static> = Workflow {
    repository: "purlis/purlis",
    file: "engine-manifest.yml",
    git_ref: "refs/heads/main",
};

/// Verifies a Sigstore bundle over a manifest, offline, as one workflow's.
pub struct Sigstore {
    verifier: Verifier,
    identity: String,
}

impl Sigstore {
    /// What engine manifests are checked with: [`TRUSTED_ROOT`] and [`MANIFEST_WORKFLOW`].
    pub fn engine_manifests() -> Self {
        Self::trusting(TRUSTED_ROOT, &MANIFEST_WORKFLOW)
            .expect("the Sigstore trusted root this build ships reads")
    }

    /// Bundles verified against `root` (a trusted root's JSON) as `workflow`'s.
    pub fn trusting(root: &str, workflow: &Workflow<'_>) -> Result<Self, String> {
        // Installed once per process; a second install is refused and the first one stays,
        // which is this same provider.
        let _ = install_default_provider(&CRYPTOGRAPHY);
        let root = TrustedRoot::from_json(root)
            .map_err(|why| format!("the Sigstore trusted root does not read: {why}"))?;
        let verifier = Verifier::new(&root)
            .map_err(|why| format!("the Sigstore trusted root cannot be used: {why}"))?;
        Ok(Self {
            verifier,
            identity: workflow.identity(),
        })
    }

    /// The one identity accepted.
    pub fn identity(&self) -> &str {
        &self.identity
    }
}

impl ManifestSignature for Sigstore {
    fn verify(&self, manifest: &[u8], bundle: &str) -> Result<(), String> {
        let bundle = Bundle::from_json(bundle)
            .map_err(|why| format!("the Sigstore bundle does not read: {why}"))?;
        if !matches!(bundle.content, SignatureContent::MessageSignature(_)) {
            return Err("the Sigstore bundle signs no manifest".to_owned());
        }
        proves_its_log_entries(&bundle)?;
        let policy = VerificationPolicy::new(IdentityMatcher::Uri(self.identity.clone()), ISSUER);
        let result = self
            .verifier
            .verify(manifest, &bundle, &policy)
            .map_err(|why| format!("the Sigstore bundle does not verify: {why}"))?;
        // What the policy above asks for, read back: a relaxed check is never taken for one.
        if !(result.certificate_verified()
            && result.tlog_verified()
            && result.identity_policy_checked())
        {
            return Err("the Sigstore bundle was not fully verified".to_owned());
        }
        Ok(())
    }
}

/// A bundle of the v0.3 format, with at least one log entry, every one of them carrying its
/// inclusion proof and a checkpoint, or why not.
fn proves_its_log_entries(bundle: &Bundle) -> Result<(), String> {
    if bundle.media_type != MediaType::Bundle0_3 {
        return Err(format!(
            "the Sigstore bundle is of the format {}, not {}",
            bundle.media_type.as_str(),
            MediaType::Bundle0_3.as_str()
        ));
    }
    let entries = &bundle.verification_material.tlog_entries;
    if entries.is_empty() {
        return Err("the Sigstore bundle carries no log entry".to_owned());
    }
    let proven = entries.iter().all(|entry| {
        entry
            .inclusion_proof
            .as_ref()
            .is_some_and(|proof| proof.checkpoint.checkpoint().is_some())
    });
    if !proven {
        return Err(
            "a log entry of the Sigstore bundle carries no inclusion proof and checkpoint"
                .to_owned(),
        );
    }
    Ok(())
}

/// Sigstore's cryptography, done by `ring` (hashing, and signatures over whole messages) and
/// RustCrypto's `p256` (an ECDSA signature over a digest). It makes no keys and signs nothing.
#[derive(Debug)]
pub struct Cryptography;

static CRYPTOGRAPHY: Cryptography = Cryptography;

struct Digesting(ring::digest::Context);

impl HashContext for Digesting {
    fn update(&mut self, data: &[u8]) {
        self.0.update(data);
    }

    fn finalize(self: Box<Self>) -> Vec<u8> {
        self.0.finish().as_ref().to_vec()
    }
}

impl CryptoProvider for Cryptography {
    fn hash(&self, algorithm: HashAlgorithm) -> Result<Box<dyn HashContext>, CryptoError> {
        let algorithm = match algorithm {
            HashAlgorithm::Sha2256 => &ring::digest::SHA256,
            HashAlgorithm::Sha2384 => &ring::digest::SHA384,
            HashAlgorithm::Sha2512 => &ring::digest::SHA512,
            other => {
                return Err(CryptoError::UnsupportedAlgorithm(format!(
                    "cannot compute {other} digests"
                )));
            }
        };
        Ok(Box::new(Digesting(ring::digest::Context::new(algorithm))))
    }

    fn verify_raw(
        &self,
        scheme: SigningScheme,
        public_key: &[u8],
        data: &[u8],
        signature: &[u8],
    ) -> Result<(), CryptoError> {
        let algorithm: &dyn ring_signature::VerificationAlgorithm = match scheme {
            SigningScheme::EcdsaP256Sha256 => &ring_signature::ECDSA_P256_SHA256_ASN1,
            SigningScheme::EcdsaP256Sha384 => &ring_signature::ECDSA_P256_SHA384_ASN1,
            SigningScheme::EcdsaP384Sha256 => &ring_signature::ECDSA_P384_SHA256_ASN1,
            SigningScheme::EcdsaP384Sha384 => &ring_signature::ECDSA_P384_SHA384_ASN1,
            SigningScheme::Ed25519 => &ring_signature::ED25519,
            SigningScheme::RsaPssSha256 => &ring_signature::RSA_PSS_2048_8192_SHA256,
            SigningScheme::RsaPssSha384 => &ring_signature::RSA_PSS_2048_8192_SHA384,
            SigningScheme::RsaPssSha512 => &ring_signature::RSA_PSS_2048_8192_SHA512,
            SigningScheme::RsaPkcs1Sha256 => &ring_signature::RSA_PKCS1_2048_8192_SHA256,
            SigningScheme::RsaPkcs1Sha384 => &ring_signature::RSA_PKCS1_2048_8192_SHA384,
            SigningScheme::RsaPkcs1Sha512 => &ring_signature::RSA_PKCS1_2048_8192_SHA512,
            other => {
                return Err(CryptoError::UnsupportedAlgorithm(format!(
                    "{} signatures are not verified here",
                    other.name()
                )));
            }
        };
        ring_signature::UnparsedPublicKey::new(algorithm, public_key)
            .verify(data, signature)
            .map_err(|_| CryptoError::Verification(format!("{} signature invalid", scheme.name())))
    }

    /// Only ECDSA over P-256, the key Sigstore's certificate authority certifies for a
    /// GitHub workflow's signature, with RustCrypto's `p256`: `ring` verifies no signature
    /// over a digest alone.
    fn verify_prehashed(
        &self,
        scheme: SigningScheme,
        public_key: &[u8],
        digest: &[u8],
        signature: &[u8],
    ) -> Result<(), CryptoError> {
        if !matches!(
            scheme,
            SigningScheme::EcdsaP256Sha256 | SigningScheme::EcdsaP256Sha384
        ) {
            return Err(CryptoError::UnsupportedAlgorithm(format!(
                "{} signatures over a digest alone are not verified here",
                scheme.name()
            )));
        }
        let invalid = || CryptoError::Verification(format!("{} signature invalid", scheme.name()));
        let key = p256::ecdsa::VerifyingKey::from_sec1_bytes(public_key).map_err(|_| invalid())?;
        let signature = p256::ecdsa::Signature::from_der(signature).map_err(|_| invalid())?;
        key.verify_prehash(digest, &signature)
            .map_err(|_| invalid())
    }
}
