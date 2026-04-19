mod basic;
mod properties;

pub use basic::*;

mod auth;
mod connack;
mod connect;
mod control_packet;
mod disconnect;
mod pingreq;
mod pingresp;
mod puback;
mod pubcomp;
mod publish;
mod pubrec;
mod pubrel;
mod reserved;
mod suback;
mod subscribe;
mod unsuback;
mod unsubscribe;

use super::*;
use alloc::vec::Vec;
use core::any::type_name;
use core::num::TryFromIntError;
use core::str::Utf8Error;
use winnow::binary;
use winnow::binary::bits;
use winnow::combinator;
use winnow::error::*;
use winnow::prelude::*;
use winnow::stream::*;
use winnow::token;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParserSettings {
    pub max_bytes_string: u16,
    pub max_bytes_binary_data: u16,
    pub max_remaining_bytes: u64,
    pub max_subscriptions_len: u32,
    pub max_user_properties_len: usize,
    pub max_subscription_identifiers_len: usize,
}

impl ParserSettings {
    #[inline]
    pub const fn new() -> Self {
        Self {
            max_bytes_string: 5 * 1024,              // 5 KiB
            max_bytes_binary_data: 5 * 1024,         // 5 KiB
            max_remaining_bytes: 1024 * 1024,        // 1 MiB
            max_subscriptions_len: 32,               // 32 subscriptions
            max_user_properties_len: 32,             // 32 properties
            max_subscription_identifiers_len: 32,    // 32 identifiers
        }
    }

    #[inline]
    pub const fn unlimited() -> Self {
        Self {
            max_bytes_string: u16::MAX,
            max_bytes_binary_data: u16::MAX,
            max_remaining_bytes: u64::MAX,
            max_subscriptions_len: u32::MAX,
            max_user_properties_len: usize::MAX,
            max_subscription_identifiers_len: usize::MAX,
        }
    }
}

impl Default for ParserSettings {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}
