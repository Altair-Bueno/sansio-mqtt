#![feature(coroutines, coroutine_trait)]

use sansio_mqtt_v5_types::*;

pub struct Client {
    settings: Settings,
}
