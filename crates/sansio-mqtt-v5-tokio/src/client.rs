use sansio_mqtt_v5_protocol::ClientMessage;
use sansio_mqtt_v5_protocol::SubscribeOptions;
use sansio_mqtt_v5_protocol::UnsubscribeOptions;
use sansio_mqtt_v5_protocol::UserWriteIn;
use tokio::sync::mpsc;

use crate::ClientError;

#[derive(Clone, Debug)]
pub struct Client {
    pub(crate) tx: mpsc::Sender<UserWriteIn>,
}

impl Client {
    pub(crate) fn new(tx: mpsc::Sender<UserWriteIn>) -> Self {
        Self { tx }
    }

    #[doc(hidden)]
    pub fn new_for_test(tx: mpsc::Sender<UserWriteIn>) -> Self {
        Self { tx }
    }

    pub async fn publish(&self, message: ClientMessage) -> Result<(), ClientError> {
        self.tx
            .send(UserWriteIn::PublishMessage(message))
            .await
            .map_err(|_| ClientError::Closed)
    }

    pub async fn subscribe(&self, options: SubscribeOptions) -> Result<(), ClientError> {
        self.tx
            .send(UserWriteIn::Subscribe(options))
            .await
            .map_err(|_| ClientError::Closed)
    }

    pub async fn unsubscribe(&self, options: UnsubscribeOptions) -> Result<(), ClientError> {
        self.tx
            .send(UserWriteIn::Unsubscribe(options))
            .await
            .map_err(|_| ClientError::Closed)
    }

    pub async fn disconnect(&self) -> Result<(), ClientError> {
        self.tx
            .send(UserWriteIn::Disconnect)
            .await
            .map_err(|_| ClientError::Closed)
    }
}
