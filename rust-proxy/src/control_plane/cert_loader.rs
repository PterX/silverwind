use home::home_dir;
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

/// 代表一个 TLS 证书和私钥对，使用 rustls 的类型。
pub struct TlsCert {
    pub cert: Vec<CertificateDer<'static>>,
    pub key: PrivateKeyDer<'static>,
}

pub fn load_or_create_cert(domain: &str) -> Result<TlsCert, AppError> {
    // 1. 尝试找到证书的预期路径
    if let Some((cert_path, key_path)) = find_cert_path(domain) {
        // 2. 检查证书和私钥文件是否存在
        if cert_path.exists() && key_path.exists() {
            info!(
                "为域名 '{}' 加载证书，路径: {}",
                domain,
                cert_path.display()
            );
            return load_cert_from_path(&cert_path, &key_path);
        }
    }

    info!(
        "在预期路径下未找到域名 '{}' 的证书，将生成一个自签名证书。",
        domain
    );
    create_self_signed_cert(domain)
}

fn load_cert_from_path(cert_path: &Path, key_path: &Path) -> Result<TlsCert, AppError> {
    let cert_file = fs::File::open(cert_path)
        .map_err(|e| app_error!("无法打开证书文件 '{}': {}", cert_path.display(), e))?;
    let mut cert_reader = BufReader::new(cert_file);
    let certs: Vec<CertificateDer> = certs(&mut cert_reader)
        .collect::<Result<_, _>>()
        .map_err(|e| app_error!("无法解析证书文件 '{}': {}", cert_path.display(), e))?;

    let key_file = fs::File::open(key_path)
        .map_err(|e| app_error!("无法打开私钥文件 '{}': {}", key_path.display(), e))?;
    let mut key_reader = BufReader::new(key_file);

    let key = private_key(&mut key_reader)
        .map_err(|e| app_error!("无法解析私钥文件 '{}': {}", key_path.display(), e))?
        .ok_or_else(|| app_error!("在 '{}' 中未找到 PKCS8/RSA 私钥", key_path.display()))?;

    Ok(TlsCert { cert: certs, key })
}

fn find_cert_path(domain: &str) -> Option<(PathBuf, PathBuf)> {
    home_dir().map(|home| {
        let base_path = home.join(".spire").join("domains").join(domain);
        let cert_path = base_path.join("tls.crt");
        let key_path = base_path.join("tls.key");
        (cert_path, key_path)
    })
}

fn create_self_signed_cert(domain: &str) -> Result<TlsCert, AppError> {
    info!("为域名 '{}' 生成自签名证书...", domain);

    let mut params = CertificateParams::new(vec![domain.to_string()])?;
    params.distinguished_name = DistinguishedName::new();
    let ca_key = KeyPair::generate()?;

    let cert = params.self_signed(&ca_key)?;

    let cert_der = cert.der().clone();
    let pem = cert.pem();
    let key_der = PrivatePkcs8KeyDer::from_pem_slice(pem.as_bytes())?;

    let key = PrivateKeyDer::Pkcs8(key_der);

    info!("成功为域名 '{}' 生成自签名证书。", domain);

    Ok(TlsCert {
        cert: vec![cert_der],
        key,
    })
}
