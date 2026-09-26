//! Shared types for openswap UniFFI bindings
//!
//! This module contains types that are used across multiple modules
//! to avoid duplicate type definitions across language bindings.

use openswap::{
    bitcoin::{
        absolute::LockTime as csLocktime, Address as csAddress, Amount as csAmount,
        OutPoint as openswapOutPoint, PublicKey as csPublicKey, ScriptBuf as csScriptBuf,
        SignedAmount, Txid as csTxid,
    },
    bitcoind::bitcoincore_rpc::Auth,
    protocol::common_messages::{FidelityProof as csFidelityProof, Offer as csOffer},
    taker::{
        error::TakerError as OpenswapTakerError,
        offers::{
            BanReason as csBanReason, BanRecord as csBanRecord, MakerAddress as csMakerAddress,
            MakerOfferCandidate as csMakerOfferCandidate, MakerProtocol as csMakerProtocol,
            MakerState as csMakerState, OfferBook as csOfferBook,
            UnavailableReason as csUnavailableReason, UnavailableState as csUnavailableState,
        },
    },
    wallet::{
        ffi::{
            restore_wallet_gui_app as cs_restore_wallet_gui_app, MakerFeeInfo as csMakerFeeInfo,
            ReportUtxo as csReportUtxo, TakerReport as csTakerReport,
        },
        AddressType as csAddressType, BackendConfig as OpenswapBackendConfig,
        Balances as OpenswapBalances, CoreRpcConfig as OpenswapCoreRpcConfig,
        ElectrumConfig as OpenswapElectrumConfig, FidelityBond as csFidelityBond,
        WalletError as OpenswapWalletError,
    },
};
use std::path::PathBuf;

/// Configuration parameters for connecting to a Bitcoin node via RPC.
#[derive(Debug, Clone, uniffi::Record)]
pub struct RpcConfig {
    /// The bitcoin node url
    pub url: String,
    /// The bitcoin node username
    pub username: String,
    /// The bitcoin node password
    pub password: String,
    /// The wallet name in the bitcoin node, derive this from the descriptor.
    pub wallet_name: String,
}

impl RpcConfig {
    pub fn into_core_rpc_config(self, zmq_addr: String) -> OpenswapCoreRpcConfig {
        OpenswapCoreRpcConfig {
            url: self.url,
            auth: Auth::UserPass(self.username, self.password),
            wallet_name: self.wallet_name,
            zmq_addr,
        }
    }
}

impl From<RpcConfig> for OpenswapCoreRpcConfig {
    fn from(config: RpcConfig) -> Self {
        let default = Self::default();
        Self {
            url: config.url,
            auth: Auth::UserPass(config.username, config.password),
            wallet_name: config.wallet_name,
            zmq_addr: default.zmq_addr,
        }
    }
}

/// Configuration for selecting a blockchain backend.
#[derive(Debug, Clone, uniffi::Record)]
pub struct BackendConfig {
    /// Backend kind: "rpc" or "electrum".
    pub kind: String,
    /// Bitcoin Core RPC URL or Electrum server URL. Required for Electrum.
    pub url: Option<String>,
    /// Bitcoin Core RPC username. Ignored for Electrum.
    pub username: Option<String>,
    /// Bitcoin Core RPC password. Ignored for Electrum.
    pub password: Option<String>,
    /// Bitcoin Core wallet name. Ignored for Electrum.
    pub wallet_name: Option<String>,
    /// Bitcoin Core ZMQ endpoint. Ignored for Electrum.
    pub zmq_addr: Option<String>,
    /// Optional SOCKS5 proxy for Electrum.
    pub socks5: Option<String>,
    /// Optional Electrum socket timeout in seconds.
    pub timeout: Option<u8>,
    /// Optional Electrum notification poll interval in seconds.
    pub poll_interval_secs: Option<u64>,
    /// Electrum reconnect attempts.
    pub max_retries: Option<u8>,
}

impl TryFrom<BackendConfig> for OpenswapBackendConfig {
    type Error = TakerError;

    fn try_from(config: BackendConfig) -> Result<Self, Self::Error> {
        match config.kind.to_lowercase().as_str() {
            "rpc" => config.into_rpc_backend(),
            "electrum" => config.into_electrum_backend(),
            other => Err(TakerError::General {
                msg: format!("Invalid backend kind: {} (expected rpc or electrum)", other),
            }),
        }
    }
}

impl BackendConfig {
    fn into_rpc_backend(self) -> Result<OpenswapBackendConfig, TakerError> {
        let mut config = OpenswapCoreRpcConfig::default();
        apply_if_some(&mut config.url, self.url);
        apply_if_some(&mut config.wallet_name, self.wallet_name);
        apply_if_some(&mut config.zmq_addr, self.zmq_addr);
        config.auth = match (self.username, self.password) {
            (Some(username), Some(password)) => Auth::UserPass(username, password),
            (None, None) => config.auth,
            _ => {
                return Err(TakerError::General {
                    msg: "RPC backend requires username and password together".to_string(),
                });
            }
        };
        Ok(OpenswapBackendConfig::CoreRpc(config))
    }

    fn into_electrum_backend(self) -> Result<OpenswapBackendConfig, TakerError> {
        let mut config = OpenswapElectrumConfig {
            url: self.url.ok_or_else(|| TakerError::General {
                msg: "Electrum backend requires url".to_string(),
            })?,
            ..OpenswapElectrumConfig::default()
        };
        config.socks5 = self.socks5;
        config.timeout = self.timeout;
        config.poll_interval_secs = self.poll_interval_secs;
        apply_if_some(&mut config.max_retries, self.max_retries);
        Ok(OpenswapBackendConfig::Electrum(config))
    }
}

fn apply_if_some<T>(target: &mut T, value: Option<T>) {
    if let Some(value) = value {
        *target = value;
    }
}

/// Represents errors that can occur during Taker operations.
///
/// This enum covers a range of errors related to I/O, wallet operations, network communication,
/// and other Taker-specific scenarios.
#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum TakerError {
    /// Contract transactions appeared on-chain before the swap completed.
    #[error("Contracts broadcasted: {txids:?}")]
    ContractsBroadcasted { txids: Vec<String> },
    /// The offerbook did not contain enough eligible makers.
    #[error("Not enough makers in offerbook: {msg}")]
    NotEnoughMakers { msg: String },
    /// Error related to wallet operations.
    #[error("Wallet error: {msg}")]
    Wallet { msg: String },
    /// Transactions were proven never to have reached the network.
    #[error("Transactions never broadcast: {txids:?}")]
    TransactionsNeverBroadcast { txids: Vec<String> },
    /// Error while screening funding inputs against a blocklist.
    #[error("Blocklist error: {msg}")]
    Blocklist { msg: String },
    /// Protocol error during openswap operations.
    #[error("Protocol error: {msg}")]
    Protocol { msg: String },
    /// Error related to network operations.
    #[error("Network error: {msg}")]
    Network { msg: String },
    /// The send amount required by the prepared swap was missing.
    #[error("Send amount not set: {msg}")]
    SendAmountNotSet { msg: String },
    /// Serialized taker data could not be decoded.
    #[error("Deserialize error: {msg}")]
    Deserialize { msg: String },
    /// Internal taker channel communication failed.
    #[error("MPSC error: {msg}")]
    Mpsc { msg: String },
    /// Tor setup or communication failed.
    #[error("Tor error: {msg}")]
    Tor { msg: String },
    /// A Bitcoin address could not be parsed.
    #[error("Address parse error: {msg}")]
    AddressParse { msg: String },
    /// The breach watcher failed.
    #[error("Watcher error: {msg}")]
    Watcher { msg: String },
    /// General error with a custom message
    #[error("General error: {msg}")]
    General { msg: String },
    /// Standard input/output error.
    #[error("IO error: {msg}")]
    IO { msg: String },
}

impl From<OpenswapTakerError> for TakerError {
    fn from(error: OpenswapTakerError) -> Self {
        match error {
            OpenswapTakerError::ContractsBroadcasted(txids) => TakerError::ContractsBroadcasted {
                txids: txids.into_iter().map(|txid| txid.to_string()).collect(),
            },
            OpenswapTakerError::NotEnoughMakersInOfferBook => TakerError::NotEnoughMakers {
                msg: "not enough eligible makers".to_string(),
            },
            OpenswapTakerError::Wallet(OpenswapWalletError::TxNeverBroadcast(txids)) => {
                TakerError::TransactionsNeverBroadcast {
                    txids: txids.into_iter().map(|txid| txid.to_string()).collect(),
                }
            }
            OpenswapTakerError::Wallet(error) => TakerError::Wallet {
                msg: error.to_string(),
            },
            OpenswapTakerError::Blocklist(error) => TakerError::Blocklist {
                msg: error.to_string(),
            },
            OpenswapTakerError::General(msg) => TakerError::General { msg },
            OpenswapTakerError::IO(error) => TakerError::IO {
                msg: error.to_string(),
            },
            OpenswapTakerError::Net(error) => TakerError::Network {
                msg: error.to_string(),
            },
            OpenswapTakerError::SendAmountNotSet => TakerError::SendAmountNotSet {
                msg: "prepared swap has no send amount".to_string(),
            },
            OpenswapTakerError::Deserialize(msg) => TakerError::Deserialize { msg },
            OpenswapTakerError::MPSC(msg) => TakerError::Mpsc { msg },
            OpenswapTakerError::TorError(error) => TakerError::Tor {
                msg: format!("{error:?}"),
            },
            OpenswapTakerError::AddressParseError(error) => TakerError::AddressParse {
                msg: error.to_string(),
            },
            OpenswapTakerError::Watcher(error) => TakerError::Watcher {
                msg: error.to_string(),
            },
        }
    }
}

/// Represents different behaviors taker can have during the swap.
/// Used for testing various possible scenarios that can happen during a swap.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum TakerBehavior {
    /// Normal behaviour
    Normal,
    /// This depicts the behavior when the taker drops connections after the full openswap setup.
    DropConnectionAfterFullSetup,
    /// Behavior to broadcast the contract after the full openswap setup.
    BroadcastContractAfterFullSetup,
}

/// Represents total wallet balances of different categories.
#[derive(Debug, uniffi::Record)]
pub struct Balances {
    /// All single signature regular wallet coins (seed balance).
    pub regular: i64,
    /// All 2of2 multisig coins received in swaps.
    pub swap: i64,
    /// All live contract transaction balance locked in timelocks.
    pub contract: i64,
    /// All coins locked in fidelity bonds.
    pub fidelity: i64,
    /// Spendable amount in wallet (regular + swap balance).
    pub spendable: i64,
}

impl From<OpenswapBalances> for Balances {
    fn from(balances: OpenswapBalances) -> Self {
        Self {
            regular: balances.regular.to_sat() as i64,
            swap: balances.swap.to_sat() as i64,
            contract: balances.contract.to_sat() as i64,
            fidelity: balances.fidelity.to_sat() as i64,
            spendable: balances.spendable.to_sat() as i64,
        }
    }
}

#[derive(Clone, Debug, uniffi::Record)]
pub struct OutPoint {
    pub txid: Txid,
    pub vout: u32,
}

impl From<openswapOutPoint> for OutPoint {
    fn from(value: openswapOutPoint) -> Self {
        Self {
            txid: value.txid.into(),
            vout: value.vout,
        }
    }
}

#[derive(Clone, uniffi::Record)]
pub struct Address {
    pub addr: String,
}

impl From<csAddress> for Address {
    fn from(addr: csAddress) -> Self {
        Self {
            addr: addr.to_string(),
        }
    }
}

#[derive(Clone, uniffi::Record)]
pub struct ListTransactionResult {
    pub info: WalletTxInfo,
    pub detail: GetTransactionResultDetail,
    pub trusted: Option<bool>,
    pub comment: Option<String>,
}

#[derive(Clone, uniffi::Record)]
pub struct WalletTxInfo {
    pub confirmations: i32,
    pub blockhash: Option<String>,
    pub blockindex: Option<u32>,
    pub blocktime: Option<i64>,
    pub blockheight: Option<u32>,
    pub txid: Txid,
    pub time: i64,
    pub timereceived: i64,
    pub bip125_replaceable: String,
    pub wallet_conflicts: Vec<Txid>,
}

#[derive(Clone, uniffi::Record)]
pub struct GetTransactionResultDetail {
    pub address: Option<Address>,
    pub category: String,
    pub amount: SignedAmountSats,
    pub label: Option<String>,
    pub vout: u32,
    pub fee: Option<SignedAmountSats>,
    pub abandoned: Option<bool>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct Amount {
    pub sats: i64,
}

impl From<csAmount> for Amount {
    fn from(amount: csAmount) -> Self {
        Self {
            sats: amount.to_sat() as i64,
        }
    }
}

#[derive(Clone, Debug, uniffi::Record)]
pub struct Txid {
    pub value: String,
}

impl From<csTxid> for Txid {
    fn from(txid: csTxid) -> Self {
        Self {
            value: txid.to_string(),
        }
    }
}

#[derive(Clone, uniffi::Record)]
pub struct ScriptBuf {
    pub hex: String,
}

impl From<csScriptBuf> for ScriptBuf {
    fn from(script: csScriptBuf) -> Self {
        Self {
            hex: hex::encode(script.as_bytes()),
        }
    }
}

#[derive(Clone, uniffi::Record)]
pub struct SignedAmountSats {
    pub sats: i64,
}

impl From<SignedAmount> for SignedAmountSats {
    fn from(amount: SignedAmount) -> Self {
        Self {
            sats: amount.to_sat(),
        }
    }
}

#[derive(Clone, uniffi::Record)]
pub struct ListUnspentResultEntry {
    pub txid: Txid,
    pub vout: u32,
    pub address: Option<String>,
    pub label: Option<String>,
    pub script_pub_key: ScriptBuf,
    pub amount: Amount,
    pub confirmations: u32,
    pub redeem_script: Option<ScriptBuf>,
    pub witness_script: Option<ScriptBuf>,
    pub spendable: bool,
    pub solvable: bool,
    pub desc: Option<String>,
    pub safe: bool,
}

#[derive(Clone, uniffi::Record)]
pub struct UtxoSpendInfo {
    pub spend_type: String,
    pub path: Option<String>,
    pub multisig_redeemscript: Option<ScriptBuf>,
    pub input_value: Option<Amount>,
    pub index: Option<u32>,
}

#[derive(uniffi::Record)]
pub struct TotalUtxoInfo {
    pub list_unspent_result_entry: ListUnspentResultEntry,
    pub utxo_spend_info: UtxoSpendInfo,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct LockTime {
    pub lock_type: String,
    pub value: u32,
}

impl From<csLocktime> for LockTime {
    fn from(locktime: csLocktime) -> Self {
        match locktime {
            csLocktime::Blocks(height) => LockTime {
                lock_type: "Blocks".to_string(),
                value: height.to_consensus_u32(),
            },
            csLocktime::Seconds(time) => LockTime {
                lock_type: "Seconds".to_string(),
                value: time.to_consensus_u32(),
            },
        }
    }
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct PublicKey {
    pub compressed: bool,
    pub inner: Vec<u8>,
}

impl From<csPublicKey> for PublicKey {
    fn from(publickey: csPublicKey) -> Self {
        Self {
            compressed: publickey.compressed,
            inner: publickey.inner.serialize().to_vec(),
        }
    }
}

/// Contains proof data related to fidelity bond.
#[derive(Debug, Clone, uniffi::Record)]
pub struct FidelityProof {
    /// Details for Fidelity Bond
    pub bond: FidelityBond,
    /// Double SHA256 hash of certificate message proving bond ownership and binding to maker address
    pub cert_hash: Vec<u8>,
    /// ECDSA signature over cert_hash using the bond's private key
    pub cert_sig: Vec<u8>,
}

impl From<csFidelityProof> for FidelityProof {
    fn from(fidelityproof: csFidelityProof) -> Self {
        Self {
            bond: fidelityproof.bond.into(),
            cert_hash: <_ as AsRef<[u8]>>::as_ref(&fidelityproof.cert_hash).to_vec(),
            cert_sig: fidelityproof.cert_sig.serialize_compact().to_vec(),
        }
    }
}

#[derive(Clone, Debug, uniffi::Record)]
pub struct FidelityBond {
    pub outpoint: OutPoint,
    pub amount: Amount,
    pub lock_time: LockTime,
    pub pubkey: PublicKey,
    pub conf_height: Option<u32>,
    pub cert_expiry: Option<u32>,
    pub is_spent: bool,
}

impl From<csFidelityBond> for FidelityBond {
    fn from(bond: csFidelityBond) -> Self {
        Self {
            outpoint: OutPoint {
                txid: bond.outpoint().txid.into(),
                vout: bond.outpoint().vout,
            },
            amount: Amount::from(bond.amount),
            lock_time: LockTime::from(bond.lock_time),
            pubkey: PublicKey {
                compressed: true,
                inner: vec![],
            },
            conf_height: None,
            cert_expiry: None,
            is_spent: bond.is_spent(),
        }
    }
}

/// Represents an offer in the context of the Openswap protocol.
#[derive(Debug, Clone, uniffi::Record)]
pub struct Offer {
    /// Base fee charged per swap in satoshis (fixed cost component)
    pub base_fee: i64,
    /// Percentage fee relative to swap amount
    pub amount_relative_fee_pct: f64,
    /// Percentage fee for time-locked funds
    pub time_relative_fee_pct: f64,
    /// Minimum confirmations required before proceeding with swap
    pub required_confirms: u32,
    /// Minimum timelock duration in blocks for contract transactions
    pub minimum_locktime: u16,
    /// Maximum swap amount accepted in sats
    pub max_size: i64,
    /// Minimum swap amount accepted in sats
    pub min_size: i64,
    /// Displayed public key of makers, for receiving swaps.
    /// Actual swap addresses are derived from this public key using unique nonces per swap.
    pub tweakable_point: PublicKey,
    /// Cryptographic proof of fidelity bond for Sybil resistance
    pub fidelity: FidelityProof,
}

impl From<csOffer> for Offer {
    fn from(offer: csOffer) -> Self {
        Self {
            base_fee: offer.base_fee as i64,
            amount_relative_fee_pct: offer.amount_relative_fee_pct,
            time_relative_fee_pct: offer.time_relative_fee_pct,
            required_confirms: offer.required_confirms,
            minimum_locktime: offer.minimum_locktime,
            max_size: offer.max_size as i64,
            min_size: offer.min_size as i64,
            tweakable_point: offer.tweakable_point.into(),
            fidelity: offer.fidelity.into(),
        }
    }
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct MakerAddress {
    pub address: String,
}

impl From<csMakerAddress> for MakerAddress {
    fn from(addr: csMakerAddress) -> Self {
        Self {
            address: addr.to_string(),
        }
    }
}

/// Represents the Maker connection state
#[derive(Debug, Clone, uniffi::Record)]
pub struct MakerState {
    /// State type: "Good", "Unavailable", or "Banned".
    pub state_type: String,
    /// Recovery details when state_type is "Unavailable".
    pub unavailable: Option<UnavailableState>,
    /// Permanent ban details when state_type is "Banned".
    pub ban: Option<BanRecord>,
}

/// Details for a maker that is temporarily unavailable.
#[derive(Debug, Clone, uniffi::Record)]
pub struct UnavailableState {
    /// One of the documented UnavailableReason variant names.
    pub reason: String,
    /// Start of the unbroken failure run, as Unix seconds.
    pub since_ts: Option<u64>,
    /// Most recent attempt, as Unix seconds.
    pub last_attempt_ts: Option<u64>,
    /// Number of failures in the current run.
    pub attempts: u32,
}

impl From<csUnavailableState> for UnavailableState {
    fn from(state: csUnavailableState) -> Self {
        Self {
            reason: match state.reason {
                csUnavailableReason::AwaitingOffer => "AwaitingOffer",
                csUnavailableReason::NoOfferResponse => "NoOfferResponse",
                csUnavailableReason::BondUnconfirmed => "BondUnconfirmed",
                csUnavailableReason::BondExpired => "BondExpired",
                csUnavailableReason::BondReorged => "BondReorged",
                csUnavailableReason::BondUnverified => "BondUnverified",
                csUnavailableReason::UnpriceableOffer => "UnpriceableOffer",
                csUnavailableReason::LegacyStatus => "LegacyStatus",
            }
            .to_string(),
            since_ts: state.since_ts,
            last_attempt_ts: state.last_attempt_ts,
            attempts: state.attempts,
        }
    }
}

/// Details for a maker that is permanently banned until explicitly removed.
#[derive(Debug, Clone, uniffi::Record)]
pub struct BanRecord {
    /// One of the documented BanReason variant names.
    pub reason: String,
    /// Time the ban was first recorded, as Unix seconds.
    pub recorded_at_ts: u64,
}

impl From<csBanRecord> for BanRecord {
    fn from(record: csBanRecord) -> Self {
        Self {
            reason: match record.reason {
                csBanReason::ProvenViolation => "ProvenViolation",
                csBanReason::InvalidFidelityProof => "InvalidFidelityProof",
                csBanReason::FundingWithheld => "FundingWithheld",
                csBanReason::LegacyProvenViolation => "LegacyProvenViolation",
            }
            .to_string(),
            recorded_at_ts: record.recorded_at_ts,
        }
    }
}

impl From<csMakerState> for MakerState {
    fn from(state: csMakerState) -> Self {
        match state {
            csMakerState::Good => MakerState {
                state_type: "Good".to_string(),
                unavailable: None,
                ban: None,
            },
            csMakerState::Unavailable(unavailable) => MakerState {
                state_type: "Unavailable".to_string(),
                unavailable: Some(unavailable.into()),
                ban: None,
            },
            csMakerState::Banned(ban) => MakerState {
                state_type: "Banned".to_string(),
                unavailable: None,
                ban: Some(ban.into()),
            },
        }
    }
}

/// Protocol which maker follows
#[derive(Debug, Clone, uniffi::Record)]
pub struct MakerProtocol {
    /// Maker capability: "Legacy", "Taproot", or "Unified".
    pub protocol_type: String,
}

impl From<csMakerProtocol> for MakerProtocol {
    fn from(protocol: csMakerProtocol) -> Self {
        match protocol {
            csMakerProtocol::Legacy => MakerProtocol {
                protocol_type: "Legacy".to_string(),
            },
            csMakerProtocol::Taproot => MakerProtocol {
                protocol_type: "Taproot".to_string(),
            },
            csMakerProtocol::Unified => MakerProtocol {
                protocol_type: "Unified".to_string(),
            },
        }
    }
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct AddressType {
    /// P2WPKH or P2TR
    pub addr_type: String,
}

impl TryFrom<AddressType> for csAddressType {
    type Error = TakerError;

    fn try_from(addr: AddressType) -> Result<Self, Self::Error> {
        match addr.addr_type.as_str() {
            "P2TR" => Ok(csAddressType::P2TR),
            "P2WPKH" => Ok(csAddressType::P2WPKH),
            _ => Err(TakerError::General {
                msg: format!(
                    "Invalid address type: {} (expected P2WPKH or P2TR)",
                    addr.addr_type
                ),
            }),
        }
    }
}

/// Canonical maker record.
/// A maker may or may not currently have an offer.
#[derive(Debug, Clone, uniffi::Record)]
pub struct MakerOfferCandidate {
    /// Maker Address: onion_addr:port
    pub address: MakerAddress,
    /// Latest offer, if successfully fetched
    pub offer: Option<Offer>,
    /// Current state of maker
    pub state: MakerState,
    /// Supporting protocol (Legacy or Taproot), if known
    pub protocol: Option<MakerProtocol>,
}

impl From<csMakerOfferCandidate> for MakerOfferCandidate {
    fn from(maker: csMakerOfferCandidate) -> Self {
        Self {
            address: maker.address.into(),
            offer: maker.offer.map(Offer::from),
            state: maker.state.into(),
            protocol: maker.protocol.map(|p| p.into()),
        }
    }
}

/// Contains all maker offers in the network
#[derive(Debug, Clone, uniffi::Record)]
pub struct OfferBook {
    /// All makers in the offerbook (good, bad, and unresponsive)
    pub makers: Vec<MakerOfferCandidate>,
}

impl From<&csOfferBook> for OfferBook {
    fn from(offerbook: &csOfferBook) -> Self {
        Self {
            makers: offerbook
                .all_makers()
                .into_iter()
                .map(MakerOfferCandidate::from)
                .collect(),
        }
    }
}

/// Information about individual maker fees in a swap
#[derive(Debug, Clone, uniffi::Record)]
pub struct MakerFeeInfo {
    /// Index of maker in the swap route
    pub maker_index: u32,
    /// Maker Addresses (Onion:Port)
    pub maker_address: String,
    /// The fixed Base Fee for each maker
    pub base_fee: f64,
    /// Dynamic Amount Fee for each maker
    pub amount_relative_fee: f64,
    /// Dynamic Time Fee(Decreases for subsequent makers) for each maker
    pub time_relative_fee: f64,
    /// All inclusive fee for each maker
    pub total_fee: f64,
}

impl From<csMakerFeeInfo> for MakerFeeInfo {
    fn from(info: csMakerFeeInfo) -> Self {
        Self {
            maker_index: info.maker_index as u32,
            maker_address: info.maker_address,
            base_fee: info.base_fee,
            amount_relative_fee: info.amount_relative_fee,
            time_relative_fee: info.time_relative_fee,
            total_fee: info.total_fee,
        }
    }
}

/// Complete swap report containing all swap information
#[derive(Debug, Clone, uniffi::Record)]
pub struct SwapReport {
    /// Unique swap ID
    pub swap_id: String,
    /// Role of report creator (Taker/Maker)
    pub role: String,
    /// Swap status (Success/Failed/RecoveryHashlock/RecoveryTimelock)
    pub status: String,
    /// Duration of the swap in seconds
    pub swap_duration_seconds: f64,
    /// Unix start timestamp
    pub start_timestamp: i64,
    /// Unix end timestamp
    pub end_timestamp: i64,
    /// Bitcoin network
    pub network: String,
    /// Error message if any
    pub error_message: Option<String>,
    /// Incoming amount in sats
    pub incoming_amount: i64,
    /// Outgoing amount in sats
    pub outgoing_amount: i64,
    /// Fee paid (negative)
    pub fee_paid: i64,
    /// Wallet UTXOs spent to fund the outgoing swap
    pub outgoing_utxos: Vec<ReportUtxo>,
    /// Wallet UTXOs created by sweeping the incoming swapcoins
    pub incoming_utxos: Vec<ReportUtxo>,
    /// Funding transaction IDs organized by hops
    pub funding_txids: Vec<Vec<String>>,
    /// Number of makers involved
    pub makers_count: Option<u32>,
    /// List of maker addresses used
    pub maker_addresses: Vec<String>,
    /// Total maker fees
    pub total_maker_fees: i64,
    /// Mining fees
    pub mining_fee: i64,
    /// Fee percentage relative to target amount
    pub fee_percentage: f64,
    /// Individual maker fee information
    pub maker_fee_info: Vec<MakerFeeInfo>,
    /// Input UTXOs amounts
    pub input_utxos: Vec<i64>,
    /// Output change UTXOs amounts
    pub output_change_amounts: Vec<i64>,
    /// Output swap coin UTXOs amounts
    pub output_swap_amounts: Vec<i64>,
    /// Output change coin UTXOs with amounts and addresses (amount, address)
    pub output_change_utxos: Vec<UtxoWithAddress>,
    /// Output swap coin UTXOs with amounts and addresses (amount, address)
    pub output_swap_utxos: Vec<UtxoWithAddress>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct UtxoWithAddress {
    pub amount: i64,
    pub address: String,
}

/// User-facing UTXO information recorded in a swap report.
#[derive(Debug, Clone, uniffi::Record)]
pub struct ReportUtxo {
    /// Address locking the reported output
    pub address: String,
    /// Output value in satoshis
    pub value: i64,
}

impl From<csReportUtxo> for ReportUtxo {
    fn from(utxo: csReportUtxo) -> Self {
        Self {
            address: utxo.address,
            value: utxo.value as i64,
        }
    }
}

impl From<csTakerReport> for SwapReport {
    fn from(report: csTakerReport) -> Self {
        let output_change_amounts: Vec<i64> = report
            .output_change_amounts
            .into_iter()
            .map(|v| v as i64)
            .collect();
        let output_swap_amounts: Vec<i64> = report
            .output_swap_amounts
            .into_iter()
            .map(|v| v as i64)
            .collect();
        let total_output_amount: i64 =
            output_change_amounts.iter().sum::<i64>() + output_swap_amounts.iter().sum::<i64>();
        let incoming_amount = if total_output_amount > 0 {
            total_output_amount
        } else {
            report.incoming_amount as i64
        };

        Self {
            swap_id: report.swap_id,
            role: "Taker".to_string(),
            status: report.status.to_string(),
            swap_duration_seconds: report.swap_duration_seconds,
            start_timestamp: report.start_timestamp as i64,
            end_timestamp: report.end_timestamp as i64,
            network: report.network.to_string(),
            error_message: report.error_message,
            incoming_amount,
            outgoing_amount: report.outgoing_amount as i64,
            fee_paid: -(report.fee_paid as i64),
            outgoing_utxos: report
                .outgoing_utxos
                .into_iter()
                .map(ReportUtxo::from)
                .collect(),
            incoming_utxos: report
                .incoming_utxos
                .into_iter()
                .map(ReportUtxo::from)
                .collect(),
            funding_txids: report.funding_txids,
            makers_count: Some(report.makers_count as u32),
            maker_addresses: report.maker_addresses,
            total_maker_fees: report.total_maker_fees as i64,
            mining_fee: report.mining_fee as i64,
            fee_percentage: report.fee_percentage,
            maker_fee_info: report
                .maker_fee_info
                .into_iter()
                .map(MakerFeeInfo::from)
                .collect(),
            input_utxos: report.input_utxos.into_iter().map(|v| v as i64).collect(),
            output_change_amounts,
            output_swap_amounts,
            output_change_utxos: report
                .output_change_utxos
                .into_iter()
                .map(|(amount, address)| UtxoWithAddress {
                    amount: amount as i64,
                    address,
                })
                .collect(),
            output_swap_utxos: report
                .output_swap_utxos
                .into_iter()
                .map(|(amount, address)| UtxoWithAddress {
                    amount: amount as i64,
                    address,
                })
                .collect(),
        }
    }
}

/// Restores a wallet from an encrypted or unencrypted JSON backup file for GUI/FFI applications.
///
/// This is a non-interactive restore method designed for programmatic use via FFI bindings.
/// Unlike `restore_wallet`, this function accepts a path to a JSON backup file and handles both
/// encrypted and unencrypted backups using [`load_sensitive_struct_from_value`].
///
/// # Behavior
///
/// 1. Reads and parses the JSON backup file into a [`WalletBackup`] structure
/// 2. If encrypted, decrypts using the provided password and preserves encryption material
/// 3. Constructs the wallet path: `{data_dir_or_default}/wallets/{wallet_file_name_or_default}`
/// 4. Calls [`Wallet::restore`] to reconstruct the wallet with all UTXOs and metadata
///
/// # Parameters
///
/// - `data_dir`: Target directory, defaults to `~/.openswap/taker`
/// - `wallet_file_name`: Restored wallet filename, defaults to name from backup if empty
/// - `backup_file_path`: Path to the JSON file containing the wallet backup (encrypted or plain)
/// - `password`: Required if backup is encrypted, ignored otherwise
#[uniffi::export]
pub fn restore_wallet_gui_app(
    data_dir: Option<String>,
    wallet_file_name: Option<String>,
    rpc_config: RpcConfig,
    backup_file_path: String,
    password: Option<String>,
) {
    let data_dir = data_dir.map(PathBuf::from);

    cs_restore_wallet_gui_app(
        data_dir,
        wallet_file_name,
        OpenswapBackendConfig::CoreRpc(rpc_config.into()),
        backup_file_path.into(),
        password,
    );
}

/// Checks whether wallet is encrypted or not.
#[uniffi::export]
pub fn is_wallet_encrypted(wallet_path: String) -> Result<bool, TakerError> {
    let path = PathBuf::from(wallet_path);

    openswap::wallet::Wallet::is_wallet_encrypted(&path).map_err(|error| TakerError::Wallet {
        msg: format!("Failed to check wallet encryption: {error}"),
    })
}

#[uniffi::export]
pub fn create_default_rpc_config() -> RpcConfig {
    RpcConfig {
        url: "http://127.0.0.1:38332".to_string(),
        username: "user".to_string(),
        password: "password".to_string(),
        wallet_name: "openswap_wallet".to_string(),
    }
}

/// Sets up the logger for the taker component.
///
/// This method initializes the logging configuration for the taker, directing logs to both
/// the console and a file. It sets the `RUST_LOG` environment variable to provide default
/// log levels and configures log4rs with the specified filter level for fine-grained control
/// of log verbosity.
#[uniffi::export]
pub fn setup_logging(
    data_dir: Option<String>,
    level: String,
    to_stdout: bool,
) -> Result<(), TakerError> {
    let path = data_dir.map(PathBuf::from);
    let level = match level.to_lowercase().as_str() {
        "trace" => log::LevelFilter::Trace,
        "debug" => log::LevelFilter::Debug,
        "info" => log::LevelFilter::Info,
        "warn" => log::LevelFilter::Warn,
        "error" => log::LevelFilter::Error,
        "off" => log::LevelFilter::Off,
        _ => log::LevelFilter::Info,
    };
    openswap::utill::setup_taker_logger(level, to_stdout, path);
    Ok(())
}

#[cfg(test)]
mod contract_tests {
    use super::*;
    use openswap::{
        bitcoin::{
            absolute::{Height, LockTime as OpenswapLockTime, Time},
            Amount as OpenswapAmount, ScriptBuf as OpenswapScriptBuf,
            SignedAmount as OpenswapSignedAmount, Txid as OpenswapTxid,
        },
        error::NetError,
        taker::error::TakerError as OpenswapTakerError,
        taker::offers::{
            BanReason, BanRecord, MakerProtocol as OpenswapMakerProtocol,
            MakerState as OpenswapMakerState, UnavailableReason, UnavailableState,
        },
        wallet::{
            AddressType as OpenswapAddressType, BackendConfig as OpenswapBackendConfig,
            WalletError as OpenswapWalletError,
        },
    };
    use std::str::FromStr;

    fn backend_config(kind: &str) -> BackendConfig {
        BackendConfig {
            kind: kind.to_string(),
            url: None,
            username: None,
            password: None,
            wallet_name: None,
            zmq_addr: None,
            socks5: None,
            timeout: None,
            poll_interval_secs: None,
            max_retries: None,
        }
    }

    fn general_message(error: TakerError) -> String {
        match error {
            TakerError::General { msg } => msg,
            other => panic!("expected General error, got {other}"),
        }
    }

    #[test]
    fn report_utxo_preserves_address_and_value() {
        let utxo = ReportUtxo::from(csReportUtxo {
            address: "bc1qreport".to_string(),
            value: 42_000,
        });

        assert_eq!(utxo.address, "bc1qreport");
        assert_eq!(utxo.value, 42_000);
    }

    #[test]
    fn default_rpc_config_is_stable_and_each_call_returns_independent_data() {
        let mut first = create_default_rpc_config();
        let second = create_default_rpc_config();

        assert_eq!(first.url, "http://127.0.0.1:38332");
        assert_eq!(first.username, "user");
        assert_eq!(first.password, "password");
        assert_eq!(first.wallet_name, "openswap_wallet");

        first.wallet_name.push_str("-changed");
        assert_eq!(second.wallet_name, "openswap_wallet");
    }

    #[test]
    fn rpc_backend_accepts_case_insensitive_kind_and_preserves_overrides() {
        let converted = OpenswapBackendConfig::try_from(BackendConfig {
            kind: "RPC".to_string(),
            url: Some("http://node.internal:18443".to_string()),
            username: Some("alice".to_string()),
            password: Some("secret".to_string()),
            wallet_name: Some("wallet-a".to_string()),
            zmq_addr: Some("tcp://node.internal:28332".to_string()),
            socks5: Some("ignored".to_string()),
            timeout: Some(9),
            poll_interval_secs: Some(10),
            max_retries: Some(11),
        })
        .unwrap();

        let OpenswapBackendConfig::CoreRpc(config) = converted else {
            panic!("RPC kind must select the Core RPC backend");
        };
        assert_eq!(config.url, "http://node.internal:18443");
        assert_eq!(config.wallet_name, "wallet-a");
        assert_eq!(config.zmq_addr, "tcp://node.internal:28332");
        match config.auth {
            Auth::UserPass(username, password) => {
                assert_eq!(username, "alice");
                assert_eq!(password, "secret");
            }
            other => panic!("expected username/password authentication, got {other:?}"),
        }
    }

    #[test]
    fn rpc_backend_requires_username_and_password_as_a_pair() {
        for (username, password) in [(Some("user"), None), (None, Some("password"))] {
            let mut config = backend_config("rpc");
            config.username = username.map(str::to_string);
            config.password = password.map(str::to_string);
            assert_eq!(
                general_message(OpenswapBackendConfig::try_from(config).unwrap_err()),
                "RPC backend requires username and password together"
            );
        }
    }

    #[test]
    fn electrum_backend_requires_url_and_preserves_transport_options() {
        assert_eq!(
            general_message(
                OpenswapBackendConfig::try_from(backend_config("electrum")).unwrap_err()
            ),
            "Electrum backend requires url"
        );

        let converted = OpenswapBackendConfig::try_from(BackendConfig {
            kind: "ElEcTrUm".to_string(),
            url: Some("ssl://electrum.example:50002".to_string()),
            username: Some("ignored".to_string()),
            password: Some("ignored".to_string()),
            wallet_name: Some("ignored".to_string()),
            zmq_addr: Some("ignored".to_string()),
            socks5: Some("127.0.0.1:9050".to_string()),
            timeout: Some(120),
            poll_interval_secs: Some(15),
            max_retries: Some(8),
        })
        .unwrap();

        let OpenswapBackendConfig::Electrum(config) = converted else {
            panic!("Electrum kind must select the Electrum backend");
        };
        assert_eq!(config.url, "ssl://electrum.example:50002");
        assert_eq!(config.socks5.as_deref(), Some("127.0.0.1:9050"));
        assert_eq!(config.timeout, Some(120));
        assert_eq!(config.poll_interval_secs, Some(15));
        assert_eq!(config.max_retries, 8);
    }

    #[test]
    fn backend_kind_rejects_unknown_values_with_the_normalized_value() {
        assert_eq!(
            general_message(
                OpenswapBackendConfig::try_from(backend_config("Unknown")).unwrap_err()
            ),
            "Invalid backend kind: unknown (expected rpc or electrum)"
        );
    }

    #[test]
    fn address_type_domain_is_exact_and_case_sensitive() {
        assert!(matches!(
            OpenswapAddressType::try_from(AddressType {
                addr_type: "P2WPKH".to_string()
            }),
            Ok(OpenswapAddressType::P2WPKH)
        ));
        assert!(matches!(
            OpenswapAddressType::try_from(AddressType {
                addr_type: "P2TR".to_string()
            }),
            Ok(OpenswapAddressType::P2TR)
        ));
        assert_eq!(
            general_message(
                OpenswapAddressType::try_from(AddressType {
                    addr_type: "p2tr".to_string()
                })
                .unwrap_err()
            ),
            "Invalid address type: p2tr (expected P2WPKH or P2TR)"
        );
    }

    #[test]
    fn maker_state_and_protocol_variants_keep_their_ffi_discriminants() {
        let good = MakerState::from(OpenswapMakerState::Good);
        assert_eq!(good.state_type, "Good");
        assert!(good.unavailable.is_none());
        assert!(good.ban.is_none());

        let unavailable = MakerState::from(OpenswapMakerState::Unavailable(UnavailableState {
            reason: UnavailableReason::NoOfferResponse,
            since_ts: Some(100),
            last_attempt_ts: Some(200),
            attempts: 7,
        }));
        assert_eq!(unavailable.state_type, "Unavailable");
        let details = unavailable.unavailable.expect("unavailable details");
        assert_eq!(details.reason, "NoOfferResponse");
        assert_eq!(details.since_ts, Some(100));
        assert_eq!(details.last_attempt_ts, Some(200));
        assert_eq!(details.attempts, 7);
        assert!(unavailable.ban.is_none());

        let banned = MakerState::from(OpenswapMakerState::Banned(BanRecord {
            reason: BanReason::ProvenViolation,
            recorded_at_ts: 300,
        }));
        assert_eq!(banned.state_type, "Banned");
        assert!(banned.unavailable.is_none());
        let ban = banned.ban.expect("ban details");
        assert_eq!(ban.reason, "ProvenViolation");
        assert_eq!(ban.recorded_at_ts, 300);

        for (input, expected) in [
            (OpenswapMakerProtocol::Legacy, "Legacy"),
            (OpenswapMakerProtocol::Taproot, "Taproot"),
            (OpenswapMakerProtocol::Unified, "Unified"),
        ] {
            assert_eq!(MakerProtocol::from(input).protocol_type, expected);
        }
    }

    #[test]
    fn primitive_bitcoin_values_cross_the_contract_without_reformatting() {
        assert_eq!(
            Amount::from(OpenswapAmount::from_sat(21_000_000)).sats,
            21_000_000
        );
        assert_eq!(
            SignedAmountSats::from(OpenswapSignedAmount::from_sat(-42)).sats,
            -42
        );

        let script = OpenswapScriptBuf::from_bytes(vec![0x00, 0x51, 0xff]);
        assert_eq!(ScriptBuf::from(script).hex, "0051ff");

        let txid = OpenswapTxid::from_str(&"ab".repeat(32)).unwrap();
        assert_eq!(Txid::from(txid).value, "ab".repeat(32));
    }

    #[test]
    fn lock_time_conversion_distinguishes_block_height_from_timestamp() {
        let blocks = LockTime::from(OpenswapLockTime::Blocks(
            Height::from_consensus(144).unwrap(),
        ));
        assert_eq!(blocks.lock_type, "Blocks");
        assert_eq!(blocks.value, 144);

        let seconds = LockTime::from(OpenswapLockTime::Seconds(
            Time::from_consensus(500_000_000).unwrap(),
        ));
        assert_eq!(seconds.lock_type, "Seconds");
        assert_eq!(seconds.value, 500_000_000);
    }

    #[test]
    fn taker_error_variants_preserve_their_category_and_message() {
        let errors = [
            (
                TakerError::ContractsBroadcasted {
                    txids: vec!["contract".into()],
                },
                "Contracts broadcasted: [\"contract\"]",
            ),
            (
                TakerError::NotEnoughMakers {
                    msg: "makers".into(),
                },
                "Not enough makers in offerbook: makers",
            ),
            (
                TakerError::Wallet {
                    msg: "wallet".into(),
                },
                "Wallet error: wallet",
            ),
            (
                TakerError::TransactionsNeverBroadcast {
                    txids: vec!["withheld".into()],
                },
                "Transactions never broadcast: [\"withheld\"]",
            ),
            (
                TakerError::Blocklist {
                    msg: "blocked".into(),
                },
                "Blocklist error: blocked",
            ),
            (
                TakerError::Protocol {
                    msg: "protocol".into(),
                },
                "Protocol error: protocol",
            ),
            (
                TakerError::Network {
                    msg: "network".into(),
                },
                "Network error: network",
            ),
            (
                TakerError::SendAmountNotSet {
                    msg: "amount".into(),
                },
                "Send amount not set: amount",
            ),
            (
                TakerError::Deserialize {
                    msg: "decode".into(),
                },
                "Deserialize error: decode",
            ),
            (
                TakerError::Mpsc {
                    msg: "channel".into(),
                },
                "MPSC error: channel",
            ),
            (TakerError::Tor { msg: "tor".into() }, "Tor error: tor"),
            (
                TakerError::AddressParse {
                    msg: "address".into(),
                },
                "Address parse error: address",
            ),
            (
                TakerError::Watcher {
                    msg: "watcher".into(),
                },
                "Watcher error: watcher",
            ),
            (
                TakerError::General {
                    msg: "general".into(),
                },
                "General error: general",
            ),
            (TakerError::IO { msg: "io".into() }, "IO error: io"),
        ];

        for (error, expected) in errors {
            assert_eq!(error.to_string(), expected);
        }
    }

    #[test]
    fn upstream_taker_errors_keep_their_display_text_and_category() {
        let wallet_error = TakerError::from(OpenswapTakerError::Wallet(
            OpenswapWalletError::General("wallet detail".into()),
        ));
        assert!(matches!(
            wallet_error,
            TakerError::Wallet { msg } if msg == "wallet detail"
        ));

        let network_error = TakerError::from(OpenswapTakerError::Net(NetError::ConnectionTimedOut));
        assert!(matches!(
            network_error,
            TakerError::Network { msg } if msg == "ConnectionTimedOut"
        ));

        let txid = OpenswapTxid::from_str(&"cd".repeat(32)).unwrap();
        let never_broadcast = TakerError::from(OpenswapTakerError::Wallet(
            OpenswapWalletError::TxNeverBroadcast(vec![txid]),
        ));
        assert!(matches!(
            never_broadcast,
            TakerError::TransactionsNeverBroadcast { txids }
                if txids == vec!["cd".repeat(32)]
        ));

        assert!(matches!(
            TakerError::from(OpenswapTakerError::NotEnoughMakersInOfferBook),
            TakerError::NotEnoughMakers { msg } if msg == "not enough eligible makers"
        ));
    }

    #[test]
    fn all_exported_taker_behavior_variants_remain_distinct() {
        let variants = [
            TakerBehavior::Normal,
            TakerBehavior::DropConnectionAfterFullSetup,
            TakerBehavior::BroadcastContractAfterFullSetup,
        ];
        assert_eq!(variants.len(), 3);
        assert_ne!(variants[0], variants[1]);
        assert_ne!(variants[1], variants[2]);
        assert_ne!(variants[0], variants[2]);
    }
}
