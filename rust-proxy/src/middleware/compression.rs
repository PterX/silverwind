use crate::vojo::app_error::AppError;
use bytes::Bytes;
use http::header::{ACCEPT_ENCODING, CONTENT_ENCODING, CONTENT_TYPE};
use http::{HeaderMap, Response};
use http_body_util::{combinators::BoxBody, Full};
use std::io::Write;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum CompressionType {
    Gzip,
    Zstd,
    #[serde(alias = "both")]
    Both,
}

impl Default for CompressionType {
    fn default() -> Self {
        CompressionType::Gzip
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct Compression {
    #[serde(default)]
    pub compression_type: CompressionType,
    #[serde(default = "default_level")]
    pub level: i32,
    #[serde(default = "default_min_size")]
    pub min_size: usize,
    #[serde(default = "default_excluded_types")]
    pub excluded_content_types: Vec<String>,
    #[serde(default)]
    pub enabled: bool,
}

fn default_level() -> i32 {
    6 // 默认压缩级别 (0-9 for gzip, 1-22 for zstd)
}

fn default_min_size() -> usize {
    1024 // 默认最小压缩大小 1KB
}

fn default_excluded_types() -> Vec<String> {
    vec![
        "image/png".to_string(),
        "image/jpeg".to_string(),
        "image/gif".to_string(),
        "image/webp".to_string(),
        "image/svg+xml".to_string(),
        "video/".to_string(),
        "audio/".to_string(),
        "application/zip".to_string(),
        "application/gzip".to_string(),
        "application/x-gzip".to_string(),
        "application/x-zip-compressed".to_string(),
        "application/wasm".to_string(),
    ]
}

impl Default for Compression {
    fn default() -> Self {
        Self {
            compression_type: CompressionType::default(),
            level: default_level(),
            min_size: default_min_size(),
            excluded_content_types: default_excluded_types(),
            enabled: true,
        }
    }
}

impl Compression {
    /// 检查是否应该压缩此内容类型
    pub fn should_compress(&self, content_type: Option<&str>) -> bool {
        if !self.enabled {
            return false;
        }

        // 检查内容类型是否在排除列表中
        if let Some(ct) = content_type {
            for excluded in &self.excluded_content_types {
                if ct.starts_with(excluded) {
                    return false;
                }
            }
        }

        true
    }

    /// 解析 Accept-Encoding 头部，选择最佳压缩算法
    pub fn parse_accept_encoding(&self, headers: &HeaderMap) -> Option<CompressionType> {
        let accept_encoding = headers.get(ACCEPT_ENCODING)?.to_str().ok()?;

        // 检查支持的压缩格式
        let supports_gzip = accept_encoding.contains("gzip") || accept_encoding.contains("*");
        let supports_zstd = accept_encoding.contains("zstd") || accept_encoding.contains("*");

        match self.compression_type {
            CompressionType::Gzip if supports_gzip => Some(CompressionType::Gzip),
            CompressionType::Zstd if supports_zstd => Some(CompressionType::Zstd),
            CompressionType::Both => {
                // 优先使用 zstd (更好的压缩率)，其次是 gzip (更广泛的兼容性)
                if supports_zstd {
                    Some(CompressionType::Zstd)
                } else if supports_gzip {
                    Some(CompressionType::Gzip)
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    /// 压缩数据
    pub fn compress_data(&self, data: &[u8], compression_type: &CompressionType) -> Result<Vec<u8>, AppError> {
        match compression_type {
            CompressionType::Gzip => {
                use flate2::write::GzEncoder;
                use flate2::Compression;

                let level = self.level.clamp(0, 9) as u32;
                let mut encoder = GzEncoder::new(Vec::new(), Compression::new(level));
                encoder.write_all(data)
                    .map_err(|e| AppError(format!("Gzip compression failed: {}", e)))?;
                encoder.finish()
                    .map_err(|e| AppError(format!("Gzip finish failed: {}", e)))
            }
            CompressionType::Zstd => {
                use zstd::stream::write::Encoder as ZstdEncoder;

                let level = self.level.clamp(1, 22);
                let mut encoder = ZstdEncoder::new(Vec::new(), level)
                    .map_err(|e| AppError(format!("Zstd encoder creation failed: {}", e)))?;
                encoder.write_all(data)
                    .map_err(|e| AppError(format!("Zstd compression failed: {}", e)))?;
                encoder.finish()
                    .map_err(|e| AppError(format!("Zstd finish failed: {}", e)))
            }
            CompressionType::Both => {
                // 这种情况不应该发生，因为 Both 在解析时会被转换为具体的类型
                Ok(data.to_vec())
            }
        }
    }

    /// 获取编码头部值
    pub fn get_encoding_value(&self, compression_type: &CompressionType) -> &'static str {
        match compression_type {
            CompressionType::Gzip => "gzip",
            CompressionType::Zstd => "zstd",
            CompressionType::Both => "gzip",
        }
    }

    /// 检查响应是否应该被压缩
    pub fn should_compress_response(
        &self,
        response: &Response<BoxBody<Bytes, AppError>>,
        request_headers: &HeaderMap,
    ) -> Option<CompressionType> {
        if !self.enabled {
            return None;
        }

        // 检查客户端支持的压缩格式
        let compression_type = self.parse_accept_encoding(request_headers)?;

        // 检查是否已经有 Content-Encoding
        if response.headers().contains_key(CONTENT_ENCODING) {
            return None;
        }

        // 获取内容类型
        let content_type = response.headers().get(CONTENT_TYPE)
            .and_then(|v| v.to_str().ok());

        // 检查是否应该压缩此内容类型
        if !self.should_compress(content_type) {
            return None;
        }

        Some(compression_type)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use http::header::HeaderMap;

    #[test]
    fn test_compression_type_default() {
        let ct = CompressionType::default();
        assert_eq!(ct, CompressionType::Gzip);
    }

    #[test]
    fn test_compression_default() {
        let comp = Compression::default();
        assert_eq!(comp.compression_type, CompressionType::Gzip);
        assert_eq!(comp.level, 6);
        assert_eq!(comp.min_size, 1024);
        assert!(comp.enabled);
    }

    #[test]
    fn test_should_compress_excluded_types() {
        let comp = Compression::default();

        // 这些类型应该被排除
        assert!(!comp.should_compress(Some("image/png")));
        assert!(!comp.should_compress(Some("image/jpeg")));
        assert!(!comp.should_compress(Some("video/mp4")));
        assert!(!comp.should_compress(Some("application/zip")));

        // 这些类型应该被压缩
        assert!(comp.should_compress(Some("text/html")));
        assert!(comp.should_compress(Some("application/json")));
        assert!(comp.should_compress(Some("text/css")));
        assert!(comp.should_compress(Some("application/javascript")));
    }

    #[test]
    fn test_parse_accept_encoding() {
        let comp = Compression::default();

        // 测试 gzip
        let mut headers = HeaderMap::new();
        headers.insert(ACCEPT_ENCODING, "gzip".parse().unwrap());
        assert_eq!(
            comp.parse_accept_encoding(&headers),
            Some(CompressionType::Gzip)
        );

        // 测试 zstd
        headers.insert(ACCEPT_ENCODING, "zstd".parse().unwrap());
        assert_eq!(
            comp.parse_accept_encoding(&headers),
            Some(CompressionType::Zstd)
        );

        // 测试两者都支持
        let comp_both = Compression {
            compression_type: CompressionType::Both,
            ..Default::default()
        };
        headers.insert(ACCEPT_ENCODING, "gzip, deflate, br".parse().unwrap());
        assert_eq!(
            comp_both.parse_accept_encoding(&headers),
            Some(CompressionType::Gzip)
        );

        headers.insert(ACCEPT_ENCODING, "zstd, gzip".parse().unwrap());
        assert_eq!(
            comp_both.parse_accept_encoding(&headers),
            Some(CompressionType::Zstd)
        );

        // 测试通配符
        headers.insert(ACCEPT_ENCODING, "*".parse().unwrap());
        assert!(comp.parse_accept_encoding(&headers).is_some());
    }

    #[test]
    fn test_compress_data_gzip() {
        let comp = Compression::default();
        let data = b"Hello, World! ".repeat(100); // 创建一些重复数据以便更好地压缩

        let compressed = comp.compress_data(&data, &CompressionType::Gzip).unwrap();

        // 压缩后的数据应该更小
        assert!(compressed.len() < data.len());
    }

    #[test]
    fn test_compress_data_zstd() {
        let comp = Compression {
            compression_type: CompressionType::Zstd,
            level: 3,
            ..Default::default()
        };
        let data = b"Hello, World! ".repeat(100);

        let compressed = comp.compress_data(&data, &CompressionType::Zstd).unwrap();

        // 压缩后的数据应该更小
        assert!(compressed.len() < data.len());
    }

    #[test]
    fn test_compress_disabled() {
        let comp = Compression {
            enabled: false,
            ..Default::default()
        };

        assert!(!comp.should_compress(Some("text/html")));
    }
}
