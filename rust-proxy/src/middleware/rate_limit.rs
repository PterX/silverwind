use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::constants::common_constants::DEFAULT_FIXEDWINDOW_MAP_SIZE;
use crate::vojo::app_error::AppError;
use core::fmt::Debug;
use http::header;
use http::HeaderMap;
use http::HeaderName;
use http::HeaderValue;
use ipnet::Ipv4Net;
use iprange::IpRange;
use serde::{Deserialize, Serialize};
use std::net::Ipv4Addr;
use std::net::SocketAddr;
use std::time::Duration;
const X_RATELIMIT_LIMIT: HeaderName = HeaderName::from_static("x-ratelimit-limit");
const X_RATELIMIT_REMAINING: HeaderName = HeaderName::from_static("x-ratelimit-remaining");
const X_RATELIMIT_RESET: HeaderName = HeaderName::from_static("x-ratelimit-reset");
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "limiter", rename_all = "PascalCase")]
pub enum Ratelimit {
    #[serde(rename = "token_bucket")]
    TokenBucket(TokenBucketRateLimit),
    #[serde(rename = "fixed_window")]
    FixedWindow(FixedWindowRateLimit),
}
impl Ratelimit {
    pub fn should_limit(
        &mut self,
        headers: &HeaderMap<HeaderValue>,
        peer_addr: &SocketAddr,
    ) -> Result<Option<HeaderMap>, AppError> {
        match self {
            Ratelimit::TokenBucket(tb) => tb.should_limit(headers, peer_addr),
            Ratelimit::FixedWindow(fw) => fw.should_limit(headers, peer_addr),
        }
    }
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IPBasedRatelimit {
    pub value: String,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HeaderBasedRatelimit {
    pub key: String,
    pub value: String,
}
impl HeaderBasedRatelimit {
    fn get_key(&self) -> String {
        format!("{}:{}", self.key, self.value)
    }
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IpRangeBasedRatelimit {
    pub value: String,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum LimitLocation {
    IP(IPBasedRatelimit),
    Header(HeaderBasedRatelimit),
    Iprange(IpRangeBasedRatelimit),
}
impl Default for LimitLocation {
    fn default() -> Self {
        LimitLocation::IP(IPBasedRatelimit {
            value: String::new(),
        })
    }
}
impl LimitLocation {
    pub fn get_key(&self) -> String {
        match self {
            LimitLocation::Header(headers) => headers.get_key(),
            LimitLocation::IP(ip) => ip.value.clone(),
            LimitLocation::Iprange(ip_range) => ip_range.value.clone(),
        }
    }
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind")]
#[derive(Default)]
pub enum TimeUnit {
    #[default]
    MillionSecond,
    Second,
    Minute,
    Hour,
    Day,
}
impl TimeUnit {
    pub fn get_million_second(&self) -> u128 {
        match self {
            Self::MillionSecond => 1,
            Self::Second => 1_000,
            Self::Minute => 60_000,
            Self::Hour => 3_600_000,
            Self::Day => 86_400_000,
        }
    }
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TokenBucketRateLimit {
    pub rate_per_unit: i32,
    pub unit: TimeUnit,
    pub capacity: i32,
    pub scope: LimitLocation,
    #[serde(skip_serializing, skip_deserializing)]
    pub current_count: i32,
    #[serde(skip_serializing, skip_deserializing, default = "default_time")]
    pub last_update_time: SystemTime,
}
impl Default for TokenBucketRateLimit {
    fn default() -> Self {
        TokenBucketRateLimit {
            last_update_time: SystemTime::now(),
            rate_per_unit: 0,
            unit: TimeUnit::default(),
            capacity: 0,
            scope: LimitLocation::default(),
            current_count: 0,
        }
    }
}
fn default_time() -> SystemTime {
    SystemTime::now()
}
fn get_time_key(time_unit: TimeUnit) -> Result<String, AppError> {
    let current_time = SystemTime::now();
    let since_the_epoch = current_time.duration_since(UNIX_EPOCH)?;
    let in_ms =
        since_the_epoch.as_secs() * 1000 + since_the_epoch.subsec_nanos() as u64 / 1_000_000;
    let key_u64 = match time_unit {
        TimeUnit::MillionSecond => in_ms,
        TimeUnit::Second => in_ms / 1000,
        TimeUnit::Minute => in_ms / 60000,
        TimeUnit::Hour => in_ms / 3600000,
        TimeUnit::Day => in_ms / 86400000,
    };
    Ok(key_u64.to_string())
}

fn matched(
    limit_location: LimitLocation,
    headers: &HeaderMap<HeaderValue>,
    peer_addr: &SocketAddr,
) -> Result<bool, AppError> {
    let remote_ip = peer_addr.ip().to_string();
    match limit_location {
        LimitLocation::IP(ip_based_ratelimit) => Ok(ip_based_ratelimit.value == remote_ip),
        LimitLocation::Header(header_based_ratelimit) => {
            if !headers.contains_key(header_based_ratelimit.key.clone()) {
                return Ok(false);
            }
            let header_value = headers
                .get(header_based_ratelimit.key.clone())
                .ok_or("Can not find the header_based_ratelimit key.")?;
            let header_value_str = header_value.to_str()?;

            Ok(header_value_str == header_based_ratelimit.value)
        }
        LimitLocation::Iprange(ip_range_based_ratelimit) => {
            if !ip_range_based_ratelimit.value.contains('/') {
                return Err(AppError(("The Ip Range should contain '/'.").to_string()));
            }
            let ip_range: IpRange<Ipv4Net> = [ip_range_based_ratelimit.value]
                .iter()
                .map(|s| s.parse::<Ipv4Net>().map_err(|e| AppError(e.to_string())))
                .collect::<Result<IpRange<Ipv4Net>, AppError>>()?;
            let source_ip = remote_ip.parse::<Ipv4Addr>()?;
            Ok(ip_range.contains(&source_ip))
        }
    }
}

impl TokenBucketRateLimit {
    fn should_limit(
        &mut self,
        headers: &HeaderMap<HeaderValue>,
        peer_addr: &SocketAddr,
    ) -> Result<Option<HeaderMap>, AppError> {
        if !matched(self.scope.clone(), headers, peer_addr)? {
            return Ok(None);
        }

        let now = SystemTime::now();
        let elapsed = now.duration_since(self.last_update_time)?;

        let elapsed_millis = elapsed.as_millis();
        let tokens_to_add =
            (elapsed_millis * self.rate_per_unit as u128) / self.unit.get_million_second();

        if tokens_to_add > 0 {
            self.current_count = (self.current_count + tokens_to_add as i32).min(self.capacity);
            self.last_update_time = now;
        }

        if self.current_count > 0 {
            self.current_count -= 1;
            Ok(None) // Not limited
        } else {
            // --- 这是你需要修改的核心部分 ---
            let mut response_headers = HeaderMap::new();

            // 1. 计算 Retry-After: 需要等待多久才能获得下一个令牌
            //    时间(毫秒) = 单位时间(毫秒) / 该单位时间生成的令牌数
            let millis_for_one_token = self.unit.get_million_second() / self.rate_per_unit as u128;
            //    Retry-After 标准使用秒，所以向上取整
            let retry_after_seconds = (millis_for_one_token as f64 / 1000.0).ceil() as u64;

            // 2. 计算 X-RateLimit-Reset: 令牌桶重置（有下一个可用令牌）的绝对时间戳
            let reset_time =
                self.last_update_time + Duration::from_millis(millis_for_one_token as u64);
            let reset_timestamp = reset_time.duration_since(UNIX_EPOCH)?.as_secs();

            // 3. 填充 HeaderMap
            // X-RateLimit-Limit: 桶的总容量
            response_headers.insert(X_RATELIMIT_LIMIT, HeaderValue::from(self.capacity));

            // X-RateLimit-Remaining: 剩余令牌数，此时为 0
            response_headers.insert(X_RATELIMIT_REMAINING, HeaderValue::from(0));

            // X-RateLimit-Reset: 重置时间的 Unix 时间戳
            response_headers.insert(X_RATELIMIT_RESET, HeaderValue::from(reset_timestamp));

            // Retry-After: 建议的重试秒数
            response_headers.insert(header::RETRY_AFTER, HeaderValue::from(retry_after_seconds));

            Ok(Some(response_headers)) // 返回包含限流信息的头部
        }
    }
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]

pub struct FixedWindowRateLimit {
    pub rate_per_unit: u128,
    pub unit: TimeUnit,
    pub limit_location: LimitLocation,
    #[serde(skip_serializing, skip_deserializing)]
    pub count_map: HashMap<String, i32>,
}
impl FixedWindowRateLimit {
    fn should_limit(
        &mut self,
        headers: &HeaderMap<HeaderValue>,
        peer_addr: &SocketAddr,
    ) -> Result<Option<HeaderMap>, AppError> {
        if !matched(self.limit_location.clone(), headers, peer_addr)? {
            return Ok(None);
        }

        let time_unit_key = get_time_key(self.unit.clone())?;
        let location_key = self.limit_location.get_key();
        let key = format!("{}:{}", location_key, time_unit_key);

        if self.count_map.len() >= DEFAULT_FIXEDWINDOW_MAP_SIZE as usize {
            if let Some(oldest_key) = self.count_map.keys().next().cloned() {
                self.count_map.remove(&oldest_key);
            }
        }
        let counter = self.count_map.entry(key).or_insert(0);
        *counter += 1;
        let remaining_requests = self.rate_per_unit as i32 - *counter;

        if remaining_requests >= 0 {
            Ok(None)
        } else {
            let mut response_headers = HeaderMap::new();

            let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
            let window_size = (self.unit.get_million_second() / 1000) as u64;
            let reset_timestamp = (now - (now % window_size)) + window_size;

            let retry_after_seconds = reset_timestamp - now;
            response_headers.insert(
                X_RATELIMIT_LIMIT,
                HeaderValue::from(self.rate_per_unit as u64),
            );
            response_headers.insert(X_RATELIMIT_REMAINING, HeaderValue::from(0));
            response_headers.insert(X_RATELIMIT_RESET, HeaderValue::from(reset_timestamp));
            response_headers.insert(header::RETRY_AFTER, HeaderValue::from(retry_after_seconds));

            Ok(Some(response_headers))
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr};

    #[test]
    fn test_token_bucket_rate_limit() {
        let mut headers = HeaderMap::new();
        headers.insert("test-header", "test-value".parse().unwrap());

        let socket_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080);

        let mut rate_limit = TokenBucketRateLimit {
            rate_per_unit: 10,
            unit: TimeUnit::Second,
            capacity: 10,
            scope: LimitLocation::IP(IPBasedRatelimit {
                value: "127.0.0.1".to_string(),
            }),
            current_count: 5,
            last_update_time: SystemTime::now(),
        };

        assert!(matches!(
            rate_limit.should_limit(&headers, &socket_addr),
            Ok(None)
        ),);

        rate_limit.current_count = 0;
        assert!(matches!(
            rate_limit.should_limit(&headers, &socket_addr),
            Ok(Some(_))
        ));
    }

    #[test]
    fn test_fixed_window_rate_limit() {
        let mut headers = HeaderMap::new();
        headers.insert("test-header", "test-value".parse().unwrap());

        let socket_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080);

        let mut rate_limit = FixedWindowRateLimit {
            rate_per_unit: 2,
            unit: TimeUnit::Second,
            limit_location: LimitLocation::IP(IPBasedRatelimit {
                value: "127.0.0.1".to_string(),
            }),
            count_map: HashMap::new(),
        };

        assert!(matches!(
            rate_limit.should_limit(&headers, &socket_addr),
            Ok(None)
        ));
        assert!(matches!(
            rate_limit.should_limit(&headers, &socket_addr),
            Ok(None)
        ));
        assert!(matches!(
            rate_limit.should_limit(&headers, &socket_addr),
            Ok(Some(_))
        ));
    }

    #[test]
    fn test_ip_range_rate_limit() {
        let headers = HeaderMap::new();
        let socket_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1)), 8080);

        let mut rate_limit = TokenBucketRateLimit {
            rate_per_unit: 10,
            unit: TimeUnit::Second,
            capacity: 10,
            scope: LimitLocation::Iprange(IpRangeBasedRatelimit {
                value: "192.168.1.0/24".to_string(),
            }),
            current_count: 5,
            last_update_time: SystemTime::now(),
        };

        assert!(matches!(
            rate_limit.should_limit(&headers, &socket_addr),
            Ok(None)
        ));
        let socket_addr_outside = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 2, 1)), 8080);
        assert!(matches!(
            rate_limit.should_limit(&headers, &socket_addr),
            Ok(None)
        ));
    }

    #[test]
    fn test_header_based_rate_limit() {
        let mut headers = HeaderMap::new();
        headers.insert("X-API-Key", "test-key".parse().unwrap());

        let socket_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080);

        let mut rate_limit = TokenBucketRateLimit {
            rate_per_unit: 10,
            unit: TimeUnit::Second,
            capacity: 10,
            scope: LimitLocation::Header(HeaderBasedRatelimit {
                key: "X-API-Key".to_string(),
                value: "test-key".to_string(),
            }),
            current_count: 5,
            last_update_time: SystemTime::now(),
        };

        assert!(matches!(
            rate_limit.should_limit(&headers, &socket_addr),
            Ok(None)
        ));

        headers.insert("X-API-Key", "wrong-key".parse().unwrap());
        assert!(matches!(
            rate_limit.should_limit(&headers, &socket_addr),
            Ok(None)
        ));
    }
}
