use crate::cli::SyncParams;
use alloy::primitives::Address;
use anyhow::Context;
use dc_db::db_metrics::DbMetrics;
use dc_db::{DatabaseService, DeoxysBackend};
use dc_eth::client::EthereumClient;
use dc_eth::l1_gas_price::L1GasPrices;
use dc_metrics::MetricsRegistry;
use dc_sync::fetch::fetchers::FetchConfig;
use dc_sync::metrics::block_metrics::BlockMetrics;
use dc_telemetry::TelemetryHandle;
use dp_utils::service::Service;
use futures::lock::Mutex;
use std::sync::Arc;
use std::time::Duration;
use tokio::task::JoinSet;

#[derive(Clone)]
pub struct SyncService {
    db_backend: Arc<DeoxysBackend>,
    fetch_config: FetchConfig,
    backup_every_n_blocks: Option<u64>,
    eth_client: EthereumClient,
    l1_gas_prices: Arc<Mutex<L1GasPrices>>,
    starting_block: Option<u64>,
    block_metrics: BlockMetrics,
    db_metrics: DbMetrics,
    start_params: Option<TelemetryHandle>,
    disabled: bool,
    pending_block_poll_interval: Duration,
}

impl SyncService {
    pub async fn new(
        config: &SyncParams,
        db: &DatabaseService,
        metrics_handle: MetricsRegistry,
        telemetry: TelemetryHandle,
        l1_gas_prices: Arc<Mutex<L1GasPrices>>,
    ) -> anyhow::Result<Self> {
        // TODO: create l1 metrics here
        let block_metrics = BlockMetrics::register(&metrics_handle)?;
        let db_metrics = DbMetrics::register(&metrics_handle)?;
        let fetch_config = config.block_fetch_config();

        let l1_endpoint = if !config.sync_l1_disabled {
            if let Some(l1_rpc_url) = &config.l1_endpoint {
                Some(l1_rpc_url.clone())
            } else {
                return Err(anyhow::anyhow!(
                    "❗ No L1 endpoint provided. You must provide one in order to verify the synced state."
                ));
            }
        } else {
            None
        };

        let core_address = Address::from_slice(config.network.l1_core_address().as_bytes());
        let eth_client = EthereumClient::new(l1_endpoint.unwrap(), core_address, metrics_handle)
            .await
            .context("Creating ethereum client")?;

        Ok(Self {
            db_backend: Arc::clone(db.backend()),
            fetch_config,
            eth_client,
            l1_gas_prices,
            starting_block: config.starting_block,
            backup_every_n_blocks: config.backup_every_n_blocks,
            block_metrics,
            db_metrics,
            start_params: Some(telemetry),
            disabled: config.sync_disabled,
            pending_block_poll_interval: Duration::from_secs(config.pending_block_poll_interval),
        })
    }
}

#[async_trait::async_trait]
impl Service for SyncService {
    async fn start(&mut self, join_set: &mut JoinSet<anyhow::Result<()>>) -> anyhow::Result<()> {
        if self.disabled {
            return Ok(());
        }
        let SyncService {
            fetch_config,
            backup_every_n_blocks,
            eth_client,
            l1_gas_prices,
            starting_block,
            block_metrics,
            db_metrics,
            pending_block_poll_interval,
            ..
        } = self.clone();
        let telemetry = self.start_params.take().context("Service already started")?;

        let db_backend = Arc::clone(&self.db_backend);

        join_set.spawn(async move {
            dc_sync::starknet_sync_worker::sync(
                &db_backend,
                fetch_config,
                eth_client,
                l1_gas_prices,
                starting_block,
                backup_every_n_blocks,
                block_metrics,
                db_metrics,
                telemetry,
                pending_block_poll_interval,
            )
            .await
        });

        Ok(())
    }
}
