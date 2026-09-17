/**
 * JVM integration test: 4 takers × 2 makers.
 *
 * Mirrors the Rust `swap_test`: one test per (backend × protocol) combination —
 * legacy/taproot over rpc/electrum — each running a 2-maker openswap against the
 * Docker regtest stack (1 RPC maker + 1 Electrum maker).
 */

package org.openswap

import org.junit.jupiter.api.Test
import org.junit.jupiter.api.io.TempDir
import java.nio.file.Path
import kotlin.test.assertEquals
import kotlin.test.assertNotNull
import kotlin.test.assertTrue

class SwapTest {

    companion object {
        private const val WALLET_PASSWORD = "ffi-live-test-wallet-password"

        /** Keep the native handle alive until this case's JVM exits. */
        private val liveTakers = mutableListOf<Taker>()
    }

    private enum class Backend { RPC, ELECTRUM }

    private val swapAmount = 500_000uL
    private val makerCount = 2u
    private val makerContainers = listOf("openswap-makerd1", "openswap-makerd2")
    private val makerReadyAttempts = 3

    /** Fund [address] with [btc] BTC from the Docker bitcoind `test` wallet. */
    private fun fund(address: String, btc: String) {
        val p = ProcessBuilder(
            "docker", "exec", "openswap-bitcoind",
            "bitcoin-cli", "-regtest", "-rpcport=18442",
            "-rpcwallet=test", "-rpcuser=user", "-rpcpassword=password",
            "sendtoaddress", address, btc,
        ).redirectErrorStream(true).start()
        val out = p.inputStream.bufferedReader().readText().trim()
        check(p.waitFor() == 0) { "funding failed: $out" }
    }

    /** Sync until spendable reaches [target], tolerating Electrum indexing lag. */
    private fun waitForSpendable(taker: Taker, target: ULong): Balances {
        repeat(30) {
            taker.syncAndSave()
            val b = taker.getBalances()
            if (b.spendable.toULong() >= target) return b
            Thread.sleep(3000)
        }
        return taker.getBalances()
    }

    /** Read the onion addresses belonging to this job's two Docker makers. */
    private fun localMakerAddresses(): List<String> {
        val marker = "Generated new Tor Hidden Service Hostname:"
        return makerContainers.map { container ->
            val process = ProcessBuilder("docker", "logs", container)
                .redirectErrorStream(true)
                .start()
            val logs = process.inputStream.bufferedReader().readText()
            check(process.waitFor() == 0) { "$container: failed to read maker logs: $logs" }

            logs.lineSequence()
                .filter { marker in it }
                .map { it.substringAfter(marker).trim().substringBefore(' ') }
                .lastOrNull()
                ?: error("$container: maker onion address not found in logs")
        }
    }

    /** Poll only this job's makers and wait until both offers are usable. */
    private fun waitForSuitableMakers(
        taker: Taker,
        name: String,
        protocol: String,
        makerAddresses: List<String>,
    ) {
        var suitableCount = 0

        repeat(makerReadyAttempts) { attemptIndex ->
            val attempt = attemptIndex + 1
            makerAddresses.forEach { address ->
                try {
                    println("  polling local maker $address")
                    taker.pollMaker(address)
                } catch (error: Exception) {
                    println("  poll failed for $address: ${error.message}")
                }
            }

            val offerbook = taker.fetchOffers()
            val suitable = offerbook.makers.filter { maker ->
                maker.state.stateType == "Good" &&
                    maker.protocol?.protocolType in listOf(protocol, "Unified") &&
                    maker.offer?.let { offer ->
                        offer.minSize <= swapAmount.toLong() &&
                            swapAmount.toLong() <= offer.maxSize
                    } == true
            }
            suitableCount = suitable.size
            println(
                "$name: offerbook attempt $attempt/$makerReadyAttempts has " +
                    "${offerbook.makers.size} total makers, $suitableCount suitable $protocol makers",
            )
            offerbook.makers.forEach { maker ->
                val makerProtocol = maker.protocol?.protocolType ?: "None"
                val amountRange = maker.offer?.let { "${it.minSize}..${it.maxSize} sats" }
                    ?: "no offer"
                println(
                    "  ${maker.address.address}: state=${maker.state.stateType}, " +
                        "protocol=$makerProtocol, amount=$amountRange",
                )
            }
            System.out.flush()

            if (suitableCount >= makerCount.toInt()) return
            if (attempt < makerReadyAttempts) Thread.sleep(10_000)
        }

        error(
            "$name: expected $makerCount suitable $protocol makers for " +
                "$swapAmount sats, found $suitableCount",
        )
    }

    /** Run one taker end-to-end: init → fund → sync → 2-maker openswap → assert. */
    private fun runSwap(
        name: String,
        dataDir: Path,
        backend: Backend,
        protocol: String,
        addrType: String,
    ) {
        println("\n=== $name ($protocol) ===")

        val rpcConfig = if (backend == Backend.RPC) {
            RpcConfig("localhost:18442", "user", "password", "kotlin_$name")
        } else null
        val backendConfig = if (backend == Backend.ELECTRUM) {
            BackendConfig(
                kind = "electrum",
                url = "tcp://localhost:50001",
                username = null,
                password = null,
                walletName = null,
                zmqAddr = null,
                socks5 = null,
                timeout = null,
                pollIntervalSecs = null,
                maxRetries = null,
            )
        } else null

        val makerAddresses = localMakerAddresses()
        val taker = Taker.init(
            dataDir = dataDir.toString(),
            walletFileName = name,
            rpcConfig = rpcConfig,
            controlPort = 9051u,
            torAuthPassword = "openswap",
            zmqAddr = "tcp://localhost:28332",
            password = WALLET_PASSWORD,
            // Public discovery can return makers from another concurrent CI job.
            nostrRelays = emptyList(),
            backendConfig = backendConfig,
            checkBlocklist = null,
        )
        synchronized(liveTakers) { liveTakers.add(taker) }

        waitForSuitableMakers(taker, name, protocol, makerAddresses)

        // Fund with 2x the swap amount across 4 fresh addresses.
        val quarterBtc = "0.0025" // 250,000 sats; 4x = 1,000,000 = 2 * swapAmount
        repeat(4) {
            val addr = taker.getNextExternalAddress(AddressType(addrType)).addr
            fund(addr, quarterBtc)
        }
        val funded = waitForSpendable(taker, swapAmount * 2uL)
        assertEquals(
            (swapAmount * 2uL).toLong(), funded.spendable,
            "$name: spendable should equal funded amount",
        )

        val swapId = taker.prepareOpenswap(
            SwapParams(
                protocol = protocol,
                sendAmount = swapAmount,
                makerCount = makerCount,
                txCount = 1u,
                requiredConfirms = 1u,
                manuallySelectedOutpoints = null,
                preferredMakers = makerAddresses,
                paymentAddress = null,
            ),
        )
        val report = taker.startOpenswap(swapId)
        assertNotNull(report)
        assertEquals(makerCount, report.makersCount, "$name: should route through 2 makers")
        assertTrue(
            report.status.uppercase().contains("SUCCESS"),
            "$name: swap status was ${report.status}",
        )
        println("✓ $name passed (swap_id ${report.swapId})")
    }

    @Test
    fun legacyRpcSwap(@TempDir dir: Path) =
        runSwap("legacy_rpc", dir, Backend.RPC, "Legacy", "P2WPKH")

    @Test
    fun taprootRpcSwap(@TempDir dir: Path) =
        runSwap("taproot_rpc", dir, Backend.RPC, "Taproot", "P2TR")

    @Test
    fun legacyElectrumSwap(@TempDir dir: Path) =
        runSwap("legacy_electrum", dir, Backend.ELECTRUM, "Legacy", "P2WPKH")

    @Test
    fun taprootElectrumSwap(@TempDir dir: Path) =
        runSwap("taproot_electrum", dir, Backend.ELECTRUM, "Taproot", "P2TR")
}
