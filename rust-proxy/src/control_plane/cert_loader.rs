use crate::app_error;
use crate::utils::fs_utils::get_domain_path;
use crate::vojo::app_error::AppError;
use notify::RecommendedWatcher;
use notify::RecursiveMode;
use notify::Watcher;
use rcgen::KeyPair;
use rcgen::{CertificateParams, DistinguishedName};
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use rustls::ServerConfig;
use rustls_pemfile::{certs, private_key};
use rustls_pki_types::PrivatePkcs8KeyDer;
use std::fs;
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::RwLock;
use std::time::Duration;
use tracing::info;

pub struct TlsCert {
    pub cert: Vec<CertificateDer<'static>>,
    pub key: PrivateKeyDer<'static>,
}

// pub fn load_or_create_cert(domain: &str) -> Result<TlsCert, AppError> {
//     if let Ok((cert_path, key_path)) = find_cert_path(domain) {
//         if cert_path.exists() && key_path.exists() {
//             info!(
//                 "Loading certificate for domain '{}', path: {}",
//                 domain,
//                 cert_path.display()
//             );
//             return load_cert_from_path(&cert_path, &key_path);
//         }
//     }

//     info!(
//         "Certificate not found for domain '{}' at expected path, will generate a self-signed certificate.",
//         domain
//     );
//     create_self_signed_cert(domain)
// }

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
    let key_der = PrivateKeyDer::from(pkcs8_key);
    let cert_chain = vec![cert_der];
    info!(
        "Successfully generated self-signed certificate for domain '{}'.",
        domain
    );
    let config = ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(cert_chain, key_der)
        .map_err(|e| {
            AppError(format!(
                "Failed to create tls config from self-signed cert: {e}"
            ))
        })?;

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

        match rustls_pemfile::certs(&mut cert_reader).next() {
            Some(Ok(cert_der)) => {
                let cert = x509_parser::parse_x509_certificate(&cert_der)
                    .map_err(|e| AppError(format!("Failed to parse certificate: {e:?}")))?
                    .1;

                if cert.validity().is_valid() {
                    info!("Certificate for '{}' is valid.", domain);

                    let key_file = File::open(&key_path).map_err(|e| {
                        AppError(format!(
                            "Failed to open key file '{}': {}",
                            key_path.display(),
                            e
                        ))
                    })?;
                    let mut key_reader = BufReader::new(key_file);

                    let private_key = rustls_pemfile::private_key(&mut key_reader)
                        .and_then(|key| {
                            key.ok_or_else(|| {
                                std::io::Error::new(
                                    std::io::ErrorKind::NotFound,
                                    "No private key found in pem file",
                                )
                            })
                        })
                        .map_err(|e| AppError(format!("Failed to parse private key: {e}")))?;

                    let config = ServerConfig::builder()
                        .with_no_client_auth()
                        .with_single_cert(vec![cert_der], private_key)
                        .map_err(|e| AppError(format!("Failed to create tls config: {e}")))?;

                    return Ok(config);
                } else {
                    warn!("Certificate for domain '{domain}' has expired or is not yet valid. Falling back to a self-signed certificate.");
                }
            }
            Some(Err(e)) => {
                warn!("Failed to parse certificate file for '{domain}': {e}. Falling back to a self-signed certificate.");
            }
            None => {
                warn!(
                    "No certificates found in '{}'. Falling back to a self-signed certificate.",
                    cert_path.display()
                );
            }
        };
    } else {
        info!("Certificate not found for domain '{}' at path '{}', will generate a self-signed certificate.", domain, cert_dir.display());
    }

    create_self_signed_cert(domain)
}
