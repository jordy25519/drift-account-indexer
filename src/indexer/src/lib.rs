//! Drift account indexer
//!
//! Provides a service to poll an account's events on the drift program and persist into storage
use std::{str::FromStr, sync::Arc, time::Duration};

use futures::{stream::FuturesUnordered, StreamExt};
use log::{debug, info, warn};
pub use solana_rpc_client::nonblocking::rpc_client::RpcClient;
use solana_rpc_client::rpc_client::GetConfirmedSignaturesForAddress2Config;
use solana_rpc_client_api::{
    config::RpcTransactionConfig, response::RpcConfirmedTransactionStatusWithSignature,
};
use solana_sdk::{pubkey::Pubkey, signature::Signature, transaction::VersionedTransaction};
use solana_transaction_status::{option_serializer::OptionSerializer, UiTransactionEncoding};
use tokio::select;

mod db;
pub use db::{IndexerBackend, MockBackend, MongoDbClient};
mod types;
pub use types::IndexerError;
use types::*;

/// Number of account txs to request per period
/// should be balanced with polling interval to respect rate limits
const MAX_TXS_PER_PERIOD: usize = 3;

/// Provides indexing for onchain drift events
pub struct DriftEventIndexer<T: IndexerBackend> {
    /// Db client
    db: Arc<T>,
    /// Solana RPC client
    rpc: Arc<RpcClient>,
}

impl<T: IndexerBackend> DriftEventIndexer<T> {
    /// Create a new `DriftEventIndexer`
    pub fn new(db: Arc<T>, rpc: Arc<RpcClient>) -> Self {
        Self { db, rpc }
    }

    /// Run the indexer for `account`
    /// - `poll_interval` frequency to pool chain for events
    pub async fn run(self, account: &str, poll_interval: Duration) -> Result<(), IndexerError> {
        let account = &Pubkey::try_from(account).map_err(|_| IndexerError::InvalidPublicKey)?;
        let mut poll = tokio::time::interval(poll_interval);
        loop {
            select! {
                _ = poll.tick() => self.index_account_events(account).await?
            }
        }
    }

    /// Index the events for `account`
    async fn index_account_events(&self, account: &Pubkey) -> Result<(), IndexerError> {
        // TODO: can use some cached value to avoid db query
        let last_signature = self.db.last_indexed_signature(account).await?;

        let results = self
            .rpc
            .get_signatures_for_address_with_config(
                account,
                GetConfirmedSignaturesForAddress2Config {
                    limit: Some(MAX_TXS_PER_PERIOD),
                    until: last_signature,
                    ..Default::default()
                },
            )
            .await?;
        debug!("latest signatures: {:?}", results);
        let mut index_tx_futs = FuturesUnordered::from_iter(results.iter().map(
            |RpcConfirmedTransactionStatusWithSignature { signature, .. }| {
                self.index_transaction(account, signature.as_str())
            },
        ));

        while let Some(res) = index_tx_futs.next().await {
            res?;
        }

        Ok(())
    }
    /// Index events of the given transaction `signature`, provided the tx interacts with the drift program
    async fn index_transaction(
        &self,
        account: &Pubkey,
        tx_signature: &str,
    ) -> Result<(), IndexerError> {
        let tx_data = self
            .rpc
            .get_transaction_with_config(
                &Signature::from_str(tx_signature).map_err(|_| IndexerError::InvalidSignature)?,
                RpcTransactionConfig {
                    encoding: Some(UiTransactionEncoding::Base64),
                    max_supported_transaction_version: Some(0),
                    commitment: None, // finalized by default
                },
            )
            .await?;

        // only interested in txs interacting with the drift program
        match tx_data.transaction.transaction.decode() {
            Some(VersionedTransaction { message, .. }) => {
                if !message
                    .static_account_keys()
                    .iter()
                    .any(|k| k == &drift_pda())
                {
                    return Ok(());
                }
            }
            None => {
                warn!(
                    "failed deserializing tx: {:?}",
                    tx_data.transaction.transaction
                );
                return Ok(());
            }
        }
        debug!("drift tx: {:?}", &tx_data.transaction);
        if let Some(ref meta) = tx_data.transaction.meta {
            if let OptionSerializer::Some(ref logs) = meta.log_messages {
                for log in logs {
                    // TODO: this is a quick hack, map to strut using discriminant
                    if let Ok(Some(record)) = handle_log::<OrderActionRecord>(log.as_str()) {
                        info!(
                            "indexing OrderActionRecord maker={:?}, taker={:?}",
                            record.maker, record.taker
                        );
                        self.db.insert_order_action_record(record).await?;
                    }
                    if let Ok(Some(record)) = handle_log::<OrderRecord>(log.as_str()) {
                        info!("indexing OrderRecord: {:?}", record.user);
                        self.db.insert_order_record(record).await?;
                    }
                }
            }
        }

        self.db
            .update_last_indexed_signature(account, &Signature::from_str(tx_signature).unwrap())
            .await?;

        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use std::{collections::HashMap, str::FromStr};

    use solana_rpc_client_api::{
        request::RpcRequest, response::RpcConfirmedTransactionStatusWithSignature,
    };
    use solana_sdk::message::{
        v0::{self},
        MessageHeader, VersionedMessage,
    };
    use solana_transaction_status::{
        ConfirmedTransactionWithStatusMeta, TransactionStatusMeta, TransactionWithStatusMeta,
        VersionedTransactionWithStatusMeta,
    };

    #[tokio::test]
    async fn index_account() {
        let get_signature_for_address_response: Vec<RpcConfirmedTransactionStatusWithSignature> = vec![
            RpcConfirmedTransactionStatusWithSignature {
                signature: "3gvGQufckXGHrFDv4dNWEXuXKRMy3NZkKHMyFrAhLoYScaXXTGCp9vq58kWkfyJ8oDYZrz4bTyGayjUy9PKigeLS".to_string(),
                slot: 196923928_u64,
                err: None,
                memo: None,
                block_time: None,
                confirmation_status: None,
            }
        ];
        let get_transaction_response = ConfirmedTransactionWithStatusMeta {
            slot: 196923928_u64,
            tx_with_meta: TransactionWithStatusMeta::Complete(VersionedTransactionWithStatusMeta {
                transaction: VersionedTransaction {
                    message: VersionedMessage::V0(v0::Message {
                        header: MessageHeader {
                            num_required_signatures: 1, // pass sanitization
                            ..Default::default()
                        },
                        account_keys: vec![drift_pda()],
                        ..Default::default()
                    }),
                    signatures: vec![Signature::new_unique()],
                },
                meta: TransactionStatusMeta {
                    log_messages: Some(vec![
                        "Program ComputeBudget111111111111111111111111111111 invoke [1]".to_string(),
                        "Program ComputeBudget111111111111111111111111111111 success".to_string(),
                        "Program ComputeBudget111111111111111111111111111111 invoke [1]".to_string(),
                        "Program ComputeBudget111111111111111111111111111111 success".to_string(),
                        "Program dRiftyHA39MWEi3m9aunc5MzRF1JYuBsbn6VPcn33UH invoke [1]".to_string(),
                        "Program log: Instruction: FillPerpOrder".to_string(),
                        "Program log: 4DRDR8LtbQGWwHZkAAAAAAIIAQABAVAItYsox9wC2v+AAz8WXQRRjyHZ0aSDao8VZMh+F12zAd0EAAAAAAAAAYLxCAAAAAAAAWDjFgAAAAAAAbKkeQIAAAAAAaowAAAAAAAAAY/f////////AAAAAe3FfpKhZkk9E4ZlwFSFEmXchAsvmwHVTjGQOBC+69TDAQ8hIQABAAGAhB4AAAAAAAGAhB4AAAAAAAGq2EwDAAAAAAE10NxKUa97dfc1auP2TjQAqOAgggM7dWBcCJ9gI3Fn5AGbdFQAAQEBoNcmAgAAAAABYOMWAAAAAAABsqR5AgAAAABAiupxBgAAAA==".to_string(),
                        "Program dRiftyHA39MWEi3m9aunc5MzRF1JYuBsbn6VPcn33UH consumed 306396 of 400000 compute units".to_string(),
                        "Program dRiftyHA39MWEi3m9aunc5MzRF1JYuBsbn6VPcn33UH success.to_string()".to_string(),
                    ]),
                    ..Default::default()
                },
            }),
            block_time: None,
        }
        .encode(UiTransactionEncoding::Base64, Some(0))
        .expect("it encodes");

        let mock_responses = HashMap::from([
            (
                RpcRequest::GetSignaturesForAddress,
                serde_json::to_value(get_signature_for_address_response).expect("it serializes"),
            ),
            (
                RpcRequest::GetTransaction,
                serde_json::to_value(get_transaction_response).expect("it serializes"),
            ),
        ]);
        let mock_rpc =
            RpcClient::new_mock_with_mocks("http://example.com".to_string(), mock_responses);

        let indexer = DriftEventIndexer::new(
            Arc::new(MockBackend::init("mockdb").await),
            Arc::new(mock_rpc),
        );
        let account =
            Pubkey::from_str("BTDXiRzG1QBP7bfK4A33RcSP5mmZx8mGJ9YC5maetoD6").expect("valid pubkey");

        // Test
        let res = indexer.index_account_events(&account).await;

        assert!(res.is_ok());
        assert_eq!(
            indexer.db.last_indexed_signature(&account).await.unwrap(),
            Some(Signature::from_str("3gvGQufckXGHrFDv4dNWEXuXKRMy3NZkKHMyFrAhLoYScaXXTGCp9vq58kWkfyJ8oDYZrz4bTyGayjUy9PKigeLS").unwrap())
        );
        assert_eq!(
            indexer.db.order_action_records().as_slice(),
            [OrderActionRecord {
                ts: 1685504150,
                action: OrderAction::Fill,
                actionExplanation: OrderActionExplanation::OrderFilledWithMatch,
                marketIndex: 1,
                marketType: MarketType::Perp,
                filler: Some(
                    Pubkey::from_str("6PRKTZiooHi2qdBb5raxJnVvjfBhrfcDWKvfbWt2oR5C").unwrap()
                ),
                fillerReward: Some(1245),
                fillRecordId: Some(586114),
                baseAssetAmountFilled: Some(1500000),
                quoteAssetAmountFilled: Some(41526450),
                takerFee: Some(12458),
                makerFee: Some(-8305),
                referrerReward: None,
                quoteAssetAmountSurplus: None,
                spotFulfillmentMethodFee: None,
                taker: Some(
                    Pubkey::from_str("H1AHngDKHCSZe4Xsw7Yk4SV5RP9agaaDhQmwTjRzhXFG").unwrap()
                ),
                takerOrderId: Some(2171151),
                takerOrderDirection: Some(PositionDirection::Long),
                takerOrderBaseAssetAmount: Some(2000000),
                takerOrderCumulativeBaseAssetAmountFilled: Some(2000000),
                takerOrderCumulativeQuoteAssetAmountFilled: Some(55367850),
                maker: Some(
                    Pubkey::from_str("4d5KsDvVn25So6EqM6KhgJyyUbG11SaBjzDRL1FqzmRV").unwrap()
                ),
                makerOrderId: Some(5534875),
                makerOrderDirection: Some(PositionDirection::Short),
                makerOrderBaseAssetAmount: Some(36100000),
                makerOrderCumulativeBaseAssetAmountFilled: Some(1500000),
                makerOrderCumulativeQuoteAssetAmountFilled: Some(41526450),
                oraclePrice: 27681000000
            }]
        );
    }
}
