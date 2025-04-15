use super::*;

#[derive(Debug, PartialEq, Clone)]

pub struct Reserved {}

#[derive(Debug, PartialEq, Clone)]

pub struct ReservedHeaderFlags;

impl From<ReservedHeaderFlags> for u8 {
    fn from(_: ReservedHeaderFlags) -> u8 {
        0
    }
}
