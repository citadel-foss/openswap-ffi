<div align="center">

# Openswap Swift

Swift bindings for the Openswap Bitcoin privacy protocol

</div>

## Overview

`openswap-swift` packages the shared UniFFI taker API as a Swift Package backed by `openswap_ffi.xcframework`.

## Supported Platforms

| Platform | Support |
| --- | --- |
| iOS | arm64 devices, iOS simulator arm64/x86_64 |
| macOS | arm64 and x86_64 |

The Swift package declares iOS 13+ and macOS 10.15+ deployment targets.

## Build and Package

```bash
# Dev build (fast, debug; builds host arch + iOS device + iOS simulator)
bash ./build-xcframework-dev.sh

# Workflow build (configured for github CI only; builds x86_64 Mac-Intel)
bash ./build-xcframework-ci.sh

# Release build (release-smaller profile; builds all Apple targets)
bash ./build-xcframework.sh
swift build
```

Outputs for production build:
- `Sources/Openswap/Openswap.swift` - Swift bindings
- `OpenswapFFI.h` and `module.modulemap` - C headers
- `libopenswap_ffi.a` - native static lib
- `openswap_ffi.xcframework` - packaged slices (macOS, iOS device, iOS simulator). 
- Each platform slice has the aforementioned C header and static lib files.

### Installation
Add the package to your app:

Xcode: File > Add Packages... and select the `openswap-swift` folder.

Package.swift:
```swift
.package(path: "../openswap-swift")
```

Then depend on `Openswap` and `import Openswap` in your app.

## Basic Usage

```swift
import Foundation
import Openswap

// Bitcoin Core RPC settings used by the taker.
let rpcConfig = RpcConfig(
    url: "http://127.0.0.1:18442",                 // Bitcoin Core RPC endpoint
    username: "user",                              // Bitcoin Core RPC username
    password: "password",                          // Bitcoin Core RPC password
    walletName: "taker_wallet"                     // Bitcoin Core wallet name
)

// Create or load the taker wallet.
let taker = try Taker.`init`(
    dataDir: "/path/to/data",                      // taker data directory; nil uses the default taker dir
    walletFileName: "taker_wallet",                // taker wallet file to load or create
    rpcConfig: rpcConfig,                            // Bitcoin Core RPC settings
    controlPort: 9051,                               // Tor control port
    torAuthPassword: "openswap",                   // Tor control password
    zmqAddr: "tcp://127.0.0.1:28332",              // Bitcoin Core ZMQ endpoint
    password: ""                                    // optional wallet encryption password
)

// Configure logging, sync wallet state, and wait for the offer book.
try taker.setupLogging(
    dataDir: "/path/to/data",                      // directory used for file logging
    logLevel: "info"                               // trace | debug | info | warn | error
)
try taker.syncAndSave()
try taker.syncOfferbookAndWait()

// Inspect balances and derive a new receive address.
let balances = try taker.getBalances()
let receiveAddress = try taker.getNextExternalAddress(
    addressType: AddressType(
        addrType: "P2WPKH"                         // external address format to derive
    )
)

print("regular: \(balances.regular) sats")
print("swap: \(balances.swap) sats")
print("contract: \(balances.contract) sats")
print("fidelity: \(balances.fidelity) sats")
print("spendable: \(balances.spendable) sats")
print("receive to: \(receiveAddress.addr)")

// Build the swap request exactly as the taker API expects it.
let swapParams = SwapParams(
    protocol: nil,                                  // optional protocol hint; nil uses the backend default
    sendAmount: 1_000_000,                          // total sats to swap
    makerCount: 2,                                  // number of maker hops
    txCount: 2,                                     // maximum funding transaction splits per hop
    requiredConfirms: 1,                            // minimum funding confirmations
    manuallySelectedOutpoints: nil,                 // optional explicit wallet UTXOs
    preferredMakers: nil,                           // optional maker addresses to prefer
    maxInputBudget: 2,                              // inputs per forwarding tx whose fee the taker covers
    feerate: 1                                      // sats/vB used by every swap transaction
)

// Prepare the swap first, then start it with the returned swap id.
let swapId = try taker.prepareOpenswap(
    swapParams: swapParams                          // fully populated swap request
)
let report = try taker.startOpenswap(
    swapId: swapId                                  // identifier returned by prepareOpenswap
)

print("swap id: \(report.swapId)")
print("status: \(report.status)")
print("outgoing amount: \(report.outgoingAmount) sats")
print("fee paid: \(abs(report.feePaidOrEarned)) sats")
```

## API Reference

### RpcConfig

```swift
let rpcConfig = RpcConfig(
    url: rpcUrl,                                    // Bitcoin Core RPC endpoint
    username: rpcUsername,                          // Bitcoin Core RPC username
    password: rpcPassword,                          // Bitcoin Core RPC password
    walletName: walletName                          // Bitcoin Core wallet name
)
```

### SwapParams

```swift
let swapParams = SwapParams(
    protocol: protocolHint,                         // optional protocol hint string
    sendAmount: sendAmountSats,                     // total sats to swap
    makerCount: makerCount,                         // number of maker hops
    txCount: txCount,                               // maximum funding transaction splits per hop
    requiredConfirms: requiredConfirms,             // minimum funding confirmations
    manuallySelectedOutpoints: outpoints,           // optional explicit wallet UTXOs
    preferredMakers: preferredMakers,               // optional maker addresses to prefer
    maxInputBudget: maxInputBudget,                 // inputs per forwarding tx whose fee the taker covers
    feerate: feerate                                // sats/vB used by every swap transaction
)
```

### Taker

```swift
let taker = try Taker.`init`(
    dataDir: dataDir,                               // taker data directory
    walletFileName: walletFileName,                 // taker wallet file to load or create
    rpcConfig: rpcConfig,                           // Bitcoin Core RPC settings
    controlPort: controlPort,                       // Tor control port
    torAuthPassword: torAuthPassword,               // Tor control password
    zmqAddr: zmqAddr,                               // Bitcoin Core ZMQ endpoint
    password: password                              // optional wallet encryption password
)

try taker.setupLogging(dataDir: dataDir, logLevel: logLevel)                           // configure taker logging
let swapId = try taker.prepareOpenswap(swapParams: swapParams)                         // prepare a swap and return the swap id
let report = try taker.startOpenswap(swapId: swapId)                                   // execute a prepared swap
let txs = try taker.getTransactions(count: count, skip: skip)                          // recent wallet transactions
let internal = try taker.getNextInternalAddresses(count: count, addressType: addressType) // derive internal HD addresses
let external = try taker.getNextExternalAddress(addressType: addressType)              // derive an external receive address
let utxos = try taker.listAllUtxoSpendInfo()                                           // wallet UTXOs plus spend metadata
try taker.backup(destinationPath: destinationPath, password: backupPassword)           // write a wallet backup JSON file
try taker.lockUnspendableUtxos()                                                       // lock fidelity and live-contract UTXOs
let txid = try taker.sendToAddress(address: address, amount: amount, feeRate: feeRate, manuallySelectedOutpoints: outpoints) // send sats to an external address
let balances = try taker.getBalances()                                                 // read wallet balances
try taker.syncAndSave()                                                                // sync wallet state and persist it
try taker.syncOfferbookAndWait()                                                       // block until the offer book is synchronized
let offerBook = try taker.fetchOffers()                                                // read the current offer book
let renderedOffer = try taker.displayOffer(makerOffer: offer)                          // format a maker offer for display
let walletName = try taker.getWalletName()                                             // read the wallet name
try taker.recoverActiveSwap()                                                          // resume recovery for a failed active swap
let makers = try taker.fetchAllMakers()                                                // read maker addresses across all states
```

### AddressType, Balances, and SwapReport

```swift
let addressType = AddressType(
    addrType: "P2WPKH"                         // external address format to derive
)

balances.regular                                // single-signature seed balance in sats
balances.swap                                   // swap-coin balance in sats
balances.contract                               // live contract balance in sats
balances.fidelity                               // fidelity bond balance in sats
balances.spendable                              // regular + swap balance in sats

report.swapId                                   // unique swap identifier
report.role                                     // report creator, usually Taker
report.status                                   // swap terminal state
report.swapDurationSeconds                      // execution duration in seconds
report.recoveryDurationSeconds                  // recovery duration in seconds
report.startTimestamp                           // unix start timestamp
report.endTimestamp                             // unix end timestamp
report.network                                  // bitcoin network name
report.errorMessage                             // error detail, if present
report.incomingAmount                           // sats received by the taker
report.outgoingAmount                           // sats sent by the taker
report.feePaidOrEarned                          // negative when paid, positive when earned
report.fundingTxids                             // funding txids grouped by hop
report.recoveryTxids                            // recovery txids, if any
report.timelock                                 // contract timelock in blocks
report.makersCount                              // maker hop count used in the swap
report.makerAddresses                           // maker addresses used in the route
report.totalMakerFees                           // aggregate maker fees in sats
report.miningFee                                // mining fees in sats
report.feePercentage                            // total fee as a percentage of amount
report.makerFeeInfo                             // per-maker fee breakdown
report.inputUtxos                               // input UTXO amounts in sats
report.outputChangeAmounts                      // output change amounts in sats
report.outputSwapAmounts                        // output swap amounts in sats
report.outputChangeUtxos                        // change outputs with amount and address
report.outputSwapUtxos                          // swap outputs with amount and address
```

## Testing

```bash
cd ../ffi-commons
./ffi-docker-setup setup
./ffi-docker-setup start 4

cd ../openswap-swift
swift test

cd ../ffi-commons
./ffi-docker-setup stop
```

## Requirements

- Xcode 14+.
- Swift 5.7+.
- Bitcoin Core with RPC enabled, fully synced, non-pruned, and `-txindex` enabled.
- Tor daemon for live taker workflows.

## Support

- [Main Openswap Repository](https://github.com/citadel-foss/openswap)
- [FFI Commons](../ffi-commons)

## License

MIT License - see [LICENSE](../LICENSE) for details
