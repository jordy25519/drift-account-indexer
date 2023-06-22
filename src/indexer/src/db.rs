//! Database schema and connection management
use std::sync::{Mutex, MutexGuard};

use async_trait::async_trait;
use log::debug;
use mongodb::{
    bson::{doc, Bson},
    options::FindOneAndUpdateOptions,
    Client, Database,
};
use serde::{Deserialize, Serialize};
use solana_sdk::{pubkey::Pubkey, signature::Signature};

use crate::types::{OrderActionRecord, OrderRecord};

const DB_DATABASE_NAME: &str = "drift";

#[derive(Debug)]
pub enum DbError {
    /// Failure during insert
    Insert(String),
    /// Failure during read
    Read(String),
}

/// Indexer backend API
#[async_trait]
pub trait IndexerBackend: Send + Sync {
    /// Instantiate the db backend
    async fn init(conn_str: &str) -> Self;
    /// Return the last indexed tx signature for `account`
    async fn last_indexed_signature(&self, account: &Pubkey) -> Result<Option<Signature>, DbError>;
    /// Update the last processed `signature` for `account`
    async fn update_last_indexed_signature(
        &self,
        account: &Pubkey,
        signature: &Signature,
    ) -> Result<(), DbError>;
    /// Insert an `OrderActionRecord` into the db
    async fn insert_order_action_record(&self, record: OrderActionRecord) -> Result<(), DbError>;
    /// Insert an `OrderRecord` into the db
    async fn insert_order_record(&self, record: OrderRecord) -> Result<(), DbError>;
}

/// MongoDb indexer database client
pub struct MongoDbClient {
    _inner: Client,
    db: Database,
}

impl MongoDbClient {
    pub async fn new(conn_str: &str) -> Self {
        let client = Client::with_uri_str(conn_str).await.expect("db connect");
        let db = client.database(DB_DATABASE_NAME);
        Self { db, _inner: client }
    }
}

#[async_trait]
impl IndexerBackend for MongoDbClient {
    async fn init(conn_str: &str) -> Self {
        MongoDbClient::new(conn_str).await
    }
    async fn last_indexed_signature(&self, account: &Pubkey) -> Result<Option<Signature>, DbError> {
        let address_bytes = Bson::Array(
            account
                .to_bytes()
                .iter()
                .map(|d| Bson::Int32(*d as i32))
                .collect(),
        );
        let query = doc! { "address": address_bytes };
        let res = self
            .db
            .collection::<Account>("accounts")
            .find_one(query, None)
            .await
            .map_err(|err| DbError::Read(err.kind.to_string()))?;

        Ok(res.map(|u| u.last_processed_signature))
    }
    async fn update_last_indexed_signature(
        &self,
        account: &Pubkey,
        signature: &Signature,
    ) -> Result<(), DbError> {
        // TODO: consider timestamp of tx, this may re-process a signature needlessly
        debug!(
            "set last processed signature: {:?} as {:?}",
            account, signature
        );
        let address_bytes = Bson::Array(
            account
                .to_bytes()
                .iter()
                .map(|d| Bson::Int32(*d as i32))
                .collect(),
        );
        let signature_bytes = Bson::Array(
            signature
                .as_ref()
                .iter()
                .map(|d| Bson::Int32(*d as i32))
                .collect(),
        );

        self.db
            .collection::<Account>("accounts")
            .find_one_and_update(
                doc! { "address": address_bytes },
                doc! { "$set": { "last_processed_signature": signature_bytes } },
                FindOneAndUpdateOptions::builder().upsert(true).build(),
            )
            .await
            .map_err(|err| DbError::Insert(err.kind.to_string()))
            .map(|_res| ())
    }
    async fn insert_order_action_record(&self, record: OrderActionRecord) -> Result<(), DbError> {
        self.db
            .collection("order_action_records")
            .insert_one(record, None)
            .await
            .map_err(|err| DbError::Insert(err.kind.to_string()))
            .map(|_res| ())
    }
    async fn insert_order_record(&self, record: OrderRecord) -> Result<(), DbError> {
        self.db
            .collection("order_records")
            .insert_one(record, None)
            .await
            .map_err(|err| DbError::Insert(err.kind.to_string()))
            .map(|_res| ())
    }
}

/// Test backend
pub struct MockBackend {
    order_action_records: Mutex<Vec<OrderActionRecord>>,
    order_records: Mutex<Vec<OrderRecord>>,
    last_signature: Mutex<Option<Signature>>,
}

impl MockBackend {
    pub fn order_records(&self) -> MutexGuard<Vec<OrderRecord>> {
        self.order_records.lock().unwrap()
    }
    pub fn order_action_records(&self) -> MutexGuard<Vec<OrderActionRecord>> {
        self.order_action_records.lock().unwrap()
    }
}

#[async_trait]
impl IndexerBackend for MockBackend {
    async fn init(_conn_str: &str) -> Self {
        Self {
            order_action_records: Default::default(),
            order_records: Default::default(),
            last_signature: Default::default(),
        }
    }
    async fn last_indexed_signature(
        &self,
        _account: &Pubkey,
    ) -> Result<Option<Signature>, DbError> {
        Ok(*self.last_signature.lock().unwrap())
    }
    async fn insert_order_action_record(&self, record: OrderActionRecord) -> Result<(), DbError> {
        let mut records = self.order_action_records.lock().unwrap();
        records.push(record);
        Ok(())
    }
    async fn insert_order_record(&self, record: OrderRecord) -> Result<(), DbError> {
        let mut records = self.order_records.lock().unwrap();
        records.push(record);
        Ok(())
    }
    async fn update_last_indexed_signature(
        &self,
        _account: &Pubkey,
        signature: &Signature,
    ) -> Result<(), DbError> {
        let mut last_signature = self.last_signature.lock().unwrap();
        *last_signature = Some(*signature);
        Ok(())
    }
}

#[derive(Serialize, Deserialize)]
struct Account {
    address: Pubkey,
    last_processed_signature: Signature,
}
