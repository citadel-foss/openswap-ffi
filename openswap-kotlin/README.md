<div align="center">

# Openswap Kotlin

Kotlin bindings for the Openswap Bitcoin privacy protocol

</div>

## Overview

`openswap-kotlin` packages the shared UniFFI taker API for Android and JVM consumers.

## Supported Platforms

| Target | Notes |
| --- | --- |
| Android `arm64-v8a` | Release packaging supported from Linux, macOS, and Windows host scripts |
| Android `armeabi-v7a` | Release packaging supported from Linux, macOS, and Windows host scripts |
| Android `x86_64` | Development and emulator/test support |
| JVM/Desktop | Host-native library loading through the `lib` module |

## Build and Package

### Development builds

```bash
# Linux host, native JVM test build
bash ./build-scripts/development/build-dev-linux-jvm.sh

# Linux host, Android x86_64 emulator build
bash ./build-scripts/development/build-dev-linux-x86_64.sh
```

### Release builds

```bash
# Linux host
bash ./build-scripts/release/build-release-linux-arm64_v8a.sh
bash ./build-scripts/release/build-release-linux-armeabi-v7a.sh

# macOS host
bash ./build-scripts/release/build-release-macos-arm64_v8a.sh
bash ./build-scripts/release/build-release-macos-armeabi-v7a.sh
```

### Package the library

```bash
./gradlew :lib:assembleRelease
./gradlew :lib:publishToMavenLocal -PlocalBuild=true
```

## Basic Usage

```kotlin
import org.openswap.*

// Bitcoin Core RPC settings used by the taker.
val rpcConfig = RpcConfig(
    url = "http://127.0.0.1:18442",              // Bitcoin Core RPC endpoint
    username = "user",                           // Bitcoin Core RPC username
    password = "password",                       // Bitcoin Core RPC password
    walletName = "taker_wallet",                 // Bitcoin Core wallet name
)

// Create or load the taker wallet.
val taker = Taker.init(
    dataDir = "/path/to/data",                   // taker data directory; null uses the default taker dir
    walletFileName = "taker_wallet",             // taker wallet file to load or create
    rpcConfig = rpcConfig,                         // Bitcoin Core RPC settings
    controlPort = 9051u,                           // Tor control port
    torAuthPassword = "openswap",                // Tor control password
    zmqAddr = "tcp://127.0.0.1:28332",           // Bitcoin Core ZMQ endpoint
    password = "",                               // optional wallet encryption password
)

// Configure logging, sync wallet state, and wait for the offer book.
taker.setupLogging(
    dataDir = "/path/to/data",                   // directory used for file logging
    logLevel = "info",                           // trace | debug | info | warn | error
)
taker.syncAndSave()
taker.syncOfferbookAndWait()

// Inspect balances and derive a new receive address.
val balances = taker.getBalances()
val receiveAddress = taker.getNextExternalAddress(
    AddressType(addrType = "P2WPKH"),            // external address format to derive
)

println("regular: ${balances.regular} sats")
println("swap: ${balances.swap} sats")
println("contract: ${balances.contract} sats")
println("fidelity: ${balances.fidelity} sats")
println("spendable: ${balances.spendable} sats")
println("receive to: ${receiveAddress.addr}")

// Build the swap request exactly as the taker API expects it.
val swapParams = SwapParams(
    protocol = null,                              // optional protocol hint; null uses the backend default
    sendAmount = 1_000_000u,                      // total sats to swap
    makerCount = 2u,                              // number of maker hops
    txCount = 2u,                                 // maximum funding transaction splits per hop
    maxInputBudget = 2u,                          // inputs per forwarding tx whose fee the taker covers
    feerate = 1uL,                                // sats/vB used by every swap transaction
    requiredConfirms = 1u,                        // minimum funding confirmations
    manuallySelectedOutpoints = null,             // optional explicit wallet UTXOs
    preferredMakers = null,                       // optional maker addresses to prefer
)

// Prepare the swap first, then start it with the returned swap id.
val swapId = taker.prepareOpenswap(
    swapParams,                                   // fully populated swap request
)
val report = taker.startOpenswap(
    swapId,                                       // identifier returned by prepareOpenswap
)

println("swap id: ${report.swapId}")
println("status: ${report.status}")
println("outgoing amount: ${report.outgoingAmount} sats")
println("fee paid: ${kotlin.math.abs(report.feePaid)} sats")
```

## API Reference

### RpcConfig

```kotlin
val rpcConfig = RpcConfig(
    url = rpcUrl,                                 // Bitcoin Core RPC endpoint
    username = rpcUsername,                       // Bitcoin Core RPC username
    password = rpcPassword,                       // Bitcoin Core RPC password
    walletName = walletName,                      // Bitcoin Core wallet name
)
```

### SwapParams

```kotlin
val swapParams = SwapParams(
    protocol = protocolHint,                      // optional protocol hint string
    sendAmount = sendAmountSats,                  // total sats to swap
    makerCount = makerCount,                      // number of maker hops
    txCount = txCount,                            // maximum funding transaction splits per hop
    maxInputBudget = maxInputBudget,              // inputs per forwarding tx whose fee the taker covers
    feerate = feerate,                            // sats/vB used by every swap transaction
    requiredConfirms = requiredConfirms,          // minimum funding confirmations
    manuallySelectedOutpoints = outpoints,        // optional explicit wallet UTXOs
    preferredMakers = preferredMakers,            // optional maker addresses to prefer
)
```

### Taker

```kotlin
val taker = Taker.init(
    dataDir = dataDir,                            // taker data directory
    walletFileName = walletFileName,              // taker wallet file to load or create
    rpcConfig = rpcConfig,                        // Bitcoin Core RPC settings
    controlPort = controlPort,                    // Tor control port
    torAuthPassword = torAuthPassword,            // Tor control password
    zmqAddr = zmqAddr,                            // Bitcoin Core ZMQ endpoint
    password = password,                          // optional wallet encryption password
)

taker.setupLogging(dataDir = dataDir, logLevel = logLevel)                            // configure taker logging
val swapId = taker.prepareOpenswap(swapParams)                                         // prepare a swap and return the swap id
val report = taker.startOpenswap(swapId)                                               // execute a prepared swap
val txs = taker.getTransactions(count = count, skip = skip)                            // recent wallet transactions
val internal = taker.getNextInternalAddresses(count = count, addressType = addressType) // derive internal HD addresses
val external = taker.getNextExternalAddress(addressType = addressType)                 // derive an external receive address
val utxos = taker.listAllUtxoSpendInfo()                                               // wallet UTXOs plus spend metadata
taker.backup(destinationPath = destinationPath, password = backupPassword)             // write a wallet backup JSON file
taker.lockUnspendableUtxos()                                                           // lock fidelity and live-contract UTXOs
val txid = taker.sendToAddress(address, amount, feeRate, outpoints)                    // send sats to an external address
val balances = taker.getBalances()                                                     // read wallet balances
taker.syncAndSave()                                                                    // sync wallet state and persist it
taker.syncOfferbookAndWait()                                                           // block until the offer book is synchronized
val offerBook = taker.fetchOffers()                                                    // read the current offer book
val renderedOffer = taker.displayOffer(offer)                                          // format a maker offer for display
val walletName = taker.getWalletName()                                                 // read the wallet name
taker.recoverActiveSwap()                                                              // resume recovery for a failed active swap
val makers = taker.fetchAllMakers()                                                    // read maker addresses across all states
```

### AddressType, Balances, and SwapReport

```kotlin
val addressType = AddressType(addrType = "P2WPKH")    // external address format to derive

balances.regular                                      // single-signature seed balance in sats
balances.swap                                         // swap-coin balance in sats
balances.contract                                     // live contract balance in sats
balances.fidelity                                     // fidelity bond balance in sats
balances.spendable                                    // regular + swap balance in sats

report.swapId                                         // unique swap identifier
report.role                                           // report creator, usually Taker
report.status                                         // swap status: Success | Failed | RecoveryHashlock | RecoveryTimelock
report.swapDurationSeconds                            // execution duration in seconds
report.startTimestamp                                 // unix start timestamp
report.endTimestamp                                   // unix end timestamp
report.network                                        // bitcoin network name
report.errorMessage                                   // error detail, if present (nullable)
report.incomingAmount                                 // sats received by the taker
report.outgoingAmount                                 // sats sent by the taker
report.feePaid                                        // fee paid in sats (negative)
report.outgoingUtxos                                  // funding inputs with address and value (List<ReportUtxo>)
report.incomingUtxos                                  // swept wallet outputs with address and value (List<ReportUtxo>)
report.fundingTxids                                   // funding txids grouped by hop (List<List<String>>)
report.makersCount                                    // maker hop count used in the swap (nullable)
report.makerAddresses                                 // maker addresses used in the route
report.totalMakerFees                                 // aggregate maker fees in sats
report.miningFee                                      // mining fees in sats
report.feePercentage                                  // total fee as a percentage of target amount
report.makerFeeInfo                                   // per-maker fee breakdown (List<MakerFeeInfo>)
report.inputUtxos                                     // input UTXO amounts in sats
report.outputChangeAmounts                            // output change amounts in sats
report.outputSwapAmounts                              // output swap amounts in sats
report.outputChangeUtxos                              // change outputs with amount and address (List<UtxoWithAddress>)
report.outputSwapUtxos                                // swap outputs with amount and address (List<UtxoWithAddress>)
```

## Testing

```bash
./gradlew test
```

If you need live end-to-end infrastructure, bring up the shared regtest environment from `../ffi-commons` first.

## Requirements

- Kotlin 1.8+.
- JDK 11+.
- Android SDK 24+ and Android NDK for native Android builds.
- Bitcoin Core with RPC enabled and Tor for live integration testing.

## Support

- [Main Openswap Repository](https://github.com/citadel-foss/openswap)
- [FFI Commons](../ffi-commons)

## License

MIT License - see [LICENSE](../LICENSE) for details
