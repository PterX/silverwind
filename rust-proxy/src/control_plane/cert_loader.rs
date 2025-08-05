use crate::utils::fs_utils::get_domain_path;
use crate::vojo::app_error::AppError;
use notify::RecommendedWatcher;
use notify::RecursiveMode;
use notify::Watcher;
use rcgen::KeyPair;
use rcgen::{CertificateParams, DistinguishedName};
use rustls::pki_types::PrivateKeyDer;
use rustls::ServerConfig;
use rustls_pki_types::PrivatePkcs8KeyDer;
use std::fs::File;
use std::io::BufReader;
use std::sync::Arc;
use std::sync::RwLock;
use std::time::Duration;
use tracing::info;

fn create_self_signed_cert(domain: &str) -> Result<ServerConfig, AppError> {
    info!(
        "Generating self-signed certificate for domain '{}'...",
        domain
    );
    let mut params = CertificateParams::new(vec![domain.to_string()])?;
    params.distinguished_name = DistinguishedName::new();
    let key_pair = KeyPair::generate()?;
    let cert = params.self_signed(&key_pair)?;
    let cert_der = cert.der().clone();
    let pem = cert.pem();
    let private_key_der_bytes = key_pair.serialize_der();
    let pkcs8_key = PrivatePkcs8KeyDer::from(private_key_der_bytes);
    let private_key = PrivateKeyDer::from(pkcs8_key);
    // let cert_der = vec![cert_der];
    info!(
        "Successfully generated self-signed certificate for domain '{}'.",
        domain
    );

    let config = ServerConfig::builder_with_protocol_versions(&[
        &rustls::version::TLS13,
        &rustls::version::TLS12,
    ])
    .with_no_client_auth()
    .with_single_cert(vec![cert_der], private_key)
    .map_err(|e| AppError(format!("Failed to create tls config: {e}")))?;

    Ok(config)
}
pub async fn watch_for_certificate_changes(
    domain: &str,
    tls_config: Arc<RwLock<rustls::ServerConfig>>,
) -> Result<(), AppError> {
    let cert_dir = match get_domain_path(domain) {
        Ok(dir) => dir,
        Err(e) => return Err(e),
    };
    if let Err(e) = tokio::fs::create_dir_all(&cert_dir).await {
        error!(
            "Failed to create certificate directory '{}': {}",
            cert_dir.display(),
            e
        );
        return Ok(());
    }

    let cert_path = cert_dir.join("cert.pem");
    let key_path = cert_dir.join("key.pem");
    let (tx, mut rx) = tokio::sync::mpsc::channel(1);

    let mut watcher = match RecommendedWatcher::new(
        move |res: Result<notify::Event, notify::Error>| {
            if let Ok(event) = res {
                if matches!(
                    event.kind,
                    notify::EventKind::Modify(_) | notify::EventKind::Create(_)
                ) && event
                    .paths
                    .iter()
                    .any(|p| p == &cert_path || p == &key_path)
                {
                    info!("Certificate or key file change detected: {:?}", event.kind);
                    let _ = tx.blocking_send(());
                }
            }
        },
        notify::Config::default(),
    ) {
        Ok(w) => w,
        Err(e) => {
            error!("Failed to create file watcher: {e}");
            return Ok(());
        }
    };

    if let Err(e) = watcher.watch(&cert_dir, RecursiveMode::NonRecursive) {
        error!(
            "Failed to watch certificate directory at '{}': {}",
            cert_dir.display(),
            e
        );
        return Ok(());
    }

    info!(
        "Started watching for certificate changes in directory: {:?}",
        cert_dir
    );

    while rx.recv().await.is_some() {
        tokio::time::sleep(Duration::from_secs(1)).await;

        info!("Detected change in certificate/key files. Attempting to reload.");
        match load_tls_config(domain) {
            Ok(new_config) => {
                let mut config_writer = tls_config.write().map_err(|e| AppError(e.to_string()))?;
                *config_writer = new_config;
                info!("Successfully reloaded TLS certificate.");
            }
            Err(e) => {
                error!("Failed to reload TLS certificate: {e}. Keeping the old one.");
            }
        }
    }
    Ok(())
}
pub fn load_tls_config(domain: &str) -> Result<ServerConfig, AppError> {
    let cert_dir = get_domain_path(domain)?;
    let cert_path = cert_dir.join("cert.pem");
    let key_path = cert_dir.join("key.pem");

    if cert_path.exists() && key_path.exists() {
        info!(
            "Found certificate for domain '{}' at '{}'",
            domain,
            cert_dir.display()
        );

        let cert_file = File::open(&cert_path).map_err(|e| {
            AppError(format!(
                "Failed to open cert file '{}': {}",
                cert_path.display(),
                e
            ))
        })?;
        let mut cert_reader = BufReader::new(cert_file);

        match rustls_pemfile::certs(&mut cert_reader).collect::<Result<Vec<_>, _>>() {
            Ok(certs) if !certs.is_empty() => {
                let first_cert = x509_parser::parse_x509_certificate(&certs[0])
                    .map_err(|e| AppError(format!("Failed to parse certificate: {e:?}")))?
                    .1;

                if first_cert.validity().is_valid() {
                    info!("Certificate for '{}' is valid.", domain);

                    let key_file = File::open(&key_path).map_err(|e| {
                        AppError(format!(
                            "Failed to open key file '{}': {}",
                            key_path.display(),
                            e
                        ))
                    })?;
                    let mut key_reader = BufReader::new(key_file);

                    let private_key = rustls_pemfile::private_key(&mut key_reader)?
                        .ok_or(AppError("Failed to parse private key:".to_string()))?;

                    let config = ServerConfig::builder_with_protocol_versions(&[
                        &rustls::version::TLS13,
                        &rustls::version::TLS12,
                    ])
                    .with_no_client_auth()
                    .with_single_cert(certs, private_key)
                    .map_err(|e| AppError(format!("Failed to create tls config: {e}")))?;

                    return Ok(config);
                } else {
                    warn!("Certificate for domain '{domain}' has expired or is not yet valid. Falling back to a self-signed certificate.");
                }
            }
            Ok(_) => {
                warn!(
                    "No certificates found in '{}'. Falling back to a self-signed certificate.",
                    cert_path.display()
                );
            }
            Err(e) => {
                // An error occurred reading the certs
                warn!("Failed to parse certificate file for '{domain}': {e}. Falling back to a self-signed certificate.");
            }
        };
    } else {
        info!("Certificate not found for domain '{}' at path '{}', will generate a self-signed certificate.", domain, cert_dir.display());
    }

    create_self_signed_cert(domain)
}
