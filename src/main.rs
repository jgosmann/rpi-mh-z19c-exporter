use co2_metrics_exporter::measurement_channel::{
    measurement_channel, MeasurementReceiver, MeasurementSender,
};
use mh_z19c::{self, MhZ19C};
use prometheus::{self, Encoder};
use protobuf;
use rppal::uart::{Parity, Uart};
use tide;
use tokio::task;

trait Co2Sensor {
    type Error;

    fn read_co2_ppm(&mut self) -> nb::Result<u16, Self::Error>;
}

impl Co2Sensor for MhZ19C<'_, Uart, rppal::uart::Error> {
    type Error = mh_z19c::Error<rppal::uart::Error>;

    fn read_co2_ppm(&mut self) -> nb::Result<u16, Self::Error> {
        self.read_co2_ppm()
    }
}

async fn co2_sensing_worker<C: Co2Sensor>(
    mut co2_sensor: C,
    sender: MeasurementSender,
) -> Result<(), C::Error> {
    loop {
        sender.notified().await;
        loop {
            match co2_sensor.read_co2_ppm() {
                Ok(value) => {
                    match sender.send_measurement(value) {
                        Ok(_) => break,
                        Err(_) => (), // FIXME is this desired?
                    }
                }
                Err(nb::Error::Other(err)) => return Err(err), // FIXME log and continue?
                Err(nb::Error::WouldBlock) => (),
            }
        }
    }
}

async fn serve_metrics(req: tide::Request<MeasurementReceiver>) -> tide::Result {
    req.state().trigger_measurement();
    match req.state().clone().changed().await {
        Ok(value) => {
            let mut metric_family = prometheus::proto::MetricFamily::new();
            metric_family.set_name("co2_ppm".into());
            metric_family.set_help("CO2 concnentration [ppm]".into());
            metric_family.set_field_type(prometheus::proto::MetricType::GAUGE);
            let mut gauge = prometheus::proto::Gauge::new();
            gauge.set_value(value as f64);
            let mut metric = prometheus::proto::Metric::new();
            metric.set_gauge(gauge);
            metric_family.set_metric(protobuf::RepeatedField::from_slice(&[metric]));

            let mut buffer = vec![];
            let encoder = prometheus::TextEncoder::new();
            encoder.encode(&[metric_family], &mut buffer)?;
            Ok(format!("{}", String::from_utf8(buffer)?).into())
        }
        Err(err) => Err(tide::Error::from_str(
            tide::StatusCode::InternalServerError,
            err,
        )),
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (tx, rx) = measurement_channel();

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
        let (tx, mut rx) = measurement_channel();
        let co2_sensor = MockCo2Sensor {
            co2_ppm: VecDeque::from(vec![Ok(800)]),
        };

        tokio::spawn(async move { co2_sensing_worker(co2_sensor, tx).await });

        rx.trigger_measurement();
        assert_eq!(rx.changed().await.unwrap(), 800);
    }
}
