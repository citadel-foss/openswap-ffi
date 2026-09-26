//! Shared types for openswap N-API bindings
//!
//! This module contains types that are used across multiple modules
//! to avoid duplicate type definitions in TypeScript.

use napi_derive::napi;
use openswap::{
  bitcoin::{
    absolute::LockTime as csLocktime, Address as csAddress, Amount as csAmount,
    PublicKey as csPublicKey, ScriptBuf as csScriptBuf, SignedAmount, Txid as csTxid,
  },
  bitcoind::bitcoincore_rpc::Auth,
  protocol::common_messages::{FidelityProof as csFidelityProof, Offer as csOffer},
  taker::offers::{
    BanReason as csBanReason, BanRecord as csBanRecord, MakerAddress as csMakerAddress,
    MakerOfferCandidate as csMakerOfferCandidate, MakerProtocol as csMakerProtocol,
    MakerState as csMakerState, OfferBook as csOfferBook, UnavailableReason as csUnavailableReason,
    UnavailableState as csUnavailableState,
  },
  wallet::{
    ffi::{
      MakerFeeInfo as csMakerFeeInfo, ReportUtxo as csReportUtxo, TakerReport as csTakerReport,
    },
    BackendConfig as OpenswapBackendConfig, Balances as OpenswapBalances,
    CoreRpcConfig as OpenswapCoreRpcConfig, ElectrumConfig as OpenswapElectrumConfig,
    FidelityBond as csFidelityBond,
  },
};
use std::{error::Error, fmt};

#[napi]
#[derive(Debug)]
pub enum TakerError {
  Wallet,
  Protocol,
  Network,
  General,
  IO,
}

impl fmt::Display for TakerError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      TakerError::Wallet => write!(f, "Wallet error"),
      TakerError::Protocol => write!(f, "Protocol error"),
      TakerError::Network => write!(f, "Network error"),
      TakerError::General => write!(f, "General error"),
      TakerError::IO => write!(f, "IO error"),
    }
  }
}

impl AsRef<str> for TakerError {
  fn as_ref(&self) -> &str {
    match self {
      TakerError::Wallet => "Wallet error",
      TakerError::Protocol => "Protocol error",
      TakerError::Network => "Network error",
      TakerError::General => "General error",
      TakerError::IO => "IO error",
    }
  }
}

impl Error for TakerError {}

#[napi]
pub enum TakerBehavior {
  Normal,
  DropConnectionAfterFullSetup,
  BroadcastContractAfterFullSetup,
}

#[napi(object)]
pub struct Balances {
  pub regular: i64,
  pub swap: i64,
  pub contract: i64,
  pub fidelity: i64,
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

#[napi(object)]
pub struct Address {
  pub address: String,
}

impl From<csAddress> for Address {
  fn from(addr: csAddress) -> Self {
    Self {
      address: addr.to_string(),
    }
  }
}

#[napi(object)]
pub struct ListTransactionResult {
  pub info: WalletTxInfo,
  pub detail: GetTransactionResultDetail,
  pub trusted: Option<bool>,
  pub comment: Option<String>,
}

#[napi(object)]
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

#[napi(object)]
pub struct GetTransactionResultDetail {
  pub address: Option<Address>,
  pub category: String,
  pub amount: SignedAmountSats,
  pub label: Option<String>,
  pub vout: u32,
  pub fee: Option<SignedAmountSats>,
  pub abandoned: Option<bool>,
}

#[napi(object)]
pub struct RPCConfig {
  pub url: String,
  pub username: String,
  pub password: String,
  pub wallet_name: String,
}

impl RPCConfig {
  pub fn into_core_rpc_config(self, zmq_addr: String) -> OpenswapCoreRpcConfig {
    OpenswapCoreRpcConfig {
      url: self.url,
      auth: Auth::UserPass(self.username, self.password),
      wallet_name: self.wallet_name,
      zmq_addr,
    }
  }
}

impl From<RPCConfig> for OpenswapCoreRpcConfig {
  fn from(config: RPCConfig) -> Self {
    let default = Self::default();
    Self {
      url: config.url,
      auth: Auth::UserPass(config.username, config.password),
      wallet_name: config.wallet_name,
      zmq_addr: default.zmq_addr,
    }
  }
}

#[napi(object)]
pub struct BackendConfig {
  pub kind: String,
  pub url: Option<String>,
  pub username: Option<String>,
  pub password: Option<String>,
  pub wallet_name: Option<String>,
  pub zmq_addr: Option<String>,
  pub socks5: Option<String>,
  pub timeout: Option<u8>,
  pub poll_interval_secs: Option<i64>,
  pub max_retries: Option<u8>,
}

impl TryFrom<BackendConfig> for OpenswapBackendConfig {
  type Error = napi::Error;

  fn try_from(config: BackendConfig) -> napi::Result<Self> {
    match config.kind.to_lowercase().as_str() {
      "rpc" => config.into_rpc_backend(),
      "electrum" => config.into_electrum_backend(),
      other => Err(napi::Error::from_reason(format!(
        "Invalid backend kind: {} (expected rpc or electrum)",
        other
      ))),
    }
  }
}

impl BackendConfig {
  fn into_rpc_backend(self) -> napi::Result<OpenswapBackendConfig> {
    let mut config = OpenswapCoreRpcConfig::default();
    apply_if_some(&mut config.url, self.url);
    apply_if_some(&mut config.wallet_name, self.wallet_name);
    apply_if_some(&mut config.zmq_addr, self.zmq_addr);
    config.auth = match (self.username, self.password) {
      (Some(username), Some(password)) => Auth::UserPass(username, password),
      (None, None) => config.auth,
      _ => {
        return Err(napi::Error::from_reason(
          "RPC backend requires username and password together",
        ));
      }
    };
    Ok(OpenswapBackendConfig::CoreRpc(config))
  }

  fn into_electrum_backend(self) -> napi::Result<OpenswapBackendConfig> {
    let mut config = OpenswapElectrumConfig {
      url: self
        .url
        .ok_or_else(|| napi::Error::from_reason("Electrum backend requires url"))?,
      ..OpenswapElectrumConfig::default()
    };
    config.socks5 = self.socks5;
    config.timeout = self.timeout;
    config.poll_interval_secs = self.poll_interval_secs.map(|v| v as u64);
    apply_if_some(&mut config.max_retries, self.max_retries);
    Ok(OpenswapBackendConfig::Electrum(config))
  }
}

fn apply_if_some<T>(target: &mut T, value: Option<T>) {
  if let Some(value) = value {
    *target = value;
  }
}

#[napi(object)]
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

#[napi(object)]
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

#[napi(object)]
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

#[napi(object)]
pub struct OutPoint {
  pub txid: String,
  pub vout: u32,
}

#[napi(object)]
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

#[napi(object)]
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

#[napi(object)]
pub struct UtxoSpendInfo {
  pub spend_type: String,
  pub path: Option<String>,
  pub multisig_redeemscript: Option<ScriptBuf>,
  pub input_value: Option<Amount>,
  pub index: Option<u32>,
}

#[napi(object)]
#[derive(Debug)]
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

#[napi(object)]
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

#[napi(object)]
pub struct FidelityProof {
  pub bond: FidelityBond,
  pub cert_hash: Vec<u8>,
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

#[napi(object)]
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
        txid: bond.outpoint().txid.to_string(),
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

// #[napi(object)]
// pub struct MakerStats {
//   pub total_makers: u32,
//   pub online_makers: u32,
//   pub avg_base_fee: i64,
//   pub avg_amount_relative_fee_pct: f64,
//   pub avg_time_relative_fee_pct: f64,
//   pub total_liquidity: i64,
//   pub avg_min_size: i64,
//   pub avg_max_size: i64,
// }

#[napi(object)]
pub struct Offer {
  pub base_fee: i64,
  pub amount_relative_fee_pct: f64,
  pub time_relative_fee_pct: f64,
  pub required_confirms: u32,
  pub minimum_locktime: u16,
  pub max_size: i64,
  pub min_size: i64,
  pub tweakable_point: PublicKey,
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

#[napi(object)]
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

#[napi(object)]
#[derive(Debug, Clone)]
pub struct MakerState {
  /// State type: "Good", "Unavailable", or "Banned".
  pub state_type: String,
  /// Recovery details when state_type is "Unavailable".
  pub unavailable: Option<UnavailableState>,
  /// Permanent ban details when state_type is "Banned".
  pub ban: Option<BanRecord>,
}

#[napi(object)]
#[derive(Debug, Clone)]
pub struct UnavailableState {
  /// One of the documented UnavailableReason variant names.
  pub reason: String,
  /// Start of the unbroken failure run, as Unix seconds.
  pub since_ts: Option<i64>,
  /// Most recent attempt, as Unix seconds.
  pub last_attempt_ts: Option<i64>,
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
      since_ts: state
        .since_ts
        .map(|timestamp| i64::try_from(timestamp).unwrap_or(i64::MAX)),
      last_attempt_ts: state
        .last_attempt_ts
        .map(|timestamp| i64::try_from(timestamp).unwrap_or(i64::MAX)),
      attempts: state.attempts,
    }
  }
}

#[napi(object)]
#[derive(Debug, Clone)]
pub struct BanRecord {
  /// One of the documented BanReason variant names.
  pub reason: String,
  /// Time the ban was first recorded, as Unix seconds.
  pub recorded_at_ts: i64,
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
      recorded_at_ts: i64::try_from(record.recorded_at_ts).unwrap_or(i64::MAX),
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

#[napi(object)]
#[derive(Debug, Clone)]
pub struct MakerProtocol {
  /// Protocol type: "Legacy" or "Taproot"
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

#[napi(object)]
pub struct MakerOfferCandidate {
  pub address: MakerAddress,
  pub offer: Option<Offer>,
  pub state: MakerState,
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

#[napi(object)]
pub struct OfferBook {
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

#[napi(object)]
#[derive(Debug)]
pub struct MakerFeeInfo {
  pub maker_index: u32,
  pub maker_address: String,
  pub base_fee: f64,
  pub amount_relative_fee: f64,
  pub time_relative_fee: f64,
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

#[napi(object)]
#[derive(Debug)]
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
  /// Output change coin UTXOs with amounts and addresses [(amount, address)]
  pub output_change_utxos: Vec<UtxoWithAddress>,
  /// Output swap coin UTXOs with amounts and addresses [(amount, address)]
  pub output_swap_utxos: Vec<UtxoWithAddress>,
}

#[napi(object)]
#[derive(Debug)]
pub struct UtxoWithAddress {
  pub amount: i64,
  pub address: String,
}

/// User-facing UTXO information recorded in a swap report.
#[napi(object)]
#[derive(Debug)]
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
    Self {
      swap_id: report.swap_id,
      role: "Taker".to_string(),
      status: report.status.to_string(),
      swap_duration_seconds: report.swap_duration_seconds,
      start_timestamp: report.start_timestamp as i64,
      end_timestamp: report.end_timestamp as i64,
      network: report.network.to_string(),
      error_message: report.error_message,
      incoming_amount: report.incoming_amount as i64,
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
      output_change_amounts: report
        .output_change_amounts
        .into_iter()
        .map(|v| v as i64)
        .collect(),
      output_swap_amounts: report
        .output_swap_amounts
        .into_iter()
        .map(|v| v as i64)
        .collect(),
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
#[napi]
pub enum AddressType {
  P2WPKH,
  P2TR,
}

impl TryFrom<AddressType> for openswap::wallet::AddressType {
  type Error = napi::Error;

  fn try_from(addr: AddressType) -> Result<Self, Self::Error> {
    match addr {
      AddressType::P2TR => Ok(openswap::wallet::AddressType::P2TR),
      AddressType::P2WPKH => Ok(openswap::wallet::AddressType::P2WPKH),
    }
  }
}

#[napi(object)]
#[allow(unused)]
pub struct WalletBackup {
  pub file_name: String,
}

#[cfg(test)]
mod contract_tests {
  use super::*;
  use openswap::bitcoin::absolute::{Height, Time};
  use openswap::taker::{BanReason, BanRecord, UnavailableReason, UnavailableState};

  fn backend(kind: &str) -> BackendConfig {
    BackendConfig {
      kind: kind.to_owned(),
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

  #[test]
  fn report_utxo_preserves_address_and_value() {
    let utxo = ReportUtxo::from(csReportUtxo {
      address: "bc1qreport".to_owned(),
      value: 42_000,
    });

    assert_eq!(utxo.address, "bc1qreport");
    assert_eq!(utxo.value, 42_000);
  }

  #[test]
  fn backend_config_validates_kinds_credentials_and_electrum_url() {
    let invalid = OpenswapBackendConfig::try_from(backend("unknown")).unwrap_err();
    assert_eq!(
      invalid.reason,
      "Invalid backend kind: unknown (expected rpc or electrum)"
    );

    let mut incomplete_rpc = backend("rpc");
    incomplete_rpc.username = Some("user".to_owned());
    assert_eq!(
      OpenswapBackendConfig::try_from(incomplete_rpc)
        .unwrap_err()
        .reason,
      "RPC backend requires username and password together"
    );

    assert_eq!(
      OpenswapBackendConfig::try_from(backend("electrum"))
        .unwrap_err()
        .reason,
      "Electrum backend requires url"
    );
  }

  #[test]
  fn backend_config_preserves_explicit_rpc_and_electrum_options() {
    let mut rpc = backend("RPC");
    rpc.url = Some("http://node:8332".to_owned());
    rpc.username = Some("alice".to_owned());
    rpc.password = Some("secret".to_owned());
    rpc.wallet_name = Some("wallet".to_owned());
    rpc.zmq_addr = Some("tcp://node:28332".to_owned());
    match OpenswapBackendConfig::try_from(rpc).unwrap() {
      OpenswapBackendConfig::CoreRpc(config) => {
        assert_eq!(config.url, "http://node:8332");
        assert_eq!(config.wallet_name, "wallet");
        assert_eq!(config.zmq_addr, "tcp://node:28332");
        assert!(
          matches!(config.auth, Auth::UserPass(user, password) if user == "alice" && password == "secret")
        );
      }
      _ => panic!("RPC kind must produce the CoreRpc backend"),
    }

    let mut electrum = backend("Electrum");
    electrum.url = Some("ssl://electrum.example:50002".to_owned());
    electrum.socks5 = Some("127.0.0.1:9050".to_owned());
    electrum.timeout = Some(120);
    electrum.poll_interval_secs = Some(15);
    electrum.max_retries = Some(8);
    match OpenswapBackendConfig::try_from(electrum).unwrap() {
      OpenswapBackendConfig::Electrum(config) => {
        assert_eq!(config.url, "ssl://electrum.example:50002");
        assert_eq!(config.socks5.as_deref(), Some("127.0.0.1:9050"));
        assert_eq!(config.timeout, Some(120));
        assert_eq!(config.poll_interval_secs, Some(15));
        assert_eq!(config.max_retries, 8);
      }
      _ => panic!("Electrum kind must produce the Electrum backend"),
    }
  }

  #[test]
  fn public_enum_and_error_variants_remain_distinct() {
    let errors = [
      TakerError::Wallet,
      TakerError::Protocol,
      TakerError::Network,
      TakerError::General,
      TakerError::IO,
    ];
    assert_eq!(
      errors.map(|error| error.to_string()),
      [
        "Wallet error",
        "Protocol error",
        "Network error",
        "General error",
        "IO error",
      ]
    );

    assert!(matches!(
      openswap::wallet::AddressType::try_from(AddressType::P2WPKH).unwrap(),
      openswap::wallet::AddressType::P2WPKH
    ));
    assert!(matches!(
      openswap::wallet::AddressType::try_from(AddressType::P2TR).unwrap(),
      openswap::wallet::AddressType::P2TR
    ));
  }

  #[test]
  fn locktime_conversion_keeps_height_and_timestamp_semantics() {
    let blocks = LockTime::from(csLocktime::Blocks(Height::from_consensus(144).unwrap()));
    assert_eq!(blocks.lock_type, "Blocks");
    assert_eq!(blocks.value, 144);

    let seconds = LockTime::from(csLocktime::Seconds(
      Time::from_consensus(500_000_000).unwrap(),
    ));
    assert_eq!(seconds.lock_type, "Seconds");
    assert_eq!(seconds.value, 500_000_000);
  }

  #[test]
  fn maker_state_and_protocol_conversion_cover_every_variant() {
    let states = [
      MakerState::from(csMakerState::Good),
      MakerState::from(csMakerState::Unavailable(UnavailableState {
        reason: UnavailableReason::NoOfferResponse,
        since_ts: Some(100),
        last_attempt_ts: Some(200),
        attempts: 7,
      })),
      MakerState::from(csMakerState::Banned(BanRecord {
        reason: BanReason::ProvenViolation,
        recorded_at_ts: 300,
      })),
    ];
    assert_eq!(states[0].state_type, "Good");
    assert!(states[0].unavailable.is_none());
    assert!(states[0].ban.is_none());
    assert_eq!(states[1].state_type, "Unavailable");
    let unavailable = states[1].unavailable.as_ref().unwrap();
    assert_eq!(unavailable.reason, "NoOfferResponse");
    assert_eq!(unavailable.since_ts, Some(100));
    assert_eq!(unavailable.last_attempt_ts, Some(200));
    assert_eq!(unavailable.attempts, 7);
    assert!(states[1].ban.is_none());
    assert_eq!(states[2].state_type, "Banned");
    assert!(states[2].unavailable.is_none());
    let ban = states[2].ban.as_ref().unwrap();
    assert_eq!(ban.reason, "ProvenViolation");
    assert_eq!(ban.recorded_at_ts, 300);

    assert_eq!(
      MakerProtocol::from(csMakerProtocol::Legacy).protocol_type,
      "Legacy"
    );
    assert_eq!(
      MakerProtocol::from(csMakerProtocol::Taproot).protocol_type,
      "Taproot"
    );
    assert_eq!(
      MakerProtocol::from(csMakerProtocol::Unified).protocol_type,
      "Unified"
    );
  }
}
