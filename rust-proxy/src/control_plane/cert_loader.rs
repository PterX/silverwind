use crate::utils::fs_utils::get_domain_path;
use rcgen::KeyPair;
use rcgen::{CertificateParams, DistinguishedName};
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use rustls_pemfile::{certs, private_key};
use rustls_pki_types::pem::PemObject;
use rustls_pki_types::PrivatePkcs8KeyDer;
use std::fs;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use tracing::info;

use crate::app_error;
use crate::vojo::app_error::AppError;

pub struct TlsCert {
    pub cert: Vec<CertificateDer<'static>>,
    pub key: PrivateKeyDer<'static>,
}

pub fn load_or_create_cert(domain: &str) -> Result<TlsCert, AppError> {
    if let Ok((cert_path, key_path)) = find_cert_path(domain) {
        if cert_path.exists() && key_path.exists() {
            info!(
                "Loading certificate for domain '{}', path: {}",
                domain,
                cert_path.display()
            );
            return load_cert_from_path(&cert_path, &key_path);
        }
    }

    info!(
        "Certificate not found for domain '{}' at expected path, will generate a self-signed certificate.",
        domain
    );
    create_self_signed_cert(domain)
}

fn load_cert_from_path(cert_path: &Path, key_path: &Path) -> Result<TlsCert, AppError> {
    let cert_file = fs::File::open(cert_path).map_err(|e| {
        app_error!(
            "Failed to open certificate file '{}': {}",
            cert_path.display(),
            e
        )
    })?;
    let mut cert_reader = BufReader::new(cert_file);
    let certs: Vec<CertificateDer> =
        certs(&mut cert_reader)
            .collect::<Result<_, _>>()
            .map_err(|e| {
                app_error!(
                    "Failed to parse certificate file '{}': {}",
                    cert_path.display(),
                    e
                )
            })?;

    let key_file = fs::File::open(key_path).map_err(|e| {
        app_error!(
            "Failed to open private key file '{}': {}",
            key_path.display(),
            e
        )
    })?;
    let mut key_reader = BufReader::new(key_file);

    let key = private_key(&mut key_reader)
        .map_err(|e| {
            app_error!(
                "Failed to parse private key file '{}': {}",
                key_path.display(),
                e
            )
        })?
        .ok_or_else(|| app_error!("No PKCS8/RSA private key found in '{}'", key_path.display()))?;

    Ok(TlsCert { cert: certs, key })
}

fn find_cert_path(domain: &str) -> Result<(PathBuf, PathBuf), AppError> {
    let base_path = get_domain_path(domain)?;
    let cert_path = base_path.join("cert.pem");
    let key_path = base_path.join("key.pem");
    Ok((cert_path, key_path))
}

fn create_self_signed_cert(domain: &str) -> Result<TlsCert, AppError> {
    info!(
        "Generating self-signed certificate for domain '{}'...",
        domain
    );
    let mut params = CertificateParams::new(vec![domain.to_string()])?;
    params.distinguished_name = DistinguishedName::new();
    let ca_key = KeyPair::generate()?;

    let cert = params.self_signed(&ca_key)?;

    let cert_der = cert.der().clone();
    let pem = cert.pem();
    let key_der = PrivatePkcs8KeyDer::from_pem_slice(pem.as_bytes())?;

    let key = PrivateKeyDer::Pkcs8(key_der);

    info!(
        "Successfully generated self-signed certificate for domain '{}'.",
        domain
    );
    Ok(TlsCert {
        cert: vec![cert_der],
        key,
    })
}
