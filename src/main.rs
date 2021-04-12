use co2_metrics_exporter::measurement_channel::{
    measurement_channel, MeasurementReceiver, MeasurementSender,
};
use co2_metrics_exporter::middleware::LogErrors;
use libsystemd::daemon::{self, NotifyState};
use mh_z19c::{self, MhZ19C};
use prometheus::proto::{Gauge, Metric, MetricFamily, MetricType};
use prometheus::{self, Encoder};
use protobuf;
use rppal::uart::{Parity, Uart};
use std::net::{SocketAddr, ToSocketAddrs};
use std::sync::Arc;
use std::{
    convert::{From, Into},
    time::Duration,
};
use std::{
    fmt::{self, Debug, Display, Formatter},
    time::Instant,
};
use tide::{self};
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
    sender: MeasurementSender<Result<u16, Co2SensingWorkerError<C::Error>>>,
) -> C {
    loop {
        sender.notified().await;
        let value = block_with_timeout(Duration::from_millis(200), || co2_sensor.read_co2_ppm())
            .map_err(|err| match err {
                nb::Error::WouldBlock => Co2SensingWorkerError::TimedOut,
                nb::Error::Other(err) => Co2SensingWorkerError::SensorError(err),
            });
        if sender.send_measurement(value).is_err() {
            return co2_sensor;
        }
    }
}

#[derive(Clone, Debug)]
enum Co2SensingWorkerError<C> {
    TimedOut,
    SensorError(C),
}

impl<C: Display> Display for Co2SensingWorkerError<C> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        use Co2SensingWorkerError::*;
        match self {
            TimedOut => f.write_str("communication timeout"),
            SensorError(err) => err.fmt(f),
        }
    }
}

impl<C: Debug + Display> std::error::Error for Co2SensingWorkerError<C> {}

type MhZ19CWorkerError = Co2SensingWorkerError<Arc<mh_z19c::Error<rppal::uart::Error>>>;

fn block_with_timeout<F, T, E>(timeout: Duration, mut func: F) -> nb::Result<T, E>
where
    F: FnMut() -> nb::Result<T, E>,
{
    let abort_at = Instant::now() + timeout;
    while Instant::now() < abort_at {
        match func() {
            Err(nb::Error::WouldBlock) => (),
            result => return result,
        }
    }
    Err(nb::Error::WouldBlock)
}

async fn serve_metrics(
    req: tide::Request<MeasurementReceiver<Result<u16, MhZ19CWorkerError>>>,
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
    SensorError(MhZ19CWorkerError),
    WorkerDied,
}

impl From<MhZ19CWorkerError> for ServeMetricsInternalError {
    fn from(err: MhZ19CWorkerError) -> Self {
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
            SensorError(err) => Display::fmt(err, f),
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
    let uart_path = std::env::var("CO2_SENSOR_PATH").unwrap_or("/dev/ttyAMA0".into());
    let listen_addrs: Vec<SocketAddr> = std::env::var("CO2_METRICS_LISTEN_ADDRS")
        .unwrap_or("localhost:9119".into())
        .split(' ')
        .map(|addr| addr.to_socket_addrs())
        .collect::<std::io::Result<Vec<_>>>()?
        .into_iter()
        .flatten()
        .collect();

    println!("Starting co2-metrics-exporter ...");
    println!("Using sensor at: {}", uart_path);
    print!("Listening on addresses:");
    for addr in &listen_addrs {
        print!(" {}", addr)
    }
    println!("");

    let (tx, rx) = measurement_channel(Ok(0u16));

    let co2sensor = MhZ19C::new(Uart::with_path(uart_path, 9600, Parity::None, 8, 1)?);
    task::spawn(async move { co2_sensing_worker(co2sensor, tx).await });

    let mut app = tide::with_state(rx);
    app.with(LogErrors);
    app.at("/metrics").get(serve_metrics);
    let app_process = app.listen(listen_addrs);

    println!("Ready.");
    if daemon::booted() {
        daemon::notify(true, &[NotifyState::Ready])?;
    }

    Ok(app_process.await?)
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
        assert_eq!(rx.changed().await.unwrap().clone().unwrap(), 800);
    }
}
