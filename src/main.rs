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

    let server_process = run_server(config.listen_addrs, rx);

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

async fn run_server(
    listen_addrs: Vec<SocketAddr>,
    rx: Co2MeasurementReceiver<RpiMhZ19C<'static>>,
) -> std::io::Result<()> {
    let mut app = tide::with_state(rx);
    app.with(LogErrors);
    app.at("/metrics").get(serve_metrics);
    app.listen(listen_addrs).await
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
