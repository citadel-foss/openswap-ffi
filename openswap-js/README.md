<div align="center">

# Openswap JS

Node.js bindings for the Openswap Bitcoin privacy protocol

[![MIT Licensed](https://img.shields.io/badge/license-MIT-blue.svg)](./LICENSE)
[![Node.js >= 18](https://img.shields.io/badge/node-%3E%3D18.0.0-brightgreen.svg)](https://nodejs.org)

</div>

## Overview

`openswap-js` exposes the Openswap taker API to Node.js and TypeScript through N-API.

## Platform Support

Prebuilt targets are configured for:

| Platform | Architectures |
| --- | --- |
| Linux | x64, arm64, arm64 musl |
| macOS | x64, arm64 |
| Windows | x64, arm64 |
| FreeBSD | x64 |
| Android | arm64 |

## Installation

```bash
npm install openswap-js
# or
yarn add openswap-js
# or
pnpm add openswap-js
```

## Build and Package

```bash
cd openswap-js
yarn install
yarn build
```

For a debug build:

```bash
yarn build:debug
```

For CI or release automation, the package uses the standard `napi prepublish -t npm` flow wired into `prepublishOnly`.

## Basic Usage

```typescript
import {
  AddressType,
  Taker,
  type RpcConfig,
  type SwapParams,
} from 'openswap-js'

// Bitcoin Core RPC settings used by the taker.
const rpcConfig: RpcConfig = {
  url: 'http://127.0.0.1:18442',          // Bitcoin Core RPC endpoint
  username: 'user',                       // Bitcoin Core RPC username
  password: 'password',                   // Bitcoin Core RPC password
  walletName: 'taker_wallet',             // Bitcoin Core wallet name
}

// Create or load the taker wallet.
const taker = new Taker(
  null,                                   // null uses the default taker data directory
  'taker_wallet',                         // taker wallet file to load or create
  rpcConfig,                              // Bitcoin Core RPC settings
  9051,                                   // Tor control port
  'openswap',                             // Tor control password
  'tcp://127.0.0.1:28332',                // Bitcoin Core ZMQ endpoint
  '',                                     // optional wallet encryption password
)

// Configure logging, sync wallet state, and wait for the offer book.
Taker.setupLogging(
  null,                                   // null writes logs under the default taker directory
  'info',                                 // trace | debug | info | warn | error
)
taker.syncAndSave()
taker.syncOfferbookAndWait()

// Inspect balances and derive a new receive address.
const balances = taker.getBalances()
const receiveAddress = taker.getNextExternalAddress(
  AddressType.P2WPKH,                     // external address format to derive
)

console.log(`regular: ${balances.regular} sats`)
console.log(`swap: ${balances.swap} sats`)
console.log(`contract: ${balances.contract} sats`)
console.log(`fidelity: ${balances.fidelity} sats`)
console.log(`spendable: ${balances.spendable} sats`)
console.log(`receive to: ${receiveAddress.address}`)

// Build the swap request exactly as the taker API expects it.
const swapParams: SwapParams = {
  protocol: undefined,                    // optional protocol hint; omit to use the backend default
  sendAmount: 1_000_000,                  // total sats to swap
  makerCount: 2,                          // number of maker hops
  txCount: 2,                             // maximum funding transaction splits per hop
  maxInputBudget: 2,                      // inputs per forwarding tx whose fee the taker covers
  feerate: 1,                             // sats/vB used by every swap transaction
  requiredConfirms: 1,                    // minimum funding confirmations
  manuallySelectedOutpoints: undefined,   // optional explicit wallet UTXOs
  preferredMakers: undefined,             // optional maker addresses to prefer
}

// Prepare the swap first, then start it with the returned swap id.
const swapId = taker.prepareOpenswap(swapParams)
const report = taker.startOpenswap(
  swapId,                                 // identifier returned by prepareOpenswap
)

console.log(`swap id: ${report.swapId}`)
console.log(`status: ${report.status}`)
console.log(`outgoing amount: ${report.outgoingAmount} sats`)
console.log(`fee paid: ${Math.abs(report.feePaidOrEarned)} sats`)
```

## API Reference

### RpcConfig

```typescript
const rpcConfig: RpcConfig = {
  url: rpcUrl,                            // Bitcoin Core RPC endpoint
  username: rpcUsername,                  // Bitcoin Core RPC username
  password: rpcPassword,                  // Bitcoin Core RPC password
  walletName: walletName,                 // Bitcoin Core wallet name
}
```

### SwapParams

```typescript
const swapParams: SwapParams = {
  protocol: protocolHint,                 // optional protocol hint string
  sendAmount: sendAmountSats,             // total sats to swap
  makerCount: makerCount,                 // number of maker hops
  txCount: txCount,                       // maximum funding transaction splits per hop
  maxInputBudget: maxInputBudget,         // inputs per forwarding tx whose fee the taker covers
  feerate: feerate,                       // sats/vB used by every swap transaction
  requiredConfirms: requiredConfirms,     // minimum funding confirmations
  manuallySelectedOutpoints: outpoints,   // optional explicit wallet UTXOs
  preferredMakers: preferredMakers,       // optional maker addresses to prefer
}
```

### Taker

```typescript
const taker = new Taker(
  dataDir,                                // taker data directory
  walletFileName,                         // taker wallet file to load or create
  rpcConfig,                              // Bitcoin Core RPC settings
  controlPort,                            // Tor control port
  torAuthPassword,                        // Tor control password
  zmqAddr,                                // Bitcoin Core ZMQ endpoint
  password,                               // optional wallet encryption password
)

Taker.setupLogging(dataDir, logLevel)                                     // configure taker logging
const swapId = taker.prepareOpenswap(swapParams)                          // prepare a swap and return the swap id
const report = taker.startOpenswap(swapId)                                // execute a prepared swap
const txs = taker.getTransactions(count, skip)                            // recent wallet transactions
const internal = taker.getNextInternalAddresses(count, addressType)       // derive internal HD addresses
const external = taker.getNextExternalAddress(addressType)                // derive an external receive address
const utxos = taker.listAllUtxoSpendInfo()                                // wallet UTXOs plus spend metadata
taker.backup(destinationPath, password)                                   // write a wallet backup JSON file
taker.lockUnspendableUtxos()                                              // lock fidelity and live-contract UTXOs
const txid = taker.sendToAddress(address, amount, feeRate, outpoints)     // send sats to an external address
const balances = taker.getBalances()                                      // read wallet balances
taker.syncAndSave()                                                       // sync wallet state and persist it
taker.syncOfferbookAndWait()                                              // block until the offer book is synchronized
const offerBook = taker.fetchOffers()                                     // read the current offer book
const renderedOffer = taker.displayOffer(offer)                           // format a maker offer for display
const walletName = taker.getName()                                        // read the wallet name
taker.recoverActiveSwap()                                                 // resume recovery for a failed active swap
const makers = taker.fetchAllMakers()                                     // read maker addresses across all states
```

### AddressType, Balances, and SwapReport

```typescript
const addressType = AddressType.P2WPKH             // external address format to derive

balances.regular                                   // single-signature seed balance in sats
balances.swap                                      // swap-coin balance in sats
balances.contract                                  // live contract balance in sats
balances.fidelity                                  // fidelity bond balance in sats
balances.spendable                                 // regular + swap balance in sats

report.swapId                                      // unique swap identifier
report.role                                        // report creator, usually Taker
report.status                                      // swap terminal state
report.swapDurationSeconds                         // execution duration in seconds
report.recoveryDurationSeconds                     // recovery duration in seconds
report.startTimestamp                              // unix start timestamp
report.endTimestamp                                // unix end timestamp
report.network                                     // bitcoin network name
report.errorMessage                                // error detail, if present
report.incomingAmount                              // sats received by the taker
report.outgoingAmount                              // sats sent by the taker
report.feePaidOrEarned                             // negative when paid, positive when earned
report.fundingTxids                                // funding txids grouped by hop
report.recoveryTxids                               // recovery txids, if any
report.timelock                                    // contract timelock in blocks
report.makersCount                                 // maker hop count used in the swap
report.makerAddresses                              // maker addresses used in the route
report.totalMakerFees                              // aggregate maker fees in sats
report.miningFee                                   // mining fees in sats
report.feePercentage                               // total fee as a percentage of amount
report.makerFeeInfo                                // per-maker fee breakdown
report.inputUtxos                                  // input UTXO amounts in sats
report.outputChangeAmounts                         // output change amounts in sats
report.outputSwapAmounts                           // output swap amounts in sats
report.outputChangeUtxos                           // change outputs with amount and address
report.outputSwapUtxos                             // swap outputs with amount and address
```

## Requirements

- Node.js 18 or newer.
- Rust 1.75.0 or newer when building from source.
- Bitcoin Core with RPC enabled, fully synced, non-pruned, and `-txindex` enabled.
- Tor daemon for maker discovery and routing.

## Reference Application

[taker-app](https://github.com/citadel-foss/taker-app) is the primary desktop reference implementation for this binding and demonstrates wallet management, maker discovery, swap execution, analytics, and UTXO control.

## Support

- GitHub Issues: [openswap-ffi/issues](https://github.com/citadel-foss/openswap-ffi/issues)
- Discussions: [citadel-foss/openswap](https://github.com/citadel-foss/openswap/discussions)
