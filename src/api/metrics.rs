use prometheus::proto::{Gauge, Metric, MetricFamily, MetricType};
use prometheus::Encoder;

use crate::worker::measurement_channel::MeasurementReceiver;

pub async fn serve_metrics(req: tide::Request<MeasurementReceiver<u16>>) -> tide::Result {
    req.state().trigger_measurement();
    let value = req.state().clone().changed().await?.clone();

    let mut buffer = vec![];
    let encoder = prometheus::TextEncoder::new();
    encoder.encode(&[co2_metric(value as f64)], &mut buffer)?;
    Ok(format!("{}", String::from_utf8(buffer)?).into())
}

fn co2_metric(value: f64) -> MetricFamily {
    let mut gauge = Gauge::new();
    gauge.set_value(value);

    let mut metric = Metric::new();
    metric.set_gauge(gauge);

    let mut metric_family = MetricFamily::new();
    metric_family.set_name("co2_ppm".into());
    metric_family.set_help("CO2 concentration [ppm]".into());
    metric_family.set_field_type(MetricType::GAUGE);
    metric_family.set_metric(protobuf::RepeatedField::from_slice(&[metric]));
    metric_family
}
