use std::fmt::{self, Debug, Display, Formatter};

use prometheus::proto::{Gauge, Metric, MetricFamily, MetricType};
use prometheus::Encoder;
use tokio::sync::watch;

use crate::worker::measurement_channel::MeasurementReceiver;

pub async fn serve_metrics<E>(
    req: tide::Request<MeasurementReceiver<Result<u16, E>>>,
) -> tide::Result
where
    E: Clone + Debug + Display + Send + Sync + 'static,
{
    req.state().trigger_measurement();
    let value = req
        .state()
        .clone()
        .changed()
        .await
        .map_err(ServeMetricsInternalError::SensorError)?
        .clone()
        .or(Err(ServeMetricsInternalError::<E>::WorkerDied))?;

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

#[derive(Clone, Debug)]
pub enum ServeMetricsInternalError<E> {
    SensorError(E),
    WorkerDied,
}

impl<E> From<watch::error::RecvError> for ServeMetricsInternalError<E> {
    fn from(_: watch::error::RecvError) -> Self {
        Self::WorkerDied
    }
}

impl<E: Display> Display for ServeMetricsInternalError<E> {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), fmt::Error> {
        use ServeMetricsInternalError::*;
        match self {
            SensorError(err) => Display::fmt(err, f),
            WorkerDied => f.write_str("the worker for reading measurements died"),
        }
    }
}

impl<E: Debug + Display> std::error::Error for ServeMetricsInternalError<E> {}
