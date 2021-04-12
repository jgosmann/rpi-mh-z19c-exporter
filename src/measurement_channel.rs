use std::sync::Arc;
use tokio::sync::{watch, Notify};

pub fn measurement_channel<T>(initial_value: T) -> (MeasurementSender<T>, MeasurementReceiver<T>) {
    let (tx, rx) = watch::channel(initial_value);
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
pub struct MeasurementReceiver<T> {
    notifier: Arc<Notify>,
    receiver: watch::Receiver<T>,
}

impl<T> MeasurementReceiver<T> {
    pub fn trigger_measurement(&self) {
        self.notifier.notify_one()
    }

    pub async fn changed(&mut self) -> Result<watch::Ref<'_, T>, watch::error::RecvError> {
        self.receiver.changed().await?;
        Ok(self.receiver.borrow())
    }
}

#[derive(Debug)]
pub struct MeasurementSender<T> {
    notifier: Arc<Notify>,
    sender: watch::Sender<T>,
}

impl<T> MeasurementSender<T> {
    pub async fn notified(&self) {
        self.notifier.notified().await
    }

    pub fn send_measurement(&self, value: T) -> Result<(), watch::error::SendError<T>> {
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

        let (tx, mut rx) = measurement_channel(0u16);

        rx.trigger_measurement();
        rx.trigger_measurement(); // Should only trigger one measurement
        assert!(timeout(timeout_duration, tx.notified()).await.is_ok());
        assert!(timeout(timeout_duration, tx.notified()).await.is_err());

        tx.send_measurement(42).unwrap();
        assert_eq!(
            *timeout(timeout_duration, rx.changed())
                .await
                .unwrap()
                .unwrap(),
            42
        );
    }
}
