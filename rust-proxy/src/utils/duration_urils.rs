pub mod human_duration {
    use serde::{self, Deserialize, Deserializer, Serializer};
    use std::time::Duration;

    // 自定义解析函数，将 "10s", "5m" 等字符串转为 Duration
    fn parse_duration_str(s: &str) -> Result<Duration, String> {
        let s = s.trim();
        if let Some(num_str) = s.strip_suffix('s') {
            // 处理秒 (s)
            num_str
                .parse::<f64>()
                .map(Duration::from_secs_f64)
                .map_err(|e| e.to_string())
        } else if let Some(num_str) = s.strip_suffix("ms") {
            // 处理毫秒 (ms)
            num_str
                .parse::<u64>()
                .map(Duration::from_millis)
                .map_err(|e| e.to_string())
        } else if let Some(num_str) = s.strip_suffix('m') {
            // 处理分钟 (m)
            num_str
                .parse::<f64>()
                .map(|m| Duration::from_secs_f64(m * 60.0))
                .map_err(|e| e.to_string())
        } else {
            // 默认无单位为秒
            s.parse::<f64>()
                .map(Duration::from_secs_f64)
                .map_err(|_| format!("invalid duration format: '{s}'"))
        }
    }

    // 反序列化函数：将 YAML 中的字符串转为 Duration
    pub fn deserialize<'de, D>(deserializer: D) -> Result<Duration, D::Error>
    where
        D: Deserializer<'de>,
    {
        // 1. 先将输入反序列化为字符串
        let s = String::deserialize(deserializer)?;
        // 2. 使用我们的自定义解析函数
        parse_duration_str(&s).map_err(serde::de::Error::custom)
    }

    // 序列化函数：将 Duration 转为字符串（这里我们统一转为秒）
    pub fn serialize<S>(duration: &Duration, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let s = format!("{}s", duration.as_secs_f64());
        serializer.serialize_str(&s)
    }
}
