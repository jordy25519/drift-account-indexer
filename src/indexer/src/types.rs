//! Indexer types
use std::{cell::OnceCell, str::FromStr};

use anchor_attribute_event::event;
use anchor_lang::{prelude::*, Discriminator};
use serde::{Deserialize, Serialize};
use solana_rpc_client_api::client_error::Error;
use solana_sdk::pubkey::Pubkey;

use idl_gen::gen_idl_types;

use crate::db::DbError;

const DRIFT_PDA: &str = "dRiftyHA39MWEi3m9aunc5MzRF1JYuBsbn6VPcn33UH";
const DRIFT_PK: OnceCell<Pubkey> = OnceCell::new();
const PROGRAM_LOG: &str = "Program log: ";
const PROGRAM_DATA: &str = "Program data: ";

/// Get the drift PDA
#[inline]
pub fn drift_pda() -> Pubkey {
    *DRIFT_PK.get_or_init(|| Pubkey::from_str(DRIFT_PDA).unwrap())
}

// TODO: the onchain IDL may change, need to regen if so
gen_idl_types!("../../res/drift-2.30.0-beta.1.json");

#[derive(Debug)]
pub enum IndexerError {
    Rpc(Error),
    Db(DbError),
    InvalidSignature,
    InvalidPublicKey,
    LogParse(LogError),
}

#[derive(Debug, PartialEq)]
pub enum LogError {
    /// log data was not valid base64
    InvalidBase64,
}

impl From<LogError> for IndexerError {
    fn from(err: LogError) -> Self {
        Self::LogParse(err)
    }
}

impl From<DbError> for IndexerError {
    fn from(err: DbError) -> Self {
        Self::Db(err)
    }
}

impl From<Error> for IndexerError {
    fn from(err: Error) -> Self {
        Self::Rpc(err)
    }
}

/// Try deserialize a drift event type from raw log string
/// https://github.com/coral-xyz/anchor/blob/9d947cb26b693e85e1fd26072bb046ff8f95bdcf/client/src/lib.rs#L552
pub(crate) fn try_parse_log(raw: &str) -> std::result::Result<Option<DriftEvent>, LogError> {
    // Log emitted from the current program.
    if let Some(log) = raw
        .strip_prefix(PROGRAM_LOG)
        .or_else(|| raw.strip_prefix(PROGRAM_DATA))
    {
        let borsh_bytes =
            anchor_lang::__private::base64::decode(log).map_err(|_| LogError::InvalidBase64)?;
        let (sig, mut data) = borsh_bytes.split_at(8);
        Ok(DriftEvent::from_discriminant(
            sig.try_into().unwrap(),
            &mut data,
        ))
    } else {
        Ok(None)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn deserialize_order_action_record() {
        // https://explorer.solana.com/tx/3gvGQufckXGHrFDv4dNWEXuXKRMy3NZkKHMyFrAhLoYScaXXTGCp9vq58kWkfyJ8oDYZrz4bTyGayjUy9PKigeLS#ix-3
        let raw = "Program log: 4DRDR8LtbQGWwHZkAAAAAAIIAQABAVAItYsox9wC2v+AAz8WXQRRjyHZ0aSDao8VZMh+F12zAd0EAAAAAAAAAYLxCAAAAAAAAWDjFgAAAAAAAbKkeQIAAAAAAaowAAAAAAAAAY/f////////AAAAAe3FfpKhZkk9E4ZlwFSFEmXchAsvmwHVTjGQOBC+69TDAQ8hIQABAAGAhB4AAAAAAAGAhB4AAAAAAAGq2EwDAAAAAAE10NxKUa97dfc1auP2TjQAqOAgggM7dWBcCJ9gI3Fn5AGbdFQAAQEBoNcmAgAAAAABYOMWAAAAAAABsqR5AgAAAABAiupxBgAAAA==";
        let res = try_parse_log(raw).expect("it deserializes");
        dbg!(&res);
        match res {
            Some(DriftEvent::OrderActionRecord(_)) => assert!(true),
            _ => assert!(false),
        }
    }

    #[test]
    fn deserialize_order_action_record_fails() {
        let raw = "Program ComputeBudget111111111111111111111111111111 invoke [1]";
        let res = try_parse_log(raw).expect("it deserializes");
        assert!(res.is_none());

        let raw = "Program log: Instruction: FillPerpOrder";
        let res = try_parse_log(raw);
        assert!(res.is_err());
    }
}
