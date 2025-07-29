use chrono::Utc;
use home::home_dir;
use log::{error, info};
use std::sync::Arc;
use tokio::fs;
use tokio::task::JoinHandle;
use tokio::time::{interval, Duration};

use crate::vojo::app_config::{AppConfig, ServiceType};
use crate::vojo::domain_config::DomainsConfig;
use crate::{app_error, AppError};
#[derive(Debug)]
pub struct CertificateManager {
    config: Arc<AppConfig>,
    renewal_task_handles: Vec<JoinHandle<()>>,
}

impl CertificateManager {
    pub fn new(config: Arc<AppConfig>) -> Self {
        Self {
            config,
            renewal_task_handles: Vec::new(),
        }
    }

    pub fn start_renewal_task(&mut self) {
        info!("Starting single-threaded certificate renewal service...");

        let mut domains_to_check = Vec::new();
        for service in self.config.api_service_config.values() {
            if !(service.server_type == ServiceType::Https
                || service.server_type == ServiceType::Http2Tls)
            {
                continue;
            }

            for domain_config in &service.domain_config {
                if domain_config.domain_name.is_empty() {
                    info!("Found an empty domain configuration, skipping.");
                    continue;
                }

                info!(
                    "Discovered domain [{}], adding to the renewal check queue.",
                    domain_config.domain_name
                );
                domains_to_check.push((domain_config.clone(), service.sender.clone()));
            }
        }

        if domains_to_check.is_empty() {
            info!("No domains configured for renewal.");
            return;
        }

        let handle = tokio::spawn(async move {
            let mut timer = interval(Duration::from_secs(24 * 60 * 60));

            loop {
                timer.tick().await;
                info!("Starting a new certificate renewal check cycle...");

                for (domain_conf, reload_notifier) in &domains_to_check {
                    let domain_name = &domain_conf.domain_name;
                    info!("Performing renewal check for domain: [{domain_name}]");

                    if Self::needs_renewal(domain_conf).await {
                        info!(
                            "Certificate for [{domain_name}] needs renewal, attempting..."
                        );

                        match Self::renew_certificate(domain_conf).await {
                            Ok(_) => {
                                info!("Successfully renewed certificate for [{domain_name}].");
                                if let Err(e) = reload_notifier.send(()).await {
                                    error!(
                                        "Failed to send reload signal for [{domain_name}]: {e}"
                                    );
                                }
                            }
                            Err(e) => {
                                error!("Error renewing certificate for [{domain_name}]: {e:?}");
                            }
                        }
                    } else {
                        info!(
                            "Certificate for [{domain_name}] does not need renewal yet."
                        );
                    }
                    // Add a small delay between checks to avoid rate-limiting or heavy load.
                    tokio::time::sleep(Duration::from_secs(5)).await;
                }
                info!("This certificate renewal check cycle is complete.");
            }
        });

        self.renewal_task_handles.push(handle);
    }

    async fn needs_renewal(domain_config: &DomainsConfig) -> bool {
        let domain_name = &domain_config.domain_name;
        if domain_name.is_empty() {
            return true; // Treat as needing renewal to trigger an error in the renewal function.
        }

        let cert_path = match home_dir() {
            Some(dir) => dir
                .join("spire")
                .join("domains")
                .join(domain_name)
                .join("cert.pem"),
            None => return true, // Cannot determine home directory, assume renewal is needed.
        };

        info!("Checking certificate validity: {cert_path:?}");

        match tokio::fs::read(&cert_path).await {
            Ok(cert_bytes) => match x509_parser::parse_x509_certificate(&cert_bytes) {
                Ok((_, cert)) => {
                    let now = Utc::now().timestamp();
                    let expiration_time = cert.validity().not_after.timestamp();
                    let remaining_seconds = expiration_time - now;
                    remaining_seconds < (30 * 24 * 60 * 60)
                }
                Err(_) => true,
            },
            Err(_) => true,
        }
    }

    async fn renew_certificate(domain_config: &DomainsConfig) -> Result<(), AppError> {
        let domain_name = &domain_config.domain_name;
        if domain_name.is_empty() {
            return Err(app_error!("Renewal failed: domain name is empty."));
        }

        info!("Simulating renewal process for domain: [{domain_name}]");

        let base_path = home_dir()
            .ok_or_else(|| app_error!("Failed to get user home directory"))?
            .join("spire")
            .join("domains")
            .join(domain_name);

        fs::create_dir_all(&base_path).await.map_err(|e| {
            app_error!(
                "Failed to create certificate storage directory {:?}: {}",
                base_path,
                e
            )
        })?;

        let cert_path = base_path.join("cert.pem");
        let key_path = base_path.join("key.pem");

        info!(
            " - Simulating ACME challenge for domain [{domain_name}]..."
        );
        tokio::time::sleep(Duration::from_secs(2)).await;

        info!(
            " - Simulating request for a new certificate from Let's Encrypt for [{domain_name}]..."
        );
        let new_cert_content = format!(
            "-----BEGIN CERTIFICATE-----\n#\n# DUMMY CERT FOR {}\n# RENEWED AT: {}\n#\n-----END CERTIFICATE-----",
            domain_name,
            Utc::now()
        );
        let new_key_content = format!(
            "-----BEGIN PRIVATE KEY-----\n#\n# DUMMY KEY FOR {}\n# RENEWED AT: {}\n#\n-----END PRIVATE KEY-----",
            domain_name,
            Utc::now()
        );
        tokio::time::sleep(Duration::from_secs(1)).await;

        info!(" - Saving new certificate to: {cert_path:?}");
        fs::write(&cert_path, new_cert_content).await.map_err(|e| {
            app_error!("Failed to write certificate file to {:?}: {}", cert_path, e)
        })?;

        info!(" - Saving new private key to: {key_path:?}");
        fs::write(&key_path, new_key_content)
            .await
            .map_err(|e| app_error!("Failed to write private key file to {:?}: {}", key_path, e))?;

        info!(
            "Successfully wrote certificate and private key for domain [{domain_name}] to the local filesystem."
        );

        Ok(())
    }
}
