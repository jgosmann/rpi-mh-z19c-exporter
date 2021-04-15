pub mod measurement_channel;

use std::{
    fmt::{self, Debug, Display, Formatter},
    time::{Duration, Instant},
};

use measurement_channel::{MeasurementReceiver, MeasurementSender};

pub type Co2MeasurementReceiver<C> =
    MeasurementReceiver<Result<u16, Error<<C as Co2Sensor>::Error>>>;
pub type Co2MeasurementSender<C> = MeasurementSender<Result<u16, Error<<C as Co2Sensor>::Error>>>;

pub trait Co2Sensor {
    type Error;

    fn read_co2_ppm(&mut self) -> nb::Result<u16, Self::Error>;
}

pub async fn co2_sensing_worker<C: Co2Sensor>(
    mut co2_sensor: C,
    sender: MeasurementSender<Result<u16, Error<C::Error>>>,
) -> C {
    loop {
        sender.notified().await;
        let value = block_with_timeout(Duration::from_millis(200), || co2_sensor.read_co2_ppm())
            .map_err(|err| match err {
                nb::Error::WouldBlock => Error::TimedOut,
                nb::Error::Other(err) => Error::SensorError(err),
            });
        if sender.send_measurement(value).is_err() {
            return co2_sensor;
        }
    }
}

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

#[derive(Clone, Debug)]
pub enum Error<C> {
    TimedOut,
    SensorError(C),
}

impl<C: Display> Display for Error<C> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        use Error::*;
        match self {
            TimedOut => f.write_str("communication timeout"),
            SensorError(err) => err.fmt(f),
        }
    }
}

impl<C: Debug + Display> std::error::Error for Error<C> {}

#[cfg(test)]
pub mod tests {
    use super::*;
    use measurement_channel::measurement_channel;
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
