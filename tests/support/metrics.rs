//! Test-only queries over the OpenTelemetry SDK's official in-memory metrics exporter.

use std::{collections::BTreeMap, time::Duration};

use openbridge::observability::GatewayMetrics;
use opentelemetry::{KeyValue, Value, metrics::MeterProvider as _};
use opentelemetry_sdk::metrics::{
    InMemoryMetricExporter, InMemoryMetricExporterBuilder, PeriodicReader, SdkMeterProvider,
    Temporality,
    data::{AggregatedMetrics, HistogramDataPoint, Metric, MetricData, ResourceMetrics},
};
use serde::Serialize;

/// Test-owned SDK provider and instrument set with synchronous collection queries.
pub struct TestMetrics {
    instruments: GatewayMetrics,
    provider: SdkMeterProvider,
    exporter: InMemoryMetricExporter,
}

impl TestMetrics {
    /// Creates a cumulative in-memory SDK pipeline without production snapshot storage.
    pub fn new() -> Self {
        // Use the official test exporter behind a long-period reader; tests force collection explicitly.
        let exporter = InMemoryMetricExporterBuilder::new()
            .with_temporality(Temporality::Cumulative)
            .build();
        let reader = PeriodicReader::builder(exporter.clone())
            .with_interval(Duration::from_secs(3_600))
            .build();
        let provider = SdkMeterProvider::builder().with_reader(reader).build();
        let instruments = GatewayMetrics::from_meter(provider.meter("openbridge"));
        Self {
            instruments,
            provider,
            exporter,
        }
    }

    /// Returns cloneable instruments for injection into `GatewayState`.
    pub fn instruments(&self) -> GatewayMetrics {
        self.instruments.clone()
    }

    /// Collects and projects gateway totals from the latest cumulative SDK export.
    pub fn snapshot(&self) -> GatewayMetricsSnapshot {
        let exports = self.collect();
        let Some(metrics) = exports.last() else {
            return GatewayMetricsSnapshot::default();
        };
        let providers = provider_snapshots_from(metrics);
        let mut snapshot = GatewayMetricsSnapshot::default();

        // Read gateway and routing sums directly from their SDK data points.
        for metric in all_metrics(metrics) {
            match metric.name() {
                "openbridge.downstream.request.started" => {
                    snapshot.requests_started = sum_u64(metric)
                }
                "openbridge.downstream.request.duration" => {
                    for point in histogram_f64_points(metric) {
                        let attributes = attributes(point);
                        let value = point.count();
                        merge_timing(&mut snapshot.request_duration_ms, point);
                        match string_attribute(&attributes, "openbridge.request.outcome") {
                            Some("completed") => snapshot.requests_completed += value,
                            Some("http_failed") => snapshot.requests_http_failed += value,
                            Some("failed") => snapshot.requests_failed += value,
                            Some("cancelled") => snapshot.requests_cancelled += value,
                            _ => {}
                        }
                    }
                }
                "openbridge.downstream.response_ready.duration" => {
                    for point in histogram_f64_points(metric) {
                        merge_timing(&mut snapshot.response_ready_ms, point);
                    }
                }
                "openbridge.downstream.time_to_first_output" => {
                    for point in histogram_f64_points(metric) {
                        merge_timing(&mut snapshot.time_to_first_output_ms, point);
                    }
                }
                "openbridge.routing.events" => {
                    for (attributes, value) in sum_u64_points(metric) {
                        match string_attribute(&attributes, "openbridge.routing.event") {
                            Some("retry") => snapshot.upstream_retries += value,
                            Some("credential_rotation") => snapshot.credential_rotations += value,
                            Some("route_fallback") => snapshot.route_fallbacks += value,
                            Some("cooldown_skip") => snapshot.cooldown_skips += value,
                            _ => {}
                        }
                    }
                }
                _ => {}
            }
        }

        // Derive Provider and usage totals from the same exported aggregate points.
        for provider in providers {
            snapshot.upstream_attempts += provider.attempts_started;
            snapshot.upstream_http_failures += provider.attempts_http_failed;
            snapshot.upstream_transport_failures += provider.attempts_transport_failed;
            snapshot.usage_observations += provider.usage_observations;
            snapshot.input_tokens += provider.input_tokens;
            snapshot.output_tokens += provider.output_tokens;
            snapshot.total_tokens += provider.total_tokens;
        }
        snapshot
    }

    /// Collects and projects Provider aggregates from native SDK sums and histograms.
    pub fn provider_snapshots(&self) -> Vec<ProviderMetricSnapshot> {
        let exports = self.collect();
        exports
            .last()
            .map(provider_snapshots_from)
            .unwrap_or_default()
    }

    /// Forces one synchronous SDK collection and returns every captured cumulative export.
    fn collect(&self) -> Vec<ResourceMetrics> {
        self.provider.force_flush().unwrap();
        self.exporter.get_finished_metrics().unwrap()
    }
}

impl Default for TestMetrics {
    fn default() -> Self {
        Self::new()
    }
}

/// Test projection of gateway-level sums formerly asserted through production snapshots.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize)]
pub struct GatewayMetricsSnapshot {
    pub requests_started: u64,
    pub requests_completed: u64,
    pub requests_http_failed: u64,
    pub requests_failed: u64,
    pub requests_cancelled: u64,
    pub request_duration_ms: TimingSnapshot,
    pub response_ready_ms: TimingSnapshot,
    pub time_to_first_output_ms: TimingSnapshot,
    pub upstream_attempts: u64,
    pub upstream_http_failures: u64,
    pub upstream_transport_failures: u64,
    pub upstream_retries: u64,
    pub credential_rotations: u64,
    pub route_fallbacks: u64,
    pub cooldown_skips: u64,
    pub usage_observations: u64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub total_tokens: u64,
}

/// Test projection of one trusted Provider attribute set.
#[derive(Clone, Debug, Default, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub struct ProviderMetricKey {
    pub provider: String,
    pub route_id: String,
    pub upstream_target: String,
    pub upstream_operation: String,
    pub public_model: String,
    pub operation: String,
    pub route_mode: String,
    pub streaming: bool,
}

/// Test projection of one duration histogram data point.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize)]
pub struct TimingSnapshot {
    pub count: u64,
    pub sum_ms: u64,
    pub min_ms: Option<u64>,
    pub max_ms: Option<u64>,
}

/// Test projection of one output-rate histogram data point.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize)]
pub struct RateSnapshot {
    pub count: u64,
    pub sum_milli_tokens_per_second: u64,
    pub min_milli_tokens_per_second: Option<u64>,
    pub max_milli_tokens_per_second: Option<u64>,
}

/// Test projection of SDK aggregates grouped by trusted Provider dimensions.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct ProviderMetricSnapshot {
    pub key: ProviderMetricKey,
    pub attempts_started: u64,
    pub attempts_completed: u64,
    pub attempts_http_failed: u64,
    pub attempts_transport_failed: u64,
    pub attempts_stream_failed: u64,
    pub attempts_cancelled: u64,
    pub response_ready_ms: TimingSnapshot,
    pub upstream_first_byte_ms: TimingSnapshot,
    pub upstream_ttft_ms: TimingSnapshot,
    pub duration_ms: TimingSnapshot,
    pub generation_duration_ms: TimingSnapshot,
    pub output_speed: RateSnapshot,
    pub usage_observations: u64,
    pub input_token_observations: u64,
    pub output_token_observations: u64,
    pub reasoning_output_token_observations: u64,
    pub total_token_observations: u64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub reasoning_output_tokens: u64,
    pub total_tokens: u64,
    pub cache_observations: u64,
    pub cache_read_observations: u64,
    pub cache_hit_requests: u64,
    pub cached_input_tokens: u64,
    pub cache_write_input_tokens: u64,
}

/// Projects every Provider-scoped SDK point into deterministic test aggregates.
fn provider_snapshots_from(metrics: &ResourceMetrics) -> Vec<ProviderMetricSnapshot> {
    let mut snapshots = BTreeMap::<ProviderMetricKey, ProviderMetricSnapshot>::new();

    // Merge each instrument's native aggregate into its trusted Provider attribute set.
    for metric in all_metrics(metrics) {
        match metric.name() {
            "openbridge.provider.attempt.started" => {
                for (attributes, value) in sum_u64_points(metric) {
                    snapshot_for(&mut snapshots, &attributes).attempts_started += value;
                }
            }

            "openbridge.provider.response_ready.duration" => {
                merge_duration_metric(&mut snapshots, metric, |snapshot| {
                    &mut snapshot.response_ready_ms
                })
            }
            "openbridge.provider.first_byte.duration" => {
                merge_duration_metric(&mut snapshots, metric, |snapshot| {
                    &mut snapshot.upstream_first_byte_ms
                })
            }
            "openbridge.provider.time_to_first_token" => {
                merge_duration_metric(&mut snapshots, metric, |snapshot| {
                    &mut snapshot.upstream_ttft_ms
                })
            }
            "openbridge.provider.attempt.duration" => {
                for point in histogram_f64_points(metric) {
                    let point_attributes = attributes(point);
                    let snapshot = snapshot_for(&mut snapshots, &point_attributes);
                    let count = point.count();
                    match string_attribute(&point_attributes, "openbridge.attempt.outcome") {
                        Some("completed") => snapshot.attempts_completed += count,
                        Some("http_failed") => snapshot.attempts_http_failed += count,
                        Some("transport_failed") => snapshot.attempts_transport_failed += count,
                        Some("stream_failed") => snapshot.attempts_stream_failed += count,
                        Some("cancelled") => snapshot.attempts_cancelled += count,
                        _ => {}
                    }
                    merge_timing(&mut snapshot.duration_ms, point);
                }
            }
            "openbridge.provider.generation.duration" => {
                merge_duration_metric(&mut snapshots, metric, |snapshot| {
                    &mut snapshot.generation_duration_ms
                })
            }
            "openbridge.provider.output.speed" => {
                for point in histogram_f64_points(metric) {
                    let snapshot = snapshot_for(&mut snapshots, &attributes(point));
                    merge_rate(&mut snapshot.output_speed, point);
                }
            }
            "gen_ai.client.token.usage" => {
                for point in histogram_u64_points(metric) {
                    let point_attributes = attributes(point);
                    let snapshot = snapshot_for(&mut snapshots, &point_attributes);
                    match string_attribute(&point_attributes, "gen_ai.token.type") {
                        Some("input") => {
                            snapshot.input_token_observations += point.count();
                            snapshot.input_tokens += point.sum();
                        }
                        Some("output") => {
                            snapshot.output_token_observations += point.count();
                            snapshot.output_tokens += point.sum();
                        }
                        _ => {}
                    }
                }
            }
            "openbridge.provider.reasoning.output.token.usage" => {
                for point in histogram_u64_points(metric) {
                    let point_attributes = attributes(point);
                    let snapshot = snapshot_for(&mut snapshots, &point_attributes);
                    snapshot.reasoning_output_token_observations += point.count();
                    snapshot.reasoning_output_tokens += point.sum();
                }
            }
            "openbridge.provider.cache.read.token.usage" => {
                for point in histogram_u64_points(metric) {
                    let point_attributes = attributes(point);
                    let snapshot = snapshot_for(&mut snapshots, &point_attributes);
                    snapshot.cache_read_observations += point.count();
                    snapshot.cached_input_tokens += point.sum();
                }
            }
            "openbridge.provider.cache.write.token.usage" => {
                for point in histogram_u64_points(metric) {
                    let point_attributes = attributes(point);
                    let snapshot = snapshot_for(&mut snapshots, &point_attributes);
                    snapshot.cache_write_input_tokens += point.sum();
                    snapshot.cache_observations += point.count();
                }
            }
            "openbridge.provider.cache.requests" => {
                for (attributes, value) in sum_u64_points(metric) {
                    let snapshot = snapshot_for(&mut snapshots, &attributes);
                    if string_attribute(&attributes, "openbridge.cache.result") == Some("hit") {
                        snapshot.cache_hit_requests += value;
                    }
                }
            }
            _ => {}
        }
    }

    // Derive usage and total-token observations that the standard GenAI metric intentionally omits.
    for snapshot in snapshots.values_mut() {
        snapshot.usage_observations = snapshot
            .input_token_observations
            .max(snapshot.output_token_observations);
        snapshot.total_token_observations = snapshot.usage_observations;
        snapshot.total_tokens = snapshot.input_tokens.saturating_add(snapshot.output_tokens);
        snapshot.cache_observations = snapshot
            .cache_observations
            .max(snapshot.cache_read_observations);
    }
    snapshots.into_values().collect()
}

/// Returns all metrics from every instrumentation scope in one collection.
fn all_metrics(metrics: &ResourceMetrics) -> impl Iterator<Item = &Metric> {
    metrics.scope_metrics().flat_map(|scope| scope.metrics())
}

/// Returns all u64 sum points from one counter metric.
fn sum_u64_points(metric: &Metric) -> Vec<(Vec<KeyValue>, u64)> {
    match metric.data() {
        AggregatedMetrics::U64(MetricData::Sum(sum)) => sum
            .data_points()
            .map(|point| (point.attributes().cloned().collect(), point.value()))
            .collect(),
        _ => Vec::new(),
    }
}

/// Returns the total across every u64 counter attribute set.
fn sum_u64(metric: &Metric) -> u64 {
    sum_u64_points(metric)
        .into_iter()
        .map(|(_, value)| value)
        .sum()
}

/// Returns all f64 histogram points from one metric.
fn histogram_f64_points(metric: &Metric) -> Vec<&HistogramDataPoint<f64>> {
    match metric.data() {
        AggregatedMetrics::F64(MetricData::Histogram(histogram)) => {
            histogram.data_points().collect()
        }
        _ => Vec::new(),
    }
}

/// Returns all u64 histogram points from one metric.
fn histogram_u64_points(metric: &Metric) -> Vec<&HistogramDataPoint<u64>> {
    match metric.data() {
        AggregatedMetrics::U64(MetricData::Histogram(histogram)) => {
            histogram.data_points().collect()
        }
        _ => Vec::new(),
    }
}

/// Copies one data point's attributes for grouping outside its iterator lifetime.
fn attributes<T>(point: &HistogramDataPoint<T>) -> Vec<KeyValue> {
    point.attributes().cloned().collect()
}

/// Returns or creates one snapshot keyed by the complete trusted Provider dimensions.
fn snapshot_for<'a>(
    snapshots: &'a mut BTreeMap<ProviderMetricKey, ProviderMetricSnapshot>,
    attributes: &[KeyValue],
) -> &'a mut ProviderMetricSnapshot {
    let key = ProviderMetricKey {
        provider: string_attribute(attributes, "openbridge.provider.name")
            .or_else(|| string_attribute(attributes, "gen_ai.provider.name"))
            .unwrap_or_else(|| panic!("missing Provider metric attribute"))
            .to_owned(),
        route_id: required_string(attributes, "openbridge.route.id"),
        upstream_target: required_string(attributes, "openbridge.upstream.target"),
        upstream_operation: required_string(attributes, "openbridge.upstream.operation"),
        public_model: required_string(attributes, "openbridge.public_model"),
        operation: required_string(attributes, "openbridge.downstream.operation"),
        route_mode: required_string(attributes, "openbridge.route.mode"),
        streaming: bool_attribute(attributes, "gen_ai.request.stream").unwrap_or(false),
    };
    snapshots
        .entry(key.clone())
        .or_insert_with(|| ProviderMetricSnapshot {
            key,
            ..ProviderMetricSnapshot::default()
        })
}

/// Merges every f64 seconds histogram point into one millisecond timing projection.
fn merge_duration_metric(
    snapshots: &mut BTreeMap<ProviderMetricKey, ProviderMetricSnapshot>,
    metric: &Metric,
    select: fn(&mut ProviderMetricSnapshot) -> &mut TimingSnapshot,
) {
    for point in histogram_f64_points(metric) {
        let point_attributes = attributes(point);
        let timing = select(snapshot_for(snapshots, &point_attributes));
        merge_timing(timing, point);
    }
}

/// Adds one native duration histogram point to the test millisecond projection.
fn merge_timing(target: &mut TimingSnapshot, point: &HistogramDataPoint<f64>) {
    target.count += point.count();
    target.sum_ms += seconds_to_millis(point.sum());
    target.min_ms = merge_min(target.min_ms, point.min().map(seconds_to_millis));
    target.max_ms = merge_max(target.max_ms, point.max().map(seconds_to_millis));
}

/// Adds one native output-rate histogram point to the test fixed-point projection.
fn merge_rate(target: &mut RateSnapshot, point: &HistogramDataPoint<f64>) {
    target.count += point.count();
    target.sum_milli_tokens_per_second += rate_to_milli(point.sum());
    target.min_milli_tokens_per_second = merge_min(
        target.min_milli_tokens_per_second,
        point.min().map(rate_to_milli),
    );
    target.max_milli_tokens_per_second = merge_max(
        target.max_milli_tokens_per_second,
        point.max().map(rate_to_milli),
    );
}

fn seconds_to_millis(value: f64) -> u64 {
    (value * 1_000.0).round().max(0.0) as u64
}

fn rate_to_milli(value: f64) -> u64 {
    (value * 1_000.0).round().max(0.0) as u64
}

fn merge_min(current: Option<u64>, value: Option<u64>) -> Option<u64> {
    match (current, value) {
        (Some(current), Some(value)) => Some(current.min(value)),
        (current, value) => current.or(value),
    }
}

fn merge_max(current: Option<u64>, value: Option<u64>) -> Option<u64> {
    match (current, value) {
        (Some(current), Some(value)) => Some(current.max(value)),
        (current, value) => current.or(value),
    }
}

fn required_string(attributes: &[KeyValue], key: &str) -> String {
    string_attribute(attributes, key)
        .unwrap_or_else(|| panic!("missing metric attribute {key}"))
        .to_owned()
}

fn string_attribute<'a>(attributes: &'a [KeyValue], key: &str) -> Option<&'a str> {
    attributes
        .iter()
        .find(|attribute| attribute.key.as_str() == key)
        .and_then(|attribute| match &attribute.value {
            Value::String(value) => Some(value.as_str()),
            _ => None,
        })
}

fn bool_attribute(attributes: &[KeyValue], key: &str) -> Option<bool> {
    attributes
        .iter()
        .find(|attribute| attribute.key.as_str() == key)
        .and_then(|attribute| match attribute.value {
            Value::Bool(value) => Some(value),
            _ => None,
        })
}
