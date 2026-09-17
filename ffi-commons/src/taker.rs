//! Openswap Taker UniFFI bindings
//!
//! This module provides UniFFI bindings for the openswap taker functionality.

use crate::{
    AddressType,
    types::{
        Address, Amount, BackendConfig, Balances, GetTransactionResultDetail,
        ListTransactionResult, ListUnspentResultEntry, MakerOfferCandidate, Offer, OfferBook,
        OutPoint, RpcConfig, ScriptBuf, SignedAmountSats, SwapReport, TakerError, TotalUtxoInfo,
        Txid, UtxoSpendInfo, WalletTxInfo,
    },
};
use openswap::{
    bitcoin::{
        Address as OpenswapAddress, Amount as openswapAmount, OutPoint as openswapOutPoint,
        Txid as openswapTxid, address::NetworkUnchecked,
    },
    protocol::ProtocolVersion,
    taker::api::{
        ConnectionType, SwapParams as OpenswapSwapParams, Taker as OpenswapTaker, TakerInitConfig,
    },
    wallet::{
        BackendConfig as OpenswapBackendConfig, CoreRpcConfig as OpenswapCoreRpcConfig,
        UTXOSpendInfo as csUtxoSpendInfo,
    },
};
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};

/// Swap specific parameters. These are user's policy and can differ among swaps.
/// SwapParams govern the criteria to find suitable set of makers from the offerbook.
///
/// If no maker matches with a given SwapParam, that openswap round will fail.
#[derive(uniffi::Record)]
pub struct SwapParams {
    /// Protocol to use: Legacy or Taproot.
    pub protocol: Option<String>,
    /// Total Amount to Swap.
    pub send_amount: u64,
    /// How many hops.
    pub maker_count: u32,
    /// Number of transaction splits.
    pub tx_count: Option<u32>,
    /// Required funding confirmations.
    pub required_confirms: Option<u32>,
    /// User selected UTXOs
    pub manually_selected_outpoints: Option<Vec<OutPoint>>,
    /// Optional explicit maker addresses.
    pub preferred_makers: Option<Vec<String>>,
    /// Optional third-party address that receives the settled swap amount.
    #[uniffi(default = None)]
    pub payment_address: Option<String>,
}

fn checked_satoshi_amount(amount: i64) -> Result<u64, TakerError> {
    u64::try_from(amount).map_err(|_| TakerError::General {
        msg: "Amount must be non-negative".to_string(),
    })
}

fn parse_protocol(protocol: Option<&str>) -> Result<ProtocolVersion, TakerError> {
    match protocol.unwrap_or("Legacy") {
        "Legacy" | "legacy" => Ok(ProtocolVersion::Legacy),
        "Taproot" | "taproot" => Ok(ProtocolVersion::Taproot),
        other => Err(TakerError::General {
            msg: format!("Invalid protocol: {} (expected legacy or taproot)", other),
        }),
    }
}

fn parse_outpoints(
    outpoints: Option<Vec<OutPoint>>,
) -> Result<Option<Vec<openswapOutPoint>>, TakerError> {
    outpoints
        .map(|outpoints| {
            outpoints
                .into_iter()
                .map(|outpoint| {
                    let txid = outpoint
                        .txid
                        .value
                        .parse::<openswapTxid>()
                        .map_err(|error| TakerError::General {
                            msg: format!("Invalid txid: {}", error),
                        })?;
                    Ok(openswapOutPoint::new(txid, outpoint.vout))
                })
                .collect::<Result<Vec<_>, _>>()
        })
        .transpose()
}

fn format_offer(maker_offer: &Offer) -> Result<String, TakerError> {
    let offer_json = serde_json::json!({
        "base_fee": maker_offer.base_fee,
        "amount_relative_fee_pct": maker_offer.amount_relative_fee_pct,
        "time_relative_fee_pct": maker_offer.time_relative_fee_pct,
        "required_confirms": maker_offer.required_confirms,
        "minimum_locktime": maker_offer.minimum_locktime,
        "max_size": maker_offer.max_size,
        "min_size": maker_offer.min_size,
    });

    serde_json::to_string_pretty(&offer_json).map_err(|error| TakerError::General {
        msg: format!("Failed to serialize offer: {error}"),
    })
}

/// SwapParams govern the criteria to find suitable set of makers from the offerbook.
impl TryFrom<SwapParams> for OpenswapSwapParams {
    type Error = TakerError;

    /// Swap specific parameters. These are user's policy and can differ among swaps.
    fn try_from(params: SwapParams) -> Result<Self, Self::Error> {
        let protocol = parse_protocol(params.protocol.as_deref())?;

        let send_amount = openswapAmount::from_sat(params.send_amount);

        let manually_selected_outpoints = parse_outpoints(params.manually_selected_outpoints)?;

        let payment_address = params
            .payment_address
            .map(|address| {
                address
                    .parse::<OpenswapAddress<NetworkUnchecked>>()
                    .map_err(|error| TakerError::General {
                        msg: format!("Invalid payment address '{address}': {error}"),
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

/// The Taker structure that performs bulk of the openswap protocol. Taker connects
/// to multiple Makers and send protocol messages sequentially to them. The communication
/// sequence and corresponding SwapCoin infos are stored in `ongoing_swap_state`.
#[derive(uniffi::Object)]
pub struct Taker {
    /// The Taker structure that performs bulk of the openswap protocol.
    taker: Mutex<OpenswapTaker>,
}

#[uniffi::export]
impl Taker {
    #[uniffi::constructor(default(check_blocklist = None))]
    // #[allow(clippy::too_many_arguments)]
    ///  Initializes a Taker structure.
    ///
    /// This function sets up a Taker instance with configurable parameters.
    /// It handles the initialization of data directories, wallet files, and RPC configurations.
    ///
    /// ### Parameters:
    /// - `data_dir`:
    ///   - `Some(value)`: Use the specified directory for storing data.
    ///   - `None`: Use the default data directory (e.g., for Linux: `~/.openswap/taker`).
    /// - `wallet_file_name`:
    ///   - `Some(value)`: Attempt to load a wallet file named `value`. If it does not exist,
    ///     a new wallet with the given name will be created.
    ///   - `None`: Create a new wallet file with the default name `taker-wallet`.
    /// - If `rpc_config` = `None`: Use the default [`RpcConfig`]
    /// - `check_blocklist`: `Some(true)` enables funding-source screening;
    ///   `None` uses the taker configuration file's setting.
    #[allow(clippy::too_many_arguments)]
    pub fn init(
        data_dir: Option<String>,
        wallet_file_name: Option<String>,
        rpc_config: Option<RpcConfig>,
        // _behavior: Option<TakerBehavior>,
        control_port: Option<u16>,
        tor_auth_password: Option<String>,
        zmq_addr: String,
        password: Option<String>,
        nostr_relays: Option<Vec<String>>,
        backend_config: Option<BackendConfig>,
        check_blocklist: Option<bool>,
    ) -> Result<Arc<Self>, TakerError> {
        let data_dir = data_dir.map(PathBuf::from);
        let backend = match backend_config {
            Some(config) => OpenswapBackendConfig::try_from(config)?,
            None => OpenswapBackendConfig::CoreRpc(
                rpc_config
                    .map(|config| config.into_core_rpc_config(zmq_addr.clone()))
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
            // `None` keeps the compiled-in default relays; `Some` lets callers
            // (e.g. tests pointing at a local relay) override them.
            nostr_relays: nostr_relays.unwrap_or_else(|| TakerInitConfig::default().nostr_relays),
        };

        let taker = OpenswapTaker::init(init_config)?;

        Ok(Arc::new(Self {
            taker: Mutex::new(taker),
        }))
    }

    /// Sets up the logger for the taker component.
    ///
    /// This method initializes the logging configuration for the taker, directing logs to both
    /// the console and a file. It sets the `RUST_LOG` environment variable to provide default
    /// log levels and configures log4rs with the specified filter level for fine-grained control
    /// of log verbosity.
    pub fn setup_logging(
        &self,
        data_dir: Option<String>,
        log_level: String,
    ) -> Result<(), TakerError> {
        let path = data_dir.map(PathBuf::from);
        let level = match log_level.to_lowercase().as_str() {
            "trace" => log::LevelFilter::Trace,
            "debug" => log::LevelFilter::Debug,
            "info" => log::LevelFilter::Info,
            "warn" => log::LevelFilter::Warn,
            "error" => log::LevelFilter::Error,
            _ => log::LevelFilter::Info,
        };
        openswap::utill::setup_taker_logger(level, true, path);
        Ok(())
    }

    /// Prepares an openswap and returns a swap id.
    pub fn prepare_openswap(&self, swap_params: SwapParams) -> Result<String, TakerError> {
        let params = OpenswapSwapParams::try_from(swap_params)?;
        let mut taker = self.taker.lock().map_err(|_| TakerError::General {
            msg: "Failed to acquire taker lock".to_string(),
        })?;
        let summary = taker.prepare_swap(params)?;
        Ok(summary.swap_id)
    }

    /// Starts execution for a prepared openswap.
    pub fn start_openswap(&self, swap_id: String) -> Result<SwapReport, TakerError> {
        let mut taker = self.taker.lock().map_err(|_| TakerError::General {
            msg: "Failed to acquire taker lock".to_string(),
        })?;
        let report = taker.start_swap(&swap_id)?;
        Ok(SwapReport::from(report))
    }

    /// Returns a list of recent Incoming Transactions (bydefault last 10)
    pub fn get_transactions(
        &self,
        count: Option<u32>,
        skip: Option<u32>,
    ) -> Result<Vec<ListTransactionResult>, TakerError> {
        let taker = self.taker.lock().map_err(|_| TakerError::General {
            msg: "Failed to acquire taker lock".to_string(),
        })?;
        let wallet = taker.get_wallet().read().map_err(|_| TakerError::General {
            msg: "Failed to acquire wallet read lock".to_string(),
        })?;
        let txns = wallet
            .get_transactions(count.map(|c| c as usize), skip.map(|s| s as usize))
            .map_err(|error| TakerError::Wallet {
                msg: format!("Get transactions error: {error}"),
            })?;

        Ok(txns
            .into_iter()
            .map(|tx| ListTransactionResult {
                info: WalletTxInfo {
                    confirmations: tx.info.confirmations,
                    blockhash: tx.info.blockhash.map(|h| h.to_string()),
                    blockindex: tx.info.blockindex.map(|i| i as u32),
                    blocktime: tx.info.blocktime.map(|t| t as i64),
                    blockheight: tx.info.blockheight,
                    txid: Txid::from(tx.info.txid),
                    time: tx.info.time as i64,
                    timereceived: tx.info.timereceived as i64,
                    bip125_replaceable: format!("{:?}", tx.info.bip125_replaceable),
                    wallet_conflicts: tx
                        .info
                        .wallet_conflicts
                        .into_iter()
                        .map(Txid::from)
                        .collect(),
                },
                detail: GetTransactionResultDetail {
                    address: tx.detail.address.map(|a| Address::from(a.assume_checked())),
                    category: format!("{:?}", tx.detail.category),
                    amount: SignedAmountSats::from(tx.detail.amount),
                    label: tx.detail.label,
                    vout: tx.detail.vout,
                    fee: tx.detail.fee.map(SignedAmountSats::from),
                    abandoned: tx.detail.abandoned,
                },
                trusted: tx.trusted,
                comment: tx.comment,
            })
            .collect())
    }

    /// Gets the next internal addresses from the HD keychain.
    pub fn get_next_internal_addresses(
        &self,
        count: u32,
        address_type: AddressType,
    ) -> Result<Vec<Address>, TakerError> {
        let cs_address_type = openswap::wallet::AddressType::try_from(address_type)?;
        let taker = self.taker.lock().map_err(|_| TakerError::General {
            msg: "Failed to acquire taker lock".to_string(),
        })?;
        let mut wallet = taker
            .get_wallet()
            .write()
            .map_err(|_| TakerError::General {
                msg: "Failed to acquire wallet write lock".to_string(),
            })?;
        let wallet = &mut *wallet;
        let internal_addresses = wallet
            .get_next_internal_addresses(count, cs_address_type)
            .map_err(|error| TakerError::Wallet {
                msg: format!("Get internal addresses error: {error}"),
            })?;
        Ok(internal_addresses.into_iter().map(Address::from).collect())
    }

    /// Gets the next external address from the HD keychain. Saves the wallet to disk
    pub fn get_next_external_address(
        &self,
        address_type: AddressType,
    ) -> Result<Address, TakerError> {
        let cs_address_type = openswap::wallet::AddressType::try_from(address_type)?;
        let taker = self.taker.lock().map_err(|_| TakerError::General {
            msg: "Failed to acquire taker lock".to_string(),
        })?;
        let mut wallet = taker
            .get_wallet()
            .write()
            .map_err(|_| TakerError::General {
                msg: "Failed to acquire wallet write lock".to_string(),
            })?;
        let external_address =
            wallet
                .get_next_external_address(cs_address_type)
                .map_err(|error| TakerError::Wallet {
                    msg: format!("Get next external address error: {error}"),
                })?;
        Ok(Address::from(external_address))
    }

    /// Returns a list all utxos with their spend info tracked by the wallet.
    /// Optionally takes in an Utxo list to reduce RPC calls. If None is given, the
    /// full list of utxo is fetched from core rpc.
    pub fn list_all_utxo_spend_info(&self) -> Result<Vec<TotalUtxoInfo>, TakerError> {
        let taker = self.taker.lock().map_err(|_| TakerError::General {
            msg: "Failed to acquire taker lock".to_string(),
        })?;
        let wallet = taker.get_wallet().read().map_err(|_| TakerError::General {
            msg: "Failed to acquire wallet read lock".to_string(),
        })?;
        let entries = wallet.list_all_utxo_spend_info();

        Ok(entries
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
                        multisig_redeemscript: Some(ScriptBuf::from(
                            swapcoin_multisig_redeemscript.clone(),
                        )),
                        input_value: Some(Amount::from(input_value)),
                        index: None,
                    },
                    csUtxoSpendInfo::HashlockContract {
                        swapcoin_multisig_redeemscript,
                        input_value,
                    } => UtxoSpendInfo {
                        spend_type: "HashlockContract".to_string(),
                        path: None,
                        multisig_redeemscript: Some(ScriptBuf::from(
                            swapcoin_multisig_redeemscript.clone(),
                        )),
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

                TotalUtxoInfo {
                    list_unspent_result_entry: utxo,
                    utxo_spend_info: spend_info,
                }
            })
            .collect())
    }

    /// Creates a wallet backup for GUI/FFI applications with optional encryption.
    ///
    /// This is a ffi-only wrapper around [`Wallet::backup`] that handles encryption
    /// material generation internally based on whether a password is provided.
    ///
    /// # Behavior
    ///
    /// - If `password` is `Some(pwd)` and not empty: Creates encrypted backup using the password
    /// - If `password` is `None` or empty string: Creates unencrypted backup (logs warning)
    /// - The backup is written as a `.json` file at the specified path
    ///
    /// # Parameters
    ///
    /// - `destination_path`: Destination file path for the backup (`.json`)
    /// - `password`: Optional password for encryption. Use `None` or empty string for plaintext backup
    ///
    /// # Example
    ///
    /// ```ignore
    /// // Encrypted backup
    /// wallet.backup_gui_app("/path/to/backup".to_string(), Some("my_password".to_string()))?;
    ///
    /// // Unencrypted backup
    /// wallet.backup_gui_app("/path/to/backup".to_string(), None)?;
    /// ```
    pub fn backup(
        &self,
        destination_path: String,
        password: Option<String>,
    ) -> Result<(), TakerError> {
        let taker = self.taker.lock().map_err(|_| TakerError::General {
            msg: "Failed to acquire taker lock".to_string(),
        })?;
        taker
            .get_wallet()
            .write()
            .map_err(|_| TakerError::General {
                msg: "Failed to acquire wallet write lock".to_string(),
            })?
            .backup_wallet_gui_app(destination_path, password)
            .map_err(|error| TakerError::Wallet {
                msg: format!("Backup error: {error}"),
            })?;
        Ok(())
    }

    /// Locks the fidelity and live_contract utxos which are not considered for spending from the wallet.
    pub fn lock_unspendable_utxos(&self) -> Result<(), TakerError> {
        let taker = self.taker.lock().map_err(|_| TakerError::General {
            msg: "Failed to acquire taker lock".to_string(),
        })?;
        taker
            .get_wallet()
            .write()
            .map_err(|_| TakerError::General {
                msg: "Failed to acquire wallet write lock".to_string(),
            })?
            .lock_unspendable_utxos()
            .map_err(|error| TakerError::Wallet {
                msg: format!("Lock error: {error}"),
            })?;
        Ok(())
    }

    /// Sends specified Amount of Satoshis to an External Address
    pub fn send_to_address(
        &self,
        address: String,
        amount: i64,
        fee_rate: Option<f64>,
        manually_selected_outpoints: Option<Vec<OutPoint>>,
    ) -> Result<Txid, TakerError> {
        let amount = checked_satoshi_amount(amount)?;
        let manually_selected_outpoints = parse_outpoints(manually_selected_outpoints)?;

        let taker = self.taker.lock().map_err(|_| TakerError::General {
            msg: "Failed to acquire taker lock".to_string(),
        })?;
        let txid = taker
            .get_wallet()
            .write()
            .map_err(|_| TakerError::General {
                msg: "Failed to acquire wallet write lock".to_string(),
            })?
            .send_to_address(amount, address, fee_rate, manually_selected_outpoints)
            .map_err(|error| TakerError::Wallet {
                msg: format!("Send to address error: {error}"),
            })?;
        Ok(txid.into())
    }

    /// Calculates the total balances of different categories in the wallet.
    /// Includes regular, swap, contract, fidelity, and spendable (regular + swap) utxos.
    /// Optionally takes in a list of UTXOs to reduce rpc call. If None is provided,
    /// the full list is fetched from core rpc.
    pub fn get_balances(&self) -> Result<Balances, TakerError> {
        let taker = self.taker.lock().map_err(|_| TakerError::General {
            msg: "Failed to acquire taker lock".to_string(),
        })?;
        let wallet = taker.get_wallet().read().map_err(|_| TakerError::General {
            msg: "Failed to acquire wallet read lock".to_string(),
        })?;
        let balances = wallet.get_balances().map_err(|error| TakerError::Wallet {
            msg: format!("Get balances error: {error}"),
        })?;
        Ok(Balances::from(balances))
    }

    /// Wrapper around Self::sync that also saves the wallet to disk.
    ///
    /// This method first synchronizes the wallet with the Bitcoin Core node,
    /// then persists the wallet state in the disk.
    pub fn sync_and_save(&self) -> Result<(), TakerError> {
        let taker = self.taker.lock().map_err(|_| TakerError::General {
            msg: "Failed to acquire taker lock".to_string(),
        })?;
        taker
            .get_wallet()
            .write()
            .map_err(|_| TakerError::General {
                msg: "Failed to acquire wallet write lock".to_string(),
            })?
            .sync_and_save(&openswap::utill::NO_SHUTDOWN)
            .map_err(|error| TakerError::Wallet {
                msg: format!("Sync wallet error: {error}"),
            })?;
        Ok(())
    }

    /// Runs a full offerbook sync cycle and blocks until it completes.
    pub fn sync_offerbook_and_wait(&self) -> Result<(), TakerError> {
        let taker = self.taker.lock().map_err(|_| TakerError::General {
            msg: "Failed to acquire taker lock".to_string(),
        })?;
        taker
            .sync_offerbook_and_wait()
            .map_err(|e| TakerError::Network {
                msg: format!("Offerbook sync error: {:?}", e),
            })?;
        Ok(())
    }

    /// Polls a single maker, verifies its fidelity proof, stores it in the offerbook, and returns the maker's final state.
    pub fn poll_maker(&self, address: String) -> Result<MakerOfferCandidate, TakerError> {
        let taker = self.taker.lock().map_err(|_| TakerError::General {
            msg: "Failed to acquire taker lock".to_string(),
        })?;
        let candidate = taker.poll_maker(address).map_err(|e| TakerError::Network {
            msg: format!("Poll maker error: {:?}", e),
        })?;
        Ok(MakerOfferCandidate::from(candidate))
    }

    /// Removes a maker from the offerbook by address; returns true if an entry was removed.
    pub fn remove_maker(&self, address: String) -> Result<bool, TakerError> {
        let taker = self.taker.lock().map_err(|_| TakerError::General {
            msg: "Failed to acquire taker lock".to_string(),
        })?;
        taker
            .remove_maker(address)
            .map_err(|e| TakerError::General {
                msg: format!("Remove maker error: {:?}", e),
            })
    }

    /// Returns the OfferBook.
    pub fn fetch_offers(&self) -> Result<OfferBook, TakerError> {
        let taker = self.taker.lock().map_err(|_| TakerError::General {
            msg: "Failed to acquire taker lock".to_string(),
        })?;

        let offerbook = taker.fetch_offers().map_err(|e| TakerError::Network {
            msg: format!("Fetch offers error: {:?}", e),
        })?;

        Ok(OfferBook::from(&offerbook))
    }

    /// Displays a maker offer candidate in a human-readable format.
    /// If the maker does not yet have an offer, a partial view is shown.
    pub fn display_offer(&self, maker_offer: &Offer) -> Result<String, TakerError> {
        format_offer(maker_offer)
    }

    /// Get the wallet name
    pub fn get_wallet_name(&self) -> Result<String, TakerError> {
        let taker = self.taker.lock().map_err(|_| TakerError::General {
            msg: "Failed to acquire taker lock".to_string(),
        })?;
        let wallet = taker.get_wallet().read().map_err(|_| TakerError::General {
            msg: "Failed to acquire wallet read lock".to_string(),
        })?;
        Ok(wallet.get_name().to_string())
    }

    /// Recover from a bad swap
    pub fn recover_active_swap(&self) -> Result<(), TakerError> {
        let mut taker = self.taker.lock().map_err(|_| TakerError::General {
            msg: "Failed to acquire taker lock".to_string(),
        })?;
        taker.recover_active_swap()?;
        Ok(())
    }

    /// Fetch all makers good, bad, and unresponsive
    pub fn fetch_all_makers(&self) -> Result<Vec<String>, TakerError> {
        let taker = self.taker.lock().map_err(|_| TakerError::General {
            msg: "Failed to acquire taker lock".to_string(),
        })?;

        let offerbook = taker.fetch_offers()?;
        let all_makers = offerbook.all_makers();

        let addresses = all_makers
            .into_iter()
            .map(|maker| maker.address.to_string())
            .collect();

        Ok(addresses)
    }

    /// Verifies the deniability proof for a completed swap.
    pub fn verify_deniability(&self, swap_id: String) -> Result<bool, TakerError> {
        let taker = self.taker.lock().map_err(|_| TakerError::General {
            msg: "Failed to acquire taker lock".to_string(),
        })?;

        let is_deniable =
            taker
                .verify_deniability(&swap_id)
                .map_err(|error| TakerError::General {
                    msg: format!("Deniability verification error: {error}"),
                })?;
        Ok(is_deniable)
    }
}

#[cfg(test)]
mod contract_tests {
    use super::{
        SwapParams, checked_satoshi_amount, format_offer, parse_outpoints, parse_protocol,
    };
    use crate::types::{
        Amount, FidelityBond, FidelityProof, LockTime, Offer, OutPoint, PublicKey, TakerError, Txid,
    };
    use openswap::{
        bitcoin::{
            Address as OpenswapAddress, Amount as OpenswapAmount, Txid as OpenswapTxid,
            address::NetworkUnchecked,
        },
        protocol::ProtocolVersion,
    };

    fn error_message(error: TakerError) -> String {
        match error {
            TakerError::General { msg } => msg,
            other => panic!("expected General error, got {other}"),
        }
    }

    fn swap_params(protocol: Option<&str>) -> SwapParams {
        SwapParams {
            protocol: protocol.map(str::to_owned),
            send_amount: 50_000,
            maker_count: 2,
            tx_count: None,
            required_confirms: None,
            manually_selected_outpoints: None,
            preferred_makers: None,
            payment_address: None,
        }
    }

    #[test]
    fn signed_satoshi_amount_rejects_negative_values_and_preserves_valid_values() {
        assert_eq!(
            error_message(checked_satoshi_amount(-1).unwrap_err()),
            "Amount must be non-negative"
        );
        assert_eq!(checked_satoshi_amount(0).unwrap(), 0);
        assert_eq!(checked_satoshi_amount(i64::MAX).unwrap(), i64::MAX as u64);
    }

    #[test]
    fn protocol_domain_accepts_documented_values_and_defaults_to_legacy() {
        assert!(matches!(parse_protocol(None), Ok(ProtocolVersion::Legacy)));
        assert!(matches!(
            parse_protocol(Some("Legacy")),
            Ok(ProtocolVersion::Legacy)
        ));
        assert!(matches!(
            parse_protocol(Some("legacy")),
            Ok(ProtocolVersion::Legacy)
        ));
        assert!(matches!(
            parse_protocol(Some("Taproot")),
            Ok(ProtocolVersion::Taproot)
        ));
        assert!(matches!(
            parse_protocol(Some("taproot")),
            Ok(ProtocolVersion::Taproot)
        ));
        assert_eq!(
            error_message(parse_protocol(Some("Unified")).unwrap_err()),
            "Invalid protocol: Unified (expected legacy or taproot)"
        );
    }

    #[test]
    fn swap_params_apply_defaults_and_preserve_explicit_policy() {
        let defaults = openswap::taker::api::SwapParams::try_from(swap_params(None)).unwrap();
        assert!(matches!(defaults.protocol, ProtocolVersion::Legacy));
        assert_eq!(defaults.send_amount, OpenswapAmount::from_sat(50_000));
        assert_eq!(defaults.maker_count, 2);
        assert_eq!(defaults.tx_count, 1);
        assert_eq!(defaults.required_confirms, 1);
        assert!(defaults.manually_selected_outpoints.is_none());
        assert!(defaults.preferred_makers.is_none());
        assert!(defaults.payment_address.is_none());

        let txid = "0000000000000000000000000000000000000000000000000000000000000001";
        let explicit = openswap::taker::api::SwapParams::try_from(SwapParams {
            protocol: Some("Taproot".to_string()),
            send_amount: u64::MAX,
            maker_count: u32::MAX,
            tx_count: Some(4),
            required_confirms: Some(6),
            manually_selected_outpoints: Some(vec![OutPoint {
                txid: Txid {
                    value: txid.to_string(),
                },
                vout: 7,
            }]),
            preferred_makers: Some(vec!["maker.example:6102".to_string()]),
            payment_address: Some("1BoatSLRHtKNngkdXEeobR76b53LETtpyT".to_string()),
        })
        .unwrap();
        assert!(matches!(explicit.protocol, ProtocolVersion::Taproot));
        assert_eq!(explicit.send_amount, OpenswapAmount::from_sat(u64::MAX));
        assert_eq!(explicit.maker_count, u32::MAX as usize);
        assert_eq!(explicit.tx_count, 4);
        assert_eq!(explicit.required_confirms, 6);
        assert_eq!(
            explicit.manually_selected_outpoints.unwrap()[0].to_string(),
            format!("{txid}:7")
        );
        assert_eq!(explicit.preferred_makers.unwrap(), ["maker.example:6102"]);
        assert_eq!(
            explicit
                .payment_address
                .unwrap()
                .assume_checked()
                .to_string(),
            "1BoatSLRHtKNngkdXEeobR76b53LETtpyT"
        );
    }

    #[test]
    fn outpoint_and_payment_address_validation_fails_before_wallet_access() {
        let invalid_txid = "not-a-txid";
        let txid_error = invalid_txid.parse::<OpenswapTxid>().unwrap_err();
        let error = parse_outpoints(Some(vec![OutPoint {
            txid: Txid {
                value: invalid_txid.to_string(),
            },
            vout: 0,
        }]))
        .unwrap_err();
        assert_eq!(error_message(error), format!("Invalid txid: {txid_error}"));

        let invalid_address = "not-a-bitcoin-address";
        let address_error = invalid_address
            .parse::<OpenswapAddress<NetworkUnchecked>>()
            .unwrap_err();
        let mut params = swap_params(None);
        params.payment_address = Some(invalid_address.to_string());
        assert_eq!(
            error_message(openswap::taker::api::SwapParams::try_from(params).unwrap_err()),
            format!("Invalid payment address '{invalid_address}': {address_error}")
        );
    }

    #[test]
    fn offer_display_is_valid_json_with_only_the_documented_summary_fields() {
        let offer = Offer {
            base_fee: -5,
            amount_relative_fee_pct: 0.125,
            time_relative_fee_pct: 0.25,
            required_confirms: 3,
            minimum_locktime: 48,
            max_size: 2_000_000,
            min_size: 50_000,
            tweakable_point: PublicKey {
                compressed: true,
                inner: vec![2; 33],
            },
            fidelity: FidelityProof {
                bond: FidelityBond {
                    outpoint: OutPoint {
                        txid: Txid {
                            value: "00".repeat(32),
                        },
                        vout: 0,
                    },
                    amount: Amount { sats: 1_000 },
                    lock_time: LockTime {
                        lock_type: "Blocks".to_string(),
                        value: 144,
                    },
                    pubkey: PublicKey {
                        compressed: true,
                        inner: vec![3; 33],
                    },
                    conf_height: Some(100),
                    cert_expiry: Some(200),
                    is_spent: false,
                },
                cert_hash: vec![4; 32],
                cert_sig: vec![5; 64],
            },
        };

        let displayed: serde_json::Value =
            serde_json::from_str(&format_offer(&offer).unwrap()).unwrap();
        assert_eq!(displayed.as_object().unwrap().len(), 7);
        assert_eq!(displayed["base_fee"], -5);
        assert_eq!(displayed["amount_relative_fee_pct"], 0.125);
        assert_eq!(displayed["time_relative_fee_pct"], 0.25);
        assert_eq!(displayed["required_confirms"], 3);
        assert_eq!(displayed["minimum_locktime"], 48);
        assert_eq!(displayed["max_size"], 2_000_000);
        assert_eq!(displayed["min_size"], 50_000);
    }
}
