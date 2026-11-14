//! Encryption between the main computer and the extra tills.
//!
//! The main computer makes its own certificate once (`lan-cert.der` and
//! `lan-key.der` in its config folder). No certificate authority is
//! involved: each till remembers the certificate's SHA-256 fingerprint and
//! accepts only that one (pinning), so another machine on the network can't
//! pretend to be the main computer.
//!
//! How a till learns the fingerprint without trusting the network:
//! - **Pairing** runs SPAKE2 with the six-digit code as the password. Both
//!   sides end up with a key only someone who knows the code can have, and
//!   each proves it with an HMAC over the fingerprint *it* sees. A machine in
//!   the middle would show a different certificate, so the proofs wouldn't
//!   match, and it gets a single guess at the code per attempt (online, never
//!   offline).
//! - **A till paired before encryption** (or one whose main computer got a
//!   new certificate, e.g. after a restore onto a new computer) asks the main
//!   computer for an HMAC over the fingerprint, keyed with the SHA-256 of the
//!   till's secret, which only the shop's database holds.

use std::path::Path;
use std::sync::{Arc, Mutex};

use hmac::{Hmac, Mac};
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::crypto::{ring as provider, CryptoProvider, WebPkiSupportedAlgorithms};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer, ServerName, UnixTime};
use rustls::{ClientConfig, DigitallySignedStruct, ServerConfig, SignatureScheme};
use sha2::{Digest, Sha256};

use crate::error::{Error, Result};

/// The main computer's certificate and key.
pub struct Identity {
    pub cert: CertificateDer<'static>,
    key: Vec<u8>,
    pub fingerprint: String,
}

pub fn fingerprint(cert: &[u8]) -> String {
    format!("{:x}", Sha256::digest(cert))
}

fn provider() -> Arc<CryptoProvider> {
    Arc::new(provider::default_provider())
}

/// This computer's certificate, made the first time it is needed.
pub fn load_or_create(dir: &Path) -> Result<Identity> {
    let cert_path = dir.join("lan-cert.der");
    let key_path = dir.join("lan-key.der");
    if let (Ok(cert), Ok(key)) = (std::fs::read(&cert_path), std::fs::read(&key_path)) {
        if !cert.is_empty() && !key.is_empty() {
            return Ok(Identity { fingerprint: fingerprint(&cert), cert: CertificateDer::from(cert), key });
        }
    }
    let made = rcgen::generate_simple_self_signed(vec!["pos-main".to_owned()]).map_err(|e| Error::Invalid(format!("certificate: {e}")))?;
    let cert = made.cert.der().to_vec();
    let key = made.signing_key.serialize_der();
    std::fs::create_dir_all(dir)?;
    write_private(&key_path, &key)?;
    std::fs::write(&cert_path, &cert)?;
    Ok(Identity { fingerprint: fingerprint(&cert), cert: CertificateDer::from(cert), key })
}

fn write_private(path: &Path, bytes: &[u8]) -> Result<()> {
    std::fs::write(path, bytes)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}

pub fn server_config(id: &Identity) -> Result<Arc<ServerConfig>> {
    let key = PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(id.key.clone()));
    let config = ServerConfig::builder_with_provider(provider())
        .with_safe_default_protocol_versions()
        .map_err(tls_error)?
        .with_no_client_auth()
        .with_single_cert(vec![id.cert.clone()], key)
        .map_err(tls_error)?;
    Ok(Arc::new(config))
}

fn tls_error(e: rustls::Error) -> Error {
    Error::Invalid(format!("tls: {e}"))
}

/// Accepts the one certificate it was told to (or, while learning it, any
/// certificate), and remembers what it saw.
#[derive(Debug)]
pub struct Pin {
    expected: Option<String>,
    seen: Mutex<Option<String>>,
    algorithms: WebPkiSupportedAlgorithms,
}

impl Pin {
    /// The fingerprint of the last certificate the other end showed.
    pub fn seen(&self) -> Option<String> {
        self.seen.lock().unwrap().clone()
    }

    /// The other end showed a certificate other than the pinned one.
    pub fn mismatched(&self) -> bool {
        match (&self.expected, self.seen()) {
            (Some(e), Some(s)) => *e != s,
            _ => false,
        }
    }
}

impl ServerCertVerifier for Pin {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp: &[u8],
        _now: UnixTime,
    ) -> std::result::Result<ServerCertVerified, rustls::Error> {
        let fp = fingerprint(end_entity);
        *self.seen.lock().unwrap() = Some(fp.clone());
        match &self.expected {
            Some(e) if *e != fp => Err(rustls::Error::General("not the paired main computer".into())),
            _ => Ok(ServerCertVerified::assertion()),
        }
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> std::result::Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls12_signature(message, cert, dss, &self.algorithms)
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> std::result::Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls13_signature(message, cert, dss, &self.algorithms)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.algorithms.supported_schemes()
    }
}

/// A client config that trusts only `expected` (or any certificate when
/// None, to learn it), and the pin that records what it saw.
pub fn client_config(expected: Option<String>) -> Result<(ClientConfig, Arc<Pin>)> {
    let provider = provider();
    let pin = Arc::new(Pin { expected, seen: Mutex::new(None), algorithms: provider.signature_verification_algorithms });
    let config = ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .map_err(tls_error)?
        .dangerous()
        .with_custom_certificate_verifier(pin.clone())
        .with_no_client_auth();
    Ok((config, pin))
}

/// HMAC-SHA256 over the parts (each length-prefixed), as hex.
pub fn proof(key: &[u8], parts: &[&[u8]]) -> String {
    let mut mac = Hmac::<Sha256>::new_from_slice(key).expect("any key length");
    for p in parts {
        mac.update(&(p.len() as u64).to_be_bytes());
        mac.update(p);
    }
    format!("{:x}", mac.finalize().into_bytes())
}

/// Compares two proofs without leaking where they differ.
pub fn same(a: &str, b: &str) -> bool {
    a.len() == b.len() && a.bytes().zip(b.bytes()).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

// SPAKE2 for pairing: the till is side A, the main computer side B.
pub use spake2::{Ed25519Group, Identity as SpakeId, Password, Spake2};
pub const TILL_ID: &[u8] = b"pos-till";
pub const MAIN_ID: &[u8] = b"pos-main";

pub fn spake_till(code: &str) -> (Spake2<Ed25519Group>, Vec<u8>) {
    Spake2::<Ed25519Group>::start_a(&Password::new(code.as_bytes()), &SpakeId::new(TILL_ID), &SpakeId::new(MAIN_ID))
}

pub fn spake_main(code: &str) -> (Spake2<Ed25519Group>, Vec<u8>) {
    Spake2::<Ed25519Group>::start_b(&Password::new(code.as_bytes()), &SpakeId::new(TILL_ID), &SpakeId::new(MAIN_ID))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_certificate_is_made_once_and_kept() {
        let dir = std::env::temp_dir().join(format!("pos-tls-{}", uuid::Uuid::new_v4()));
        let a = load_or_create(&dir).unwrap();
        let b = load_or_create(&dir).unwrap();
        assert_eq!(a.fingerprint, b.fingerprint);
        assert_eq!(a.fingerprint.len(), 64);
        server_config(&a).unwrap();
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn spake_agrees_only_on_the_same_code() {
        let (till, a) = spake_till("123456");
        let (main, b) = spake_main("123456");
        assert_eq!(till.finish(&b).unwrap(), main.finish(&a).unwrap());

        let (till, a) = spake_till("123456");
        let (main, b) = spake_main("654321");
        assert_ne!(till.finish(&b).unwrap(), main.finish(&a).unwrap());
    }

    #[test]
    fn proofs_depend_on_every_part() {
        let p = proof(b"key", &[b"till", b"abc"]);
        assert!(same(&p, &proof(b"key", &[b"till", b"abc"])));
        assert!(!same(&p, &proof(b"key", &[b"till", b"abd"])));
        assert!(!same(&p, &proof(b"key", &[b"tillab", b"c"])));
        assert!(!same(&p, &proof(b"kez", &[b"till", b"abc"])));
    }
}
