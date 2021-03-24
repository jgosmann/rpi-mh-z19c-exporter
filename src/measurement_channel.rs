use std::sync::Arc;
use tokio::sync::{watch, Notify};

pub fn measurement_channel() -> (MeasurementSender, MeasurementReceiver) {
    let (tx, rx) = watch::channel(0u16);
    let notifier = Arc::new(Notify::new());
    let sender = MeasurementSender {
        notifier: notifier.clone(),
        sender: tx,
    };
    let receiver = MeasurementReceiver {
        notifier,
        receiver: rx,
    };
    (sender, receiver)
}

#[derive(Clone, Debug)]
pub struct MeasurementReceiver {
    notifier: Arc<Notify>,
    receiver: watch::Receiver<u16>,
}

impl MeasurementReceiver {
    pub fn trigger_measurement(&self) {
        self.notifier.notify_one()
    }

    pub async fn changed(&mut self) -> Result<u16, watch::error::RecvError> {
        self.receiver.changed().await?;
        Ok(*self.receiver.borrow())
    }
}

#[derive(Debug)]
pub struct MeasurementSender {
    notifier: Arc<Notify>,
    sender: watch::Sender<u16>,
}

impl MeasurementSender {
    pub async fn notified(&self) {
        self.notifier.notified().await
    }

    pub fn send_measurement(&self, value: u16) -> Result<(), watch::error::SendError<u16>> {
        self.sender.send(value)
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use tokio::time::{timeout, Duration};

    #[tokio::test]
    async fn test_measurement_channel() {
        let timeout_duration = Duration::from_millis(10);

        let (tx, mut rx) = measurement_channel();

        rx.trigger_measurement();
        rx.trigger_measurement(); // Should only trigger one measurement
        assert!(timeout(timeout_duration, tx.notified()).await.is_ok());
        assert!(timeout(timeout_duration, tx.notified()).await.is_err());

        tx.send_measurement(42).unwrap();
        assert_eq!(
            timeout(timeout_duration, rx.changed())
                .await
                .unwrap()
                .unwrap(),
            42
        );
    }
}
