use lazy_static::lazy_static;
use once_cell::sync::Lazy;
use prometheus::{labels, opts, register_counter_vec, register_gauge, register_histogram_vec};
use prometheus::{CounterVec, Gauge, HistogramVec};
pub mod metrics {
    use super::*;

    pub static HTTP_REQUESTS_TOTAL: Lazy<CounterVec> = Lazy::new(|| {
        register_counter_vec!(
            "http_requests_total",
            "Total number of HTTP requests.",
            &["mapping_key", "path", "method", "status"]
        )
        .expect("Failed to create http_requests_total counter")
    });

    pub static HTTP_REQUEST_DURATION_SECONDS: Lazy<HistogramVec> = Lazy::new(|| {
        register_histogram_vec!(
            "http_request_duration_seconds",
            "The HTTP request latencies in seconds.",
            &["mapping_key", "path", "method"]
        )
        .expect("Failed to create http_request_duration_seconds histogram")
    });
}
lazy_static! {
    static ref HTTP_COUNTER: CounterVec = register_counter_vec!(
        opts!("spire_http_requests_total", "Number of HTTP requests made.",),
        &["port", "request_path", "status_code"]
    )
    .unwrap();
    static ref HTTP_BODY_GAUGE: Gauge = register_gauge!(opts!(
        "spire_http_response_size_bytes",
        "The HTTP response sizes in bytes.",
        labels! {"handler" => "all",}
    ))
    .unwrap();
    static ref HTTP_REQ_HISTOGRAM: HistogramVec = register_histogram_vec!(
        "spire_http_request_duration_seconds",
        "The HTTP request latencies in seconds.",
        &["port", "request_path"]
    )
    .unwrap();
}
