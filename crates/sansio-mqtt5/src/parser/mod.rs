mod basic;
mod properties;

pub use basic::*;

pub mod auth;
pub mod connack;
pub mod connect;
pub mod control_packet;
pub mod disconnect;
pub mod pingreq;
pub mod pingresp;
pub mod puback;
pub mod pubcomp;
pub mod publish;
pub mod pubrec;
pub mod pubrel;
pub mod reserved;
pub mod suback;
pub mod subscribe;
pub mod unsuback;
pub mod unsubscribe;

use super::types::*;
use core::any::type_name;
use core::str;
use core::str::Utf8Error;
use winnow::binary;
use winnow::binary::bits;
use winnow::combinator;
use winnow::error::*;
use winnow::prelude::*;
use winnow::stream::*;
use winnow::token;

// TODO: improve error reporting and context

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Settings {
    pub max_bytes_string: u16,
    pub max_bytes_binary_data: u16,
    pub max_remaining_bytes: u64,
    pub max_properties_len: u32,
    pub max_subscriptions_len: u32,
}

impl Settings {
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    #[inline]
    pub fn unlimited() -> Self {
        Self {
            max_bytes_string: u16::MAX,
            max_bytes_binary_data: u16::MAX,
            max_remaining_bytes: u64::MAX,
            max_properties_len: u32::MAX,
            max_subscriptions_len: u32::MAX,
        }
    }
}

impl Default for Settings {
    #[inline]
    fn default() -> Self {
        Self {
            max_bytes_string: 5 * 1024,       // 5 KiB
            max_bytes_binary_data: 5 * 1024,  // 5 KiB
            max_remaining_bytes: 1024 * 1024, // 1 MiB
            max_properties_len: 32,           // 32 properties
            max_subscriptions_len: 32,        // 32 subscriptions
        }
    }
}
