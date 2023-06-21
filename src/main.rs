//! Drift indexer entrypoint
use std::{
    env::{self},
    sync::Arc,
    time::Duration,
};

use clap::Parser;
use futures_util::future::select_all;
use log::info;
use tokio::task::JoinHandle;

use drift_indexer_backend::{
    DriftEventIndexer, IndexerBackend, IndexerError, MongoDbClient, RpcClient,
};

/// Solana mainnet RPC URL
const SOLANA_MAINNET_RPC: &str = "https://api.mainnet-beta.solana.com";
/// How frequently to poll for events (seconds)
const DEFAULT_POLL_INTERVAL_S: u64 = 3;

/// Drift account indexing service 🏎️
#[derive(Parser, Debug)]
struct CliArgs {
    /// List of accounts to monitor
    #[clap(long, use_value_delimiter = true, value_delimiter = ',')]
    accounts: Vec<String>,
    /// Db connection string
    #[clap(long)]
    db: Option<String>,
    /// Solana RPC endpoint
    #[clap(long)]
    rpc: Option<String>,
    /// Polling interval (seconds)
    #[clap(long, default_value_t = DEFAULT_POLL_INTERVAL_S)]
    poll: u64,
}

#[tokio::main]
async fn main() {
    env_logger::init();

    let args = CliArgs::parse();
    let mut db_conn_str = args.db;
    let mut rpc_url = args.rpc;
    // env vars have priority of cli args
    for (k, v) in env::vars() {
        match k.as_str() {
            "INDEXER_SOLANA_RPC_URL" => {
                rpc_url.replace(v);
            }
            "INDEXER_DB_CONN_STR" => {
                db_conn_str.replace(v);
            }
            _ => (),
        }
    }
    let rpc_url = rpc_url.unwrap_or_else(|| SOLANA_MAINNET_RPC.to_string());
    let db_conn_str = db_conn_str.unwrap_or_default();
    info!("using: db: {db_conn_str}, rpc: {rpc_url}");

    let rpc_client = Arc::new(RpcClient::new(rpc_url));
    let db_client = Arc::new(MongoDbClient::init(db_conn_str.as_str()).await);
    let poll = Duration::from_secs(args.poll);

    select_all(
        args.accounts
            .into_iter()
            .map(|acc| spawn_indexer(acc, Arc::clone(&db_client), Arc::clone(&rpc_client), poll)),
    )
    .await
    .0
    .unwrap()
    .unwrap();
}

/// Spawn an indexer thread for `account`
fn spawn_indexer<T: IndexerBackend + 'static>(
    account: String,
    db: Arc<T>,
    rpc: Arc<RpcClient>,
    poll: Duration,
) -> JoinHandle<Result<(), IndexerError>> {
    info!("spawning indexer for: {}", account);
    tokio::spawn(async move {
        DriftEventIndexer::new(db, rpc)
            .run(account.as_str(), poll)
            .await
    })
}
