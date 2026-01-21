use rustls::crypto::ring::sign::any_supported_type;
use rustls::server::{ClientHello, ResolvesServerCert};
use rustls::sign;
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Debug)]
pub struct SniCertResolver {
    certs: HashMap<String, Arc<sign::CertifiedKey>>,
    default_cert: Option<Arc<sign::CertifiedKey>>,
}

impl SniCertResolver {
    pub fn new() -> Self {
        Self {
            certs: HashMap::new(),
            default_cert: None,
        }
    }

    pub fn load_cert(
        &mut self,
        domain: &str,
        cert_path: &str,
        key_path: &str,
        is_default: bool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let cert_file = std::fs::read(cert_path)?;
        let certs =
            rustls_pemfile::certs(&mut cert_file.as_slice()).collect::<Result<Vec<_>, _>>()?;

        let key_file = std::fs::read(key_path)?;
        let key = rustls_pemfile::private_key(&mut key_file.as_slice())?
            .ok_or("Could not find private key in file")?;

        let signing_key =
            any_supported_type(&key).map_err(|_| "Private key type not supported by rustls")?;

        let certified_key = Arc::new(sign::CertifiedKey::new(certs, signing_key));

        self.certs.insert(domain.to_string(), certified_key.clone());

        if is_default {
            self.default_cert = Some(certified_key);
        }

        Ok(())
    }
}

impl ResolvesServerCert for SniCertResolver {
    fn resolve(&self, client_hello: ClientHello<'_>) -> Option<Arc<sign::CertifiedKey>> {
        if let Some(sni_name) = client_hello.server_name() {
            if let Some(cert) = self.certs.get(sni_name) {
                println!("SNI match for: {sni_name}, providing specific certificate.");
                return Some(Arc::clone(cert));
            }
        }

        error!("No SNI match, providing default certificate.");
        self.default_cert.as_ref().map(Arc::clone)
    }
}
