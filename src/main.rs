use co2_metrics_exporter::measurement_channel::{
    measurement_channel, MeasurementReceiver, MeasurementSender,
};
use mh_z19c::{self, MhZ19C};
use nb::block;
use prometheus::proto::{Gauge, Metric, MetricFamily, MetricType};
use prometheus::{self, Encoder};
use protobuf;
use rppal::uart::{Parity, Uart};
use std::convert::{From, Into};
use std::fmt::{self, Display, Formatter};
use std::sync::Arc;
use tide;
use tokio::sync::watch;
use tokio::task;

trait Co2Sensor {
    type Error;

    fn read_co2_ppm(&mut self) -> nb::Result<u16, Self::Error>;
}

impl Co2Sensor for MhZ19C<'_, Uart, rppal::uart::Error> {
    type Error = Arc<mh_z19c::Error<rppal::uart::Error>>;

    fn read_co2_ppm(&mut self) -> nb::Result<u16, Self::Error> {
        self.read_co2_ppm().map_err(|e| e.map(Arc::new))
    }
}

async fn co2_sensing_worker<C: Co2Sensor>(
    mut co2_sensor: C,
    sender: MeasurementSender<Result<u16, C::Error>>,
) -> C {
    loop {
        sender.notified().await;
        let value = block!(co2_sensor.read_co2_ppm());
        if sender.send_measurement(value).is_err() {
            return co2_sensor;
        }
    }
}

async fn serve_metrics(
    req: tide::Request<MeasurementReceiver<Result<u16, Arc<mh_z19c::Error<rppal::uart::Error>>>>>,
) -> tide::Result {
    req.state().trigger_measurement();
    let value = req
        .state()
        .clone()
        .changed()
        .await
        .map_err(ServeMetricsInternalError::from)?
        .clone()
        .map_err(ServeMetricsInternalError::from)?;

    let mut buffer = vec![];
    let encoder = prometheus::TextEncoder::new();
    encoder.encode(&[co2_metric(value as f64)], &mut buffer)?;
    Ok(format!("{}", String::from_utf8(buffer)?).into())
}

#[derive(Clone, Debug)]
enum ServeMetricsInternalError {
    SensorError(Arc<mh_z19c::Error<rppal::uart::Error>>),
    WorkerDied,
}

impl From<Arc<mh_z19c::Error<rppal::uart::Error>>> for ServeMetricsInternalError {
    fn from(err: Arc<mh_z19c::Error<rppal::uart::Error>>) -> Self {
        Self::SensorError(err)
    }
}

impl From<watch::error::RecvError> for ServeMetricsInternalError {
    fn from(_: watch::error::RecvError) -> Self {
        Self::WorkerDied
    }
}

impl Display for ServeMetricsInternalError {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), fmt::Error> {
        use ServeMetricsInternalError::*;
        match self {
            SensorError(err) => err.fmt(f),
            WorkerDied => f.write_str("the worker for reading measurements died"),
        }
    }
}

impl std::error::Error for ServeMetricsInternalError {}

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

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (tx, rx) = measurement_channel(Ok(0u16));

    let co2sensor = MhZ19C::new(Uart::with_path("/dev/ttyAMA0", 9600, Parity::None, 8, 1)?);
    task::spawn(async move { co2_sensing_worker(co2sensor, tx).await });

    let mut app = tide::with_state(rx);
    app.at("/metrics").get(serve_metrics);
    println!("Spawning server ...");
    app.listen("0.0.0.0:9119").await?;
    println!("Shutdown.");

    Ok(())
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use std::collections::VecDeque;
    use std::fmt::{self, Display, Formatter};

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub struct MockError;

    impl Display for MockError {
        fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), fmt::Error> {
            f.write_str("MockError")
        }
    }

    impl std::error::Error for MockError {}

    pub struct MockCo2Sensor {
        pub co2_ppm: VecDeque<nb::Result<u16, MockError>>,
    }

    impl Co2Sensor for MockCo2Sensor {
        type Error = MockError;

        fn read_co2_ppm(&mut self) -> nb::Result<u16, Self::Error> {
            self.co2_ppm
                .pop_front()
                .unwrap_or(Err(nb::Error::Other(MockError)))
        }
    }

    #[tokio::test]
    async fn test_co2_sensing_worker_normal_operation() {
        let (tx, mut rx) = measurement_channel(Ok(0u16));
        let co2_sensor = MockCo2Sensor {
            co2_ppm: VecDeque::from(vec![Ok(800)]),
        };

        tokio::spawn(async move { co2_sensing_worker(co2_sensor, tx).await });

        rx.trigger_measurement();
        assert_eq!(rx.changed().await.unwrap(), 800);
    }
}
