use prometheus::{
    labels, opts, register_counter_vec, register_gauge, register_histogram_vec, CounterVec, Gauge,
    HistogramVec,
};
use std::sync::OnceLock;

pub mod metrics {
    use delay_timer::anyhow;

    use super::*;

    // --------------------------
    // All metrics defined here
    // --------------------------

    pub static HTTP_REQUESTS_TOTAL: OnceLock<CounterVec> = OnceLock::new();
    pub static HTTP_REQUEST_DURATION_SECONDS: OnceLock<HistogramVec> = OnceLock::new();

    pub static HTTP_COUNTER: OnceLock<CounterVec> = OnceLock::new();
    pub static HTTP_BODY_GAUGE: OnceLock<Gauge> = OnceLock::new();
    pub static HTTP_REQ_HISTOGRAM: OnceLock<HistogramVec> = OnceLock::new();

    // --------------------------
    // Unified initialization
    // --------------------------

    pub fn init() -> Result<(), anyhow::Error> {
        let register_count_vec = register_counter_vec!(
            "http_requests_total",
            "Total number of HTTP requests.",
            &["mapping_key", "path", "method", "status"]
        )?;
        HTTP_REQUESTS_TOTAL.get_or_init(|| register_count_vec);

        let register_duration_histogram = register_histogram_vec!(
            "http_request_duration_seconds",
            "The HTTP request latencies in seconds.",
            &["mapping_key", "path", "method"]
        )?;
        HTTP_REQUEST_DURATION_SECONDS.get_or_init(|| register_duration_histogram);

        let register_http_counter = register_counter_vec!(
            opts!("spire_http_requests_total", "Number of HTTP requests made."),
            &["port", "request_path", "status_code"]
        )?;
        HTTP_COUNTER.get_or_init(|| register_http_counter);

        let register_body_gauge = register_gauge!(opts!(
            "spire_http_response_size_bytes",
            "The HTTP response sizes in bytes.",
            labels! {"handler" => "all",}
        ))?;
        HTTP_BODY_GAUGE.get_or_init(|| register_body_gauge);

        let register_req_histogram = register_histogram_vec!(
            "spire_http_request_duration_seconds",
            "The HTTP request latencies in seconds.",
            &["port", "request_path"]
        )?;
        HTTP_REQ_HISTOGRAM.get_or_init(|| register_req_histogram);
        Ok(())
    }
}
