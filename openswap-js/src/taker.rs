//! Openswap Taker N-API bindings
//!
//! This module provides N-API bindings for the openswap taker functionality.

use crate::types::{
  Address, AddressType, Amount, BackendConfig, Balances, FeeRates, GetTransactionResultDetail,
  ListTransactionResult, ListUnspentResultEntry, MakerOfferCandidate, Offer, OfferBook, OutPoint,
  RPCConfig as RpcConfig, ScriptBuf, SignedAmountSats, SwapReport, Txid, UtxoSpendInfo,
  WalletTxInfo,
};
use napi::bindgen_prelude::*;
use napi_derive::napi;
use openswap::{
  bitcoin::{
    address::NetworkUnchecked, Address as BitcoinAddress, Amount as csAmount,
    OutPoint as BitcoinOutPoint, Txid as csTxid,
  },
  fee_estimation::{BlockTarget, FeeEstimator},
  protocol::ProtocolVersion,
  taker::{
    api::{
      ConnectionType, SwapParams as OpenswapSwapParams, Taker as OpenswapTaker, TakerInitConfig,
    },
    offers::{MakerAddress, OfferSyncClient},
  },
  wallet::{
    ffi, AddressType as csAddressType, BackendConfig as OpenswapBackendConfig,
    CoreRpcConfig as OpenswapCoreRpcConfig, UTXOSpendInfo as csUtxoSpendInfo,
  },
};
use std::{
  path::PathBuf,
  str::FromStr,
  sync::{atomic::AtomicBool, Arc, Mutex},
};

#[napi(object)]
pub struct SwapParams {
  pub protocol: Option<String>,
  pub send_amount: i64,
  pub maker_count: u32,
  pub tx_count: Option<u32>,
  pub required_confirms: Option<u32>,
  pub manually_selected_outpoints: Option<Vec<OutPoint>>,
  pub preferred_makers: Option<Vec<String>>,
  pub payment_address: Option<String>,
}

fn checked_satoshi_amount(amount: i64) -> Result<u64> {
  u64::try_from(amount).map_err(|_| napi::Error::from_reason("Amount must be non-negative"))
}

impl TryFrom<SwapParams> for OpenswapSwapParams {
  type Error = napi::Error;

  fn try_from(params: SwapParams) -> Result<Self> {
    let protocol = match params.protocol.as_deref().unwrap_or("Legacy") {
      "Legacy" | "legacy" => ProtocolVersion::Legacy,
      "Taproot" | "taproot" => ProtocolVersion::Taproot,
      other => {
        return Err(napi::Error::from_reason(format!(
          "Invalid protocol: {} (expected legacy or taproot)",
          other
        )));
      }
    };

    let send_amount = csAmount::from_sat(checked_satoshi_amount(params.send_amount)?);

    let manually_selected_outpoints = params
      .manually_selected_outpoints
      .map(|outpoints| {
        outpoints
          .into_iter()
          .map(|outpoint| {
            let txid = csTxid::from_str(&outpoint.txid)
              .map_err(|error| napi::Error::from_reason(format!("Invalid txid: {error}")))?;
            Ok(BitcoinOutPoint::new(txid, outpoint.vout))
          })
          .collect::<Result<Vec<_>, _>>()
      })
      .transpose()?;

    let payment_address = params
      .payment_address
      .map(|address| {
        BitcoinAddress::<NetworkUnchecked>::from_str(&address).map_err(|error| {
          napi::Error::from_reason(format!("Invalid payment address '{address}': {error}"))
        })
      })
      .transpose()?;

    Ok(OpenswapSwapParams {
      protocol,
      send_amount,
      maker_count: params.maker_count as usize,
      tx_count: params.tx_count.unwrap_or(1),
      required_confirms: params.required_confirms.unwrap_or(1),
      manually_selected_outpoints,
      preferred_makers: params.preferred_makers,
      payment_address,
    })
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn params(protocol: Option<&str>, send_amount: i64) -> SwapParams {
    SwapParams {
      protocol: protocol.map(str::to_owned),
      send_amount,
      maker_count: 2,
      tx_count: None,
      required_confirms: None,
      manually_selected_outpoints: None,
      preferred_makers: None,
      payment_address: None,
    }
  }

  #[test]
  fn satoshi_amount_rejects_negative_values_and_preserves_boundaries() {
    assert_eq!(checked_satoshi_amount(0).unwrap(), 0);
    assert_eq!(checked_satoshi_amount(i64::MAX).unwrap(), i64::MAX as u64);
    assert_eq!(
      checked_satoshi_amount(-1).unwrap_err().reason,
      "Amount must be non-negative"
    );
  }

  #[test]
  fn swap_params_apply_documented_defaults_and_accept_both_protocol_cases() {
    let defaults = OpenswapSwapParams::try_from(params(None, 50_000)).unwrap();
    assert_eq!(defaults.protocol, ProtocolVersion::Legacy);
    assert_eq!(defaults.send_amount.to_sat(), 50_000);
    assert_eq!(defaults.maker_count, 2);
    assert_eq!(defaults.tx_count, 1);
    assert_eq!(defaults.required_confirms, 1);

    for value in ["Legacy", "legacy"] {
      assert_eq!(
        OpenswapSwapParams::try_from(params(Some(value), 1))
          .unwrap()
          .protocol,
        ProtocolVersion::Legacy
      );
    }
    for value in ["Taproot", "taproot"] {
      assert_eq!(
        OpenswapSwapParams::try_from(params(Some(value), 1))
          .unwrap()
          .protocol,
        ProtocolVersion::Taproot
      );
    }
  }

  #[test]
  fn swap_params_validate_user_supplied_protocol_outpoint_and_address() {
    let protocol_error = OpenswapSwapParams::try_from(params(Some("Unified"), 1)).unwrap_err();
    assert_eq!(
      protocol_error.reason,
      "Invalid protocol: Unified (expected legacy or taproot)"
    );

    let invalid_txid = "not-a-txid";
    let txid_error = csTxid::from_str(invalid_txid).unwrap_err();
    let mut invalid_outpoint = params(None, 1);
    invalid_outpoint.manually_selected_outpoints = Some(vec![OutPoint {
      txid: invalid_txid.to_owned(),
      vout: 0,
    }]);
    assert_eq!(
      OpenswapSwapParams::try_from(invalid_outpoint)
        .unwrap_err()
        .reason,
      format!("Invalid txid: {txid_error}")
    );

    let invalid_address = "not-an-address";
    let address_error = BitcoinAddress::<NetworkUnchecked>::from_str(invalid_address).unwrap_err();
    let mut invalid_address_params = params(None, 1);
    invalid_address_params.payment_address = Some(invalid_address.to_owned());
    assert_eq!(
      OpenswapSwapParams::try_from(invalid_address_params)
        .unwrap_err()
        .reason,
      format!("Invalid payment address '{invalid_address}': {address_error}")
    );
  }
}

#[napi]
pub struct Taker {
  inner: Arc<Mutex<OpenswapTaker>>,
  /// Clone-able client for the background offer sync service. Used by the
  /// async sync method so it can run without holding `inner`'s Mutex, leaving
  /// other Taker calls free to proceed concurrently.
  offer_sync: OfferSyncClient,
}

/// AsyncTask that runs `sync_offerbook_and_wait` on libuv's worker pool so the
/// JS event loop is not blocked. Resolves to `undefined` on success. Uses the
/// `OfferSyncClient` directly (no `inner` Mutex), so other taker methods can
/// run in parallel during the sync.
pub struct SyncOfferbookTask {
  client: OfferSyncClient,
}

impl Task for SyncOfferbookTask {
  type Output = ();
  type JsValue = ();

  fn compute(&mut self) -> Result<Self::Output> {
    self
      .client
      .sync_and_wait()
      .map_err(|e| napi::Error::from_reason(format!("Offerbook sync error: {:?}", e)))
  }

  fn resolve(&mut self, _env: Env, _output: Self::Output) -> Result<Self::JsValue> {
    Ok(())
  }
}

/// AsyncTask that runs a single-maker poll on libuv's worker pool. Uses the
/// `OfferSyncClient` directly (no `inner` Mutex), so other taker methods can
/// run in parallel during the poll.
pub struct PollMakerTask {
  client: OfferSyncClient,
  address: String,
}

impl Task for PollMakerTask {
  type Output = openswap::taker::offers::MakerOfferCandidate;
  type JsValue = MakerOfferCandidate;

  fn compute(&mut self) -> Result<Self::Output> {
    let parsed = MakerAddress::try_from(self.address.clone())
      .map_err(|e| napi::Error::from_reason(format!("Invalid maker address: {}", e)))?;
    self
      .client
      .poll_maker(parsed)
      .map_err(|e| napi::Error::from_reason(format!("Poll maker error: {:?}", e)))
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(MakerOfferCandidate::from(output))
  }
}

#[napi]
#[allow(clippy::too_many_arguments)]
impl Taker {
  #[napi(constructor)]
  pub fn init(
    data_dir: Option<String>,
    wallet_file_name: Option<String>,
    rpc_config: Option<RpcConfig>,
    // _behavior: Option<TakerBehavior>,
    control_port: Option<u16>,
    tor_auth_password: Option<String>,
    zmq_addr: String,
    password: Option<String>,
    backend_config: Option<BackendConfig>,
    check_blocklist: Option<bool>,
  ) -> Result<Self> {
    let data_dir = data_dir.map(PathBuf::from);
    let backend = match backend_config {
      Some(config) => OpenswapBackendConfig::try_from(config)?,
      None => OpenswapBackendConfig::CoreRpc(
        rpc_config
          .map(|cfg| cfg.into_core_rpc_config(zmq_addr.clone()))
          .unwrap_or_else(|| OpenswapCoreRpcConfig {
            zmq_addr: zmq_addr.clone(),
            ..OpenswapCoreRpcConfig::default()
          }),
      ),
    };

    let init_config = TakerInitConfig {
      data_dir,
      wallet_name: wallet_file_name.unwrap_or_else(|| "taker-wallet".to_string()),
      backend,
      control_port,
      tor_auth_password,
      socks_port: 9050,
      check_blocklist,
      password,
      connection_type: ConnectionType::Tor,
      nostr_relays: TakerInitConfig::default().nostr_relays,
    };

    let taker = OpenswapTaker::init(init_config)
      .map_err(|e| napi::Error::from_reason(format!("Init error: {:?}", e)))?;

    let offer_sync = taker.offer_sync_client();

    Ok(Self {
      inner: Arc::new(Mutex::new(taker)),
      offer_sync,
    })
  }

  #[napi]
  pub fn setup_logging(data_dir: Option<String>, level: String) -> Result<()> {
    let path = data_dir.map(PathBuf::from);

    let log_level = match level.to_lowercase().as_str() {
      "trace" => log::LevelFilter::Trace,
      "debug" => log::LevelFilter::Debug,
      "info" => log::LevelFilter::Info,
      "warn" => log::LevelFilter::Warn,
      "error" => log::LevelFilter::Error,
      "off" => log::LevelFilter::Off,
      _ => log::LevelFilter::Info,
    };

    openswap::utill::setup_taker_logger(log_level, false, path);
    Ok(())
  }

  #[napi]
  pub fn init_native_logging() {
    // For full backtrace panics
    console_error_panic_hook::set_once();
    // This makes ALL log:: macros from any crate go to the JS console
    console_log::init_with_level(log::Level::Trace).expect("Failed to initialize console_log");
    log::info!("Rust logging → Electron console is ready!");
  }

  /// Fetch fee estimates from Mempool.space API with automatic fallback to Esplora
  #[napi]
  pub fn fetch_mempool_fees() -> Result<FeeRates> {
    // mempool.space serves live data and is recommended for user facing apps over esplora, the latter serving historical(mov_avg)+live
    let fees = FeeEstimator::fetch_mempool_fees()
      .or_else(|mempool_err| {
        log::warn!(
          "Mempool.space API failed: {:?}, falling back to Esplora",
          mempool_err
        );
        FeeEstimator::fetch_esplora_fees()
      })
      .map_err(|e| napi::Error::from_reason(format!("Both fee APIs failed: {:?}", e)))?;

    let get = |target| {
      fees
        .get(&target)
        .ok_or_else(|| napi::Error::from_reason(format!("Missing fee for {:?}", target)))
    };

    Ok(FeeRates {
      fastest: *get(BlockTarget::Fastest)?,
      standard: *get(BlockTarget::Standard)?,
      economy: *get(BlockTarget::Economy)?,
    })
  }

  #[napi]
  pub fn prepare_openswap(&self, swap_params: SwapParams) -> Result<String> {
    let params = OpenswapSwapParams::try_from(swap_params)?;
    let mut taker = self
      .inner
      .lock()
      .map_err(|e| napi::Error::from_reason(format!("Failed to acquire taker lock: {}", e)))?;
    let summary = taker
      .prepare_swap(params)
      .map_err(|e| napi::Error::from_reason(format!("Prepare openswap error: {:?}", e)))?;
    Ok(summary.swap_id)
  }

  #[napi]
  pub fn start_openswap(&self, swap_id: String) -> Result<SwapReport> {
    let mut taker = self
      .inner
      .lock()
      .map_err(|e| napi::Error::from_reason(format!("Failed to acquire taker lock: {}", e)))?;
    let report = taker
      .start_swap(&swap_id)
      .map_err(|e| napi::Error::from_reason(format!("Start openswap error: {:?}", e)))?;
    Ok(SwapReport::from(report))
  }

  #[napi]
  pub fn sync_offerbook_and_wait(&self) -> Result<()> {
    let taker = self
      .inner
      .lock()
      .map_err(|e| napi::Error::from_reason(format!("Failed to acquire taker lock: {}", e)))?;
    taker
      .sync_offerbook_and_wait()
      .map_err(|e| napi::Error::from_reason(format!("Offerbook sync error: {:?}", e)))
  }

  /// Async variant of `sync_offerbook_and_wait`. Runs the blocking sync on
  /// libuv's worker pool and returns a Promise that resolves when the cycle
  /// completes. Does NOT hold the inner Taker Mutex, so other Taker methods
  /// (getBalance, listUnspent, etc.) remain callable in parallel during the
  /// sync. Removes the need for a separate Node `worker_thread`.
  #[napi(ts_return_type = "Promise<void>")]
  pub fn sync_offerbook_and_wait_async(&self) -> AsyncTask<SyncOfferbookTask> {
    AsyncTask::new(SyncOfferbookTask {
      client: self.offer_sync.clone(),
    })
  }

  /// Trigger a single-maker offer fetch + fidelity verification cycle on demand.
  /// Routes through the cloned `OfferSyncClient`, so it does NOT hold the inner
  /// Taker Mutex — safe to call concurrently with other Taker methods.
  /// Returns the maker's final state after the poll.
  #[napi(ts_return_type = "Promise<MakerOfferCandidate>")]
  pub fn poll_maker_async(&self, address: String) -> AsyncTask<PollMakerTask> {
    AsyncTask::new(PollMakerTask {
      client: self.offer_sync.clone(),
      address,
    })
  }

  /// Remove a maker from the offerbook by address. Persists to disk.
  /// Returns `true` if a matching entry was removed, `false` otherwise.
  #[napi]
  pub fn remove_maker(&self, address: String) -> Result<bool> {
    let taker = self
      .inner
      .lock()
      .map_err(|e| napi::Error::from_reason(format!("Failed to acquire taker lock: {}", e)))?;
    taker
      .remove_maker(address)
      .map_err(|e| napi::Error::from_reason(format!("Remove maker error: {:?}", e)))
  }

  #[napi]
  pub fn get_transactions(
    &self,
    count: Option<u32>,
    skip: Option<u32>,
  ) -> Result<Vec<ListTransactionResult>> {
    let taker = self
      .inner
      .lock()
      .map_err(|e| napi::Error::from_reason(format!("Failed to acquire taker lock: {}", e)))?;
    let wallet = taker
      .get_wallet()
      .read()
      .map_err(|e| napi::Error::from_reason(format!("Failed to acquire wallet lock: {}", e)))?;
    let txns = wallet
      .get_transactions(count.map(|c| c as usize), skip.map(|s| s as usize))
      .map_err(|error| napi::Error::from_reason(format!("Get transactions error: {error}")))?;

    Ok(
      txns
        .into_iter()
        .map(|tx| ListTransactionResult {
          info: {
            WalletTxInfo {
              confirmations: tx.info.confirmations,
              blockhash: tx.info.blockhash.map(|h| h.to_string()),
              blockindex: tx.info.blockindex.map(|i| i as u32),
              blocktime: tx.info.blocktime.map(|t| t as i64),
              blockheight: tx.info.blockheight,
              txid: Txid {
                value: tx.info.txid.to_string(),
              },
              time: tx.info.time as i64,
              timereceived: tx.info.timereceived as i64,
              bip125_replaceable: format!("{:?}", tx.info.bip125_replaceable),
              wallet_conflicts: tx
                .info
                .wallet_conflicts
                .into_iter()
                .map(|txid| Txid {
                  value: txid.to_string(),
                })
                .collect(),
            }
          },
          detail: {
            GetTransactionResultDetail {
              address: tx.detail.address.map(|addr| Address {
                address: addr.assume_checked().to_string(),
              }),
              category: format!("{:?}", tx.detail.category),
              amount: SignedAmountSats {
                sats: tx.detail.amount.to_sat(),
              },
              label: tx.detail.label,
              vout: tx.detail.vout,
              fee: tx.detail.fee.map(|f| SignedAmountSats { sats: f.to_sat() }),
              abandoned: tx.detail.abandoned,
            }
          },
          trusted: tx.trusted,
          comment: tx.comment,
        })
        .collect(),
    )
  }

  #[napi]
  pub fn get_next_internal_addresses(
    &self,
    count: u32,
    address_type: AddressType,
  ) -> Result<Vec<Address>> {
    let cs_address_type = csAddressType::try_from(address_type)?;
    let taker = self
      .inner
      .lock()
      .map_err(|e| napi::Error::from_reason(format!("Failed to acquire taker lock: {}", e)))?;
    let mut wallet = taker
      .get_wallet()
      .write()
      .map_err(|e| napi::Error::from_reason(format!("Failed to acquire wallet lock: {}", e)))?;
    let wallet = &mut *wallet;
    let internal_addresses = wallet
      .get_next_internal_addresses(count, cs_address_type)
      .map_err(|error| {
        napi::Error::from_reason(format!("Get internal addresses error: {error}"))
      })?;
    Ok(internal_addresses.into_iter().map(Address::from).collect())
  }

  #[napi]
  pub fn get_next_external_address(&mut self, address_type: AddressType) -> Result<Address> {
    let cs_address_type = csAddressType::try_from(address_type)?;
    let taker = self
      .inner
      .lock()
      .map_err(|e| napi::Error::from_reason(format!("Failed to acquire taker lock: {}", e)))?;
    let mut wallet = taker
      .get_wallet()
      .write()
      .map_err(|e| napi::Error::from_reason(format!("Failed to acquire wallet lock: {}", e)))?;
    let external_address = wallet
      .get_next_external_address(cs_address_type)
      .map_err(|error| {
        napi::Error::from_reason(format!("Get next external address error: {error}"))
      })?;
    Ok(Address::from(external_address))
  }

  // Get Name of the Wallet
  #[napi]
  pub fn get_name(&self) -> Result<String> {
    let taker = self
      .inner
      .lock()
      .map_err(|e| napi::Error::from_reason(format!("Failed to acquire taker lock: {}", e)))?;
    let wallet = taker
      .get_wallet()
      .read()
      .map_err(|e| napi::Error::from_reason(format!("Failed to acquire wallet lock: {}", e)))?;
    Ok(wallet.get_name().to_string())
  }

  #[napi]
  pub fn list_all_utxo_spend_info(&self) -> Result<Vec<(ListUnspentResultEntry, UtxoSpendInfo)>> {
    let taker = self
      .inner
      .lock()
      .map_err(|e| napi::Error::from_reason(format!("Failed to acquire taker lock: {}", e)))?;
    let wallet = taker
      .get_wallet()
      .read()
      .map_err(|e| napi::Error::from_reason(format!("Failed to acquire wallet lock: {}", e)))?;
    let entries = wallet.list_all_utxo_spend_info();
    Ok(
      entries
        .into_iter()
        .map(|(cs_utxo, cs_info)| {
          let utxo = ListUnspentResultEntry {
            txid: Txid::from(cs_utxo.txid),
            vout: cs_utxo.vout,
            address: cs_utxo
              .address
              .as_ref()
              .map(|a| a.clone().assume_checked().to_string()),
            label: cs_utxo.label.clone(),
            script_pub_key: ScriptBuf::from(cs_utxo.script_pub_key.clone()),
            amount: Amount::from(cs_utxo.amount),
            confirmations: cs_utxo.confirmations,
            redeem_script: cs_utxo.redeem_script.clone().map(ScriptBuf::from),
            witness_script: cs_utxo.witness_script.clone().map(ScriptBuf::from),
            spendable: cs_utxo.spendable,
            solvable: cs_utxo.solvable,
            desc: cs_utxo.descriptor.clone(),
            safe: cs_utxo.safe,
          };
          let spend_info = match cs_info {
            csUtxoSpendInfo::SeedCoin {
              path,
              input_value,
              address_type: _,
            } => UtxoSpendInfo {
              spend_type: "SeedCoin".to_string(),
              path: Some(path.to_string()),
              multisig_redeemscript: None,
              input_value: Some(Amount::from(input_value)),
              index: None,
            },
            csUtxoSpendInfo::IncomingSwapCoin {
              multisig_redeemscript,
            } => UtxoSpendInfo {
              spend_type: "IncomingSwapCoin".to_string(),
              path: None,
              multisig_redeemscript: Some(ScriptBuf::from(multisig_redeemscript.clone())),
              input_value: None,
              index: None,
            },
            csUtxoSpendInfo::OutgoingSwapCoin {
              multisig_redeemscript,
            } => UtxoSpendInfo {
              spend_type: "OutgoingSwapCoin".to_string(),
              path: None,
              multisig_redeemscript: Some(ScriptBuf::from(multisig_redeemscript.clone())),
              input_value: None,
              index: None,
            },
            csUtxoSpendInfo::TimelockContract {
              swapcoin_multisig_redeemscript,
              input_value,
            } => UtxoSpendInfo {
              spend_type: "TimelockContract".to_string(),
              path: None,
              multisig_redeemscript: Some(ScriptBuf::from(swapcoin_multisig_redeemscript.clone())),
              input_value: Some(Amount::from(input_value)),
              index: None,
            },
            csUtxoSpendInfo::HashlockContract {
              swapcoin_multisig_redeemscript,
              input_value,
            } => UtxoSpendInfo {
              spend_type: "HashlockContract".to_string(),
              path: None,
              multisig_redeemscript: Some(ScriptBuf::from(swapcoin_multisig_redeemscript.clone())),
              input_value: Some(Amount::from(input_value)),
              index: None,
            },
            csUtxoSpendInfo::FidelityBondCoin { index, input_value } => UtxoSpendInfo {
              spend_type: "FidelityBondCoin".to_string(),
              path: None,
              multisig_redeemscript: None,
              input_value: Some(Amount::from(input_value)),
              index: Some(index),
            },
            csUtxoSpendInfo::SweptCoin {
              path,
              input_value,
              address_type: _,
            } => UtxoSpendInfo {
              spend_type: "SweptCoin".to_string(),
              path: Some(path.to_string()),
              multisig_redeemscript: None,
              input_value: Some(Amount::from(input_value)),
              index: None,
            },
          };
          (utxo, spend_info)
        })
        .collect(),
    )
  }

  #[napi]
  pub fn backup(&self, destination_path: String, password: Option<String>) -> Result<()> {
    let taker = self
      .inner
      .lock()
      .map_err(|e| napi::Error::from_reason(format!("Failed to acquire taker lock: {}", e)))?;
    taker
      .get_wallet()
      .write()
      .map_err(|e| napi::Error::from_reason(format!("Failed to acquire wallet lock: {}", e)))?
      .backup_wallet_gui_app(destination_path, password)
      .map_err(|error| napi::Error::from_reason(format!("Backup error: {error}")))?;

    Ok(())
  }

  #[napi]
  pub fn restore_wallet_gui_app(
    data_dir: Option<String>,
    wallet_file_name: Option<String>,
    rpc_config: RpcConfig,
    backup_file: String,
    password: Option<String>,
  ) {
    let data_dir = data_dir.map(PathBuf::from);

    ffi::restore_wallet_gui_app(
      data_dir,
      wallet_file_name,
      OpenswapBackendConfig::CoreRpc(rpc_config.into()),
      backup_file.into(),
      password,
    );
  }

  #[napi]
  pub fn lock_unspendable_utxos(&self) -> Result<()> {
    let taker = self
      .inner
      .lock()
      .map_err(|e| napi::Error::from_reason(format!("Failed to acquire taker lock: {}", e)))?;
    taker
      .get_wallet()
      .write()
      .map_err(|e| napi::Error::from_reason(format!("Failed to acquire wallet lock: {}", e)))?
      .lock_unspendable_utxos()
      .map_err(|error| napi::Error::from_reason(format!("Lock error: {error}")))?;
    Ok(())
  }

  #[napi]
  pub fn send_to_address(
    &mut self,
    address: String,
    amount: i64,
    fee_rate: Option<f64>,
    manually_selected_outpoints: Option<Vec<OutPoint>>,
  ) -> Result<Txid> {
    let amount = checked_satoshi_amount(amount)?;
    let manually_selected_outpoints = manually_selected_outpoints
      .map(|outpoints| {
        outpoints
          .into_iter()
          .map(|outpoint| {
            let txid = csTxid::from_str(&outpoint.txid)
              .map_err(|error| napi::Error::from_reason(format!("Invalid txid: {error}")))?;
            Ok(BitcoinOutPoint::new(txid, outpoint.vout))
          })
          .collect::<Result<Vec<_>, _>>()
      })
      .transpose()?;
    let taker = self
      .inner
      .lock()
      .map_err(|e| napi::Error::from_reason(format!("Failed to acquire taker lock: {}", e)))?;
    let txid = taker
      .get_wallet()
      .write()
      .map_err(|e| napi::Error::from_reason(format!("Failed to acquire wallet lock: {}", e)))?
      .send_to_address(amount, address, fee_rate, manually_selected_outpoints)
      .map_err(|error| napi::Error::from_reason(format!("Send to address error: {error}")))?;
    Ok(txid.into())
  }

  /// Get wallet balances
  #[napi]
  pub fn get_balances(&self) -> Result<Balances> {
    let taker = self
      .inner
      .lock()
      .map_err(|e| napi::Error::from_reason(format!("Failed to acquire taker lock: {}", e)))?;
    let wallet = taker
      .get_wallet()
      .read()
      .map_err(|e| napi::Error::from_reason(format!("Failed to acquire wallet lock: {}", e)))?;
    let balances = wallet
      .get_balances()
      .map_err(|error| napi::Error::from_reason(format!("Get balances error: {error}")))?;
    Ok(Balances::from(balances))
  }

  #[napi]
  pub fn sync_and_save(&mut self) -> Result<()> {
    let taker = self
      .inner
      .lock()
      .map_err(|e| napi::Error::from_reason(format!("Failed to acquire taker lock: {}", e)))?;
    taker
      .get_wallet()
      .write()
      .map_err(|e| napi::Error::from_reason(format!("Failed to acquire wallet lock: {}", e)))?
      .sync_and_save(&AtomicBool::new(false))
      .map_err(|error| napi::Error::from_reason(format!("Sync wallet error: {error}")))?;
    Ok(())
  }

  #[napi]
  pub fn display_offer(&self, maker_offer: Offer) -> Result<String> {
    let offer_json = serde_json::json!({
        "base_fee": maker_offer.base_fee,
        "amount_relative_fee_pct": maker_offer.amount_relative_fee_pct,
        "time_relative_fee_pct": maker_offer.time_relative_fee_pct,
        "required_confirms": maker_offer.required_confirms,
        "minimum_locktime": maker_offer.minimum_locktime,
        "max_size": maker_offer.max_size,
        "min_size": maker_offer.min_size,
        // "tweakable_point": maker_offer.tweakable_point,
        // "fidelity": maker_offer.fidelity
    });

    serde_json::to_string_pretty(&offer_json)
      .map_err(|error| napi::Error::from_reason(format!("Failed to serialize offer: {error}")))
  }

  /// Recover from a failed swap
  #[napi]
  pub fn recover_active_swap(&mut self) -> Result<()> {
    let mut taker = self
      .inner
      .lock()
      .map_err(|e| napi::Error::from_reason(format!("Failed to acquire taker lock: {}", e)))?;
    taker
      .recover_active_swap()
      .map_err(|e| napi::Error::from_reason(format!("Recover error: {:?}", e)))?;
    Ok(())
  }

  #[napi]
  pub fn fetch_all_makers(&self) -> Result<Vec<String>> {
    let taker = self
      .inner
      .lock()
      .map_err(|e| napi::Error::from_reason(format!("Failed to acquire taker lock: {}", e)))?;

    let offerbook = taker
      .fetch_offers()
      .map_err(|e| napi::Error::from_reason(format!("Fetch offers error: {:?}", e)))?;
    let all_makers = offerbook.all_makers();

    let addresses = all_makers
      .into_iter()
      .map(|maker| maker.address.to_string())
      .collect();

    Ok(addresses)
  }

  #[napi]
  pub fn fetch_offers(&self) -> Result<OfferBook> {
    let taker = self
      .inner
      .lock()
      .map_err(|e| napi::Error::from_reason(format!("Failed to acquire taker lock: {}", e)))?;

    let offerbook = taker
      .fetch_offers()
      .map_err(|e| napi::Error::from_reason(format!("Fetch offers error: {:?}", e)))?;

    Ok(OfferBook::from(&offerbook))
  }

  #[napi]
  pub fn is_wallet_encrypted(wallet_path: String) -> Result<bool> {
    let path = PathBuf::from(wallet_path);

    openswap::wallet::Wallet::is_wallet_encrypted(&path).map_err(|error| {
      napi::Error::from_reason(format!("Failed to check wallet encryption: {error}"))
    })
  }

  #[napi]
  pub fn verify_deniability(&self, swap_id: String) -> Result<bool> {
    let taker = self
      .inner
      .lock()
      .map_err(|e| napi::Error::from_reason(format!("Failed to acquire taker lock: {}", e)))?;

    let is_deniable = taker.verify_deniability(&swap_id).map_err(|error| {
      napi::Error::from_reason(format!("Deniability verification error: {error}"))
    })?;

    Ok(is_deniable)
  }
}
