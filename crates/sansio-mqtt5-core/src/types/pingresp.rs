use super::*;

#[derive(Debug, PartialEq, Clone)]

pub struct PingResp {}
#[derive(Debug, PartialEq, Clone)]

pub struct PingRespHeaderFlags;

impl From<PingRespHeaderFlags> for u8 {
    fn from(_: PingRespHeaderFlags) -> u8 {
        0
    }
}
