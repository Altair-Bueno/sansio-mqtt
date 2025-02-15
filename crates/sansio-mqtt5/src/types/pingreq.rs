use super::*;

#[derive(Debug, PartialEq, Clone)]

pub struct PingReq {}

#[derive(Debug, PartialEq, Clone)]
pub struct PingReqHeaderFlags;

impl From<PingReqHeaderFlags> for u8 {
    fn from(_: PingReqHeaderFlags) -> u8 {
        0
    }
}
