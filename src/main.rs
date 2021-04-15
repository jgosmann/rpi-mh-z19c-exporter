mod api;
mod worker;

use crate::api::metrics::serve_metrics;
use crate::api::middleware::LogErrors;
use crate::worker::measurement_channel::measurement_channel;
use crate::worker::{co2_sensing_worker, Co2MeasurementSender, Co2Sensor};
use libsystemd::daemon::{self, NotifyState};
use mh_z19c::{self, MhZ19C};
use rppal::uart::{Parity, Uart};
use std::{convert::Into, fmt::Display};
use std::{
    fmt,
    net::{SocketAddr, ToSocketAddrs},
};
use std::{path::Path, sync::Arc};
use tide::{self};
use tokio::task::{self, JoinHandle};
use worker::Co2MeasurementReceiver;

type RpiMhZ19C<'a> = MhZ19C<'a, Uart, rppal::uart::Error>;

impl Co2Sensor for RpiMhZ19C<'_> {
    type Error = Arc<mh_z19c::Error<rppal::uart::Error>>;

    fn read_co2_ppm(&mut self) -> nb::Result<u16, Self::Error> {
        self.read_co2_ppm().map_err(|e| e.map(Arc::new))
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::load()?;

    println!(
        "Starting co2-metrics-exporter v{} ...",
        env!("CARGO_PKG_VERSION")
    );
    print!("{}", config);

    let (tx, rx) = measurement_channel(Ok(0u16));
    spawn_worker(config.uart_path, tx)?;

    let app = create_app(rx);
    let server_process = app.listen(config.listen_addrs);

    println!("Ready.");
    if daemon::booted() {
        daemon::notify(true, &[NotifyState::Ready])?;
    }

    Ok(server_process.await?)
}

fn spawn_worker<P: AsRef<Path>>(
    uart_path: P,
    tx: Co2MeasurementSender<RpiMhZ19C<'static>>,
) -> Result<JoinHandle<RpiMhZ19C<'static>>, Box<dyn std::error::Error>> {
    let co2sensor = MhZ19C::new(Uart::with_path(uart_path, 9600, Parity::None, 8, 1)?);
    Ok(task::spawn(async move {
        co2_sensing_worker(co2sensor, tx).await
    }))
}

fn create_app(
    rx: Co2MeasurementReceiver<RpiMhZ19C<'static>>,
) -> tide::Server<Co2MeasurementReceiver<RpiMhZ19C<'static>>> {
    let mut app = tide::with_state(rx);
    app.with(LogErrors);
    app.at("/metrics").get(serve_metrics);
    app
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Config {
    uart_path: String,
    listen_addrs: Vec<SocketAddr>,
}

impl Config {
    fn load() -> Result<Self, Box<dyn std::error::Error>> {
        Ok(Self {
            uart_path: std::env::var("CO2_SENSOR_PATH").unwrap_or("/dev/ttyAMA0".into()),
            listen_addrs: std::env::var("CO2_METRICS_LISTEN_ADDRS")
                .unwrap_or("localhost:1202".into())
                .split(' ')
                .map(|addr| addr.to_socket_addrs())
                .collect::<std::io::Result<Vec<_>>>()?
                .into_iter()
                .flatten()
                .collect(),
        })
    }
}

impl Display for Config {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_fmt(format_args!("Using sensor at: {}\n", self.uart_path))?;
        f.write_str("Listening on addresses:")?;
        for addr in &self.listen_addrs {
            f.write_fmt(format_args!(" {}", addr))?;
        }
        f.write_str("\n")?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{create_app, RpiMhZ19C};
    use crate::worker::measurement_channel::measurement_channel;
    use crate::worker::{self, Co2MeasurementSender, Co2Sensor};
    use tide_testing::TideTestingExt;

    struct MockWorker {
        handle: tokio::task::JoinHandle<()>,
    }

    impl MockWorker {
        fn run(
            tx: Co2MeasurementSender<RpiMhZ19C<'static>>,
            value: Result<u16, worker::Error<<RpiMhZ19C as Co2Sensor>::Error>>,
        ) -> Self {
            Self {
                handle: tokio::spawn(async move {
                    loop {
                        tx.notified().await;
                        if tx.send_measurement(value.clone()).is_err() {
                            return;
                        }
                    }
                }),
            }
        }
    }

    impl Drop for MockWorker {
        fn drop(&mut self) {
            self.handle.abort();
        }
    }

    #[tokio::test]
    async fn test_serving_metrics() {
        let (tx, rx) = measurement_channel(Ok(0u16));
        let app = create_app(rx);
        let _worker = MockWorker::run(tx, Ok(42));

        assert!(app
            .get("/metrics")
            .recv_string()
            .await
            .unwrap()
            .contains("\n# TYPE co2_ppm gauge\nco2_ppm 42\n"));
    }

    #[tokio::test]
    async fn test_internal_server_error() {
        let (tx, rx) = measurement_channel(Ok(0u16));
        let app = create_app(rx);
        let _worker = MockWorker::run(tx, Err(worker::Error::TimedOut));

        assert_eq!(
            app.get("/metrics").await.unwrap().status(),
            tide::http::StatusCode::InternalServerError
        );
    }
}
