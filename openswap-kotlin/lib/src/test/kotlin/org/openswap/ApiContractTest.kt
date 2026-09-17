package org.openswap

import org.junit.jupiter.api.Test
import java.util.UUID
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertIs
import kotlin.test.assertNull
import kotlin.test.assertTrue
import kotlin.test.fail

/** Network-free contract tests for the generated Kotlin binding. */
class ApiContractTest {

    @Test
    fun takerMethodSurfaceIsComplete() {
        val expected = setOf(
            "backup",
            "displayOffer",
            "fetchAllMakers",
            "fetchOffers",
            "getBalances",
            "getNextExternalAddress",
            "getNextInternalAddresses",
            "getTransactions",
            "getWalletName",
            "listAllUtxoSpendInfo",
            "lockUnspendableUtxos",
            "pollMaker",
            "prepareOpenswap",
            "recoverActiveSwap",
            "removeMaker",
            "sendToAddress",
            "setupLogging",
            "startOpenswap",
            "syncAndSave",
            "syncOfferbookAndWait",
            "verifyDeniability",
        )
        // Kotlin value classes such as UInt add a generated "-<hash>" suffix to
        // JVM method names; compare the public Kotlin names represented by them.
        val actual = Taker::class.java.methods
            .mapTo(mutableSetOf()) { it.name.substringBefore('-') }

        assertEquals(emptySet(), expected - actual)

        val expectedGlobals = setOf(
            "createDefaultRpcConfig",
            "fetchMempoolFees",
            "isWalletEncrypted",
            "openswapFfiVersion",
            "restoreWalletGuiApp",
            "setupLogging",
        )
        val actualGlobals = Class.forName("org.openswap.OpenswapKt")
            .methods
            .mapTo(mutableSetOf()) { it.name }
        assertEquals(emptySet(), expectedGlobals - actualGlobals)
    }

    @Test
    fun nativeGlobalsAndTypedValidationErrors() {
        assertEquals("0.1.0", openswapFfiVersion())
        assertEquals(
            RpcConfig(
                url = "http://127.0.0.1:38332",
                username = "user",
                password = "password",
                walletName = "openswap_wallet",
            ),
            createDefaultRpcConfig(),
        )
        val missingWallet = "${System.getProperty("java.io.tmpdir")}/openswap-${UUID.randomUUID()}"
        assertFalse(isWalletEncrypted(missingWallet))

        val invalidBackend = BackendConfig(
            kind = "invalid",
            url = null,
            username = null,
            password = null,
            walletName = null,
            zmqAddr = null,
            socks5 = null,
            timeout = null,
            pollIntervalSecs = null,
            maxRetries = null,
        )
        val error = try {
            Taker.init(
                dataDir = null,
                walletFileName = null,
                rpcConfig = null,
                controlPort = null,
                torAuthPassword = null,
                zmqAddr = "tcp://127.0.0.1:28332",
                password = null,
                nostrRelays = emptyList(),
                backendConfig = invalidBackend,
                checkBlocklist = null,
            )
            fail("an unknown backend must be rejected before native resources are started")
        } catch (error: TakerException) {
            error
        }
        val general = assertIs<TakerException.General>(error)
        assertEquals("Invalid backend kind: invalid (expected rpc or electrum)", general.msg)
    }

    @Test
    fun scalarAndConfigurationRecordsPreserveEveryField() {
        assertEquals("bc1qexample", Address("bc1qexample").addr)
        assertEquals("P2TR", AddressType("P2TR").addrType)
        assertEquals(Long.MIN_VALUE, SignedAmountSats(Long.MIN_VALUE).sats)
        assertEquals("ab".repeat(32), Txid("ab".repeat(32)).value)
        assertEquals("0051ff", ScriptBuf("0051ff").hex)

        val keyBytes = ByteArray(33) { it.toByte() }
        val publicKey = PublicKey(compressed = true, inner = keyBytes)
        assertTrue(publicKey.compressed)
        assertContentEquals(keyBytes, publicKey.inner)

        val backend = BackendConfig(
            kind = "electrum",
            url = "ssl://electrum.example:50002",
            username = null,
            password = null,
            walletName = null,
            zmqAddr = null,
            socks5 = "127.0.0.1:9050",
            timeout = 120u.toUByte(),
            pollIntervalSecs = 15uL,
            maxRetries = 8u.toUByte(),
        )
        assertEquals("electrum", backend.kind)
        assertEquals("ssl://electrum.example:50002", backend.url)
        assertNull(backend.username)
        assertNull(backend.password)
        assertNull(backend.walletName)
        assertNull(backend.zmqAddr)
        assertEquals("127.0.0.1:9050", backend.socks5)
        assertEquals(120u.toUByte(), backend.timeout)
        assertEquals(15uL, backend.pollIntervalSecs)
        assertEquals(8u.toUByte(), backend.maxRetries)

        val balances = Balances(regular = 1, swap = 2, contract = 3, fidelity = 4, spendable = 3)
        assertEquals(listOf(1L, 2L, 3L, 4L, 3L), listOf(
            balances.regular,
            balances.swap,
            balances.contract,
            balances.fidelity,
            balances.spendable,
        ))
        val feeRates = FeeRates(12.5, 6.25, 1.0)
        assertEquals(listOf(12.5, 6.25, 1.0), listOf(
            feeRates.fastest,
            feeRates.standard,
            feeRates.economy,
        ))
        assertEquals("Blocks", LockTime("Blocks", 144u).lockType)
        assertEquals(144u, LockTime("Blocks", 144u).value)
        assertEquals("maker.onion:6102", MakerAddress("maker.onion:6102").address)
        val makerState = MakerState("Unresponsive", 7u.toUByte())
        assertEquals("Unresponsive", makerState.stateType)
        assertEquals(7u.toUByte(), makerState.retries)
        assertEquals("Unified", MakerProtocol("Unified").protocolType)
        val change = UtxoWithAddress(50_000, "bc1qchange")
        assertEquals(50_000L, change.amount)
        assertEquals("bc1qchange", change.address)
    }

    @Test
    fun transactionAndUtxoRecordsPreserveNestedOptionalData() {
        val txid = Txid("01".repeat(32))
        val info = WalletTxInfo(
            confirmations = -1,
            blockhash = "02".repeat(32),
            blockindex = 3u,
            blocktime = 1_700_000_000,
            blockheight = 250u,
            txid = txid,
            time = 1_700_000_001,
            timereceived = 1_700_000_002,
            bip125Replaceable = "Yes",
            walletConflicts = listOf(Txid("03".repeat(32))),
        )
        val detail = GetTransactionResultDetail(
            address = Address("bc1qtransaction"),
            category = "Send",
            amount = SignedAmountSats(-50_000),
            label = "payment",
            vout = 2u,
            fee = SignedAmountSats(-250),
            abandoned = false,
        )
        val transaction = ListTransactionResult(info, detail, trusted = true, comment = "memo")
        assertEquals(info, transaction.info)
        assertEquals(detail, transaction.detail)
        assertEquals(true, transaction.trusted)
        assertEquals("memo", transaction.comment)

        val unspent = ListUnspentResultEntry(
            txid = txid,
            vout = 4u,
            address = "bc1qutxo",
            label = "seed",
            scriptPubKey = ScriptBuf("0014"),
            amount = Amount(75_000),
            confirmations = 6u,
            redeemScript = ScriptBuf("51"),
            witnessScript = null,
            spendable = true,
            solvable = true,
            desc = "wpkh(...)",
            safe = false,
        )
        val spendInfo = UtxoSpendInfo(
            spendType = "FidelityBondCoin",
            path = "m/84'/1'/0'/0/1",
            multisigRedeemscript = null,
            inputValue = Amount(75_000),
            index = 9u,
        )
        val total = TotalUtxoInfo(unspent, spendInfo)
        assertEquals(unspent, total.listUnspentResultEntry)
        assertEquals(spendInfo, total.utxoSpendInfo)
    }

    @Test
    fun offerAndSwapRecordsPreserveTheCompleteNestedGraph() {
        val outpoint = OutPoint(Txid("04".repeat(32)), 5u)
        val publicKey = PublicKey(true, ByteArray(33) { 2 })
        val bond = FidelityBond(
            outpoint = outpoint,
            amount = Amount(100_000),
            lockTime = LockTime("Seconds", 500_000_000u),
            pubkey = publicKey,
            confHeight = 100u,
            certExpiry = null,
            isSpent = false,
        )
        val proof = FidelityProof(bond, ByteArray(32) { 5 }, ByteArray(64) { 6 })
        val offer = Offer(
            baseFee = -5,
            amountRelativeFeePct = 0.125,
            timeRelativeFeePct = 0.25,
            requiredConfirms = 2u,
            minimumLocktime = 48u.toUShort(),
            maxSize = 2_000_000,
            minSize = 50_000,
            tweakablePoint = publicKey,
            fidelity = proof,
        )
        val candidate = MakerOfferCandidate(
            address = MakerAddress("maker.onion:6102"),
            offer = offer,
            state = MakerState("Good", null),
            protocol = MakerProtocol("Taproot"),
        )
        val offerBook = OfferBook(listOf(candidate))
        assertEquals(candidate, offerBook.makers.single())
        assertEquals(-5L, offerBook.makers.single().offer?.baseFee)
        assertContentEquals(ByteArray(64) { 6 }, offerBook.makers.single().offer!!.fidelity.certSig)

        val params = SwapParams(
            protocol = "Taproot",
            sendAmount = 500_000uL,
            makerCount = 2u,
            txCount = 3u,
            requiredConfirms = 4u,
            manuallySelectedOutpoints = listOf(outpoint),
            preferredMakers = listOf("maker.onion:6102"),
            paymentAddress = null,
        )
        assertEquals("Taproot", params.protocol)
        assertEquals(500_000uL, params.sendAmount)
        assertEquals(2u, params.makerCount)
        assertEquals(3u, params.txCount)
        assertEquals(4u, params.requiredConfirms)
        assertEquals(listOf(outpoint), params.manuallySelectedOutpoints)
        assertEquals(listOf("maker.onion:6102"), params.preferredMakers)
        assertNull(params.paymentAddress)

        val fee = MakerFeeInfo(1u, "maker.onion:6102", 100.0, 200.0, 300.0, 600.0)
        val change = UtxoWithAddress(25_000, "bc1qchange")
        val swapOutput = UtxoWithAddress(475_000, "bc1qswap")
        val outgoingUtxo = ReportUtxo("bc1qoutgoing", 510_000)
        val incomingUtxo = ReportUtxo("bc1qincoming", 475_000)
        val report = SwapReport(
            swapId = "swap-1",
            role = "Taker",
            status = "SUCCESS",
            swapDurationSeconds = 12.5,
            startTimestamp = 1_700_000_000,
            endTimestamp = 1_700_000_013,
            network = "regtest",
            errorMessage = null,
            incomingAmount = 500_000,
            outgoingAmount = 510_000,
            feePaid = -10_000,
            outgoingUtxos = listOf(outgoingUtxo),
            incomingUtxos = listOf(incomingUtxo),
            fundingTxids = listOf(listOf("08".repeat(32)), listOf("09".repeat(32))),
            makersCount = 2u,
            makerAddresses = listOf("maker-1", "maker-2"),
            totalMakerFees = 8_000,
            miningFee = 2_000,
            feePercentage = 2.0,
            makerFeeInfo = listOf(fee),
            inputUtxos = listOf(510_000),
            outputChangeAmounts = listOf(25_000),
            outputSwapAmounts = listOf(475_000),
            outputChangeUtxos = listOf(change),
            outputSwapUtxos = listOf(swapOutput),
        )
        assertEquals("swap-1", report.swapId)
        assertEquals("Taker", report.role)
        assertEquals("SUCCESS", report.status)
        assertEquals(12.5, report.swapDurationSeconds)
        assertEquals(1_700_000_000L, report.startTimestamp)
        assertEquals(1_700_000_013L, report.endTimestamp)
        assertEquals("regtest", report.network)
        assertNull(report.errorMessage)
        assertEquals(500_000L, report.incomingAmount)
        assertEquals(510_000L, report.outgoingAmount)
        assertEquals(-10_000L, report.feePaid)
        assertEquals(listOf(outgoingUtxo), report.outgoingUtxos)
        assertEquals(listOf(incomingUtxo), report.incomingUtxos)
        assertEquals(2, report.fundingTxids.size)
        assertEquals(2u, report.makersCount)
        assertEquals(listOf("maker-1", "maker-2"), report.makerAddresses)
        assertEquals(8_000L, report.totalMakerFees)
        assertEquals(2_000L, report.miningFee)
        assertEquals(2.0, report.feePercentage)
        assertEquals(listOf(fee), report.makerFeeInfo)
        assertEquals(listOf(510_000L), report.inputUtxos)
        assertEquals(listOf(25_000L), report.outputChangeAmounts)
        assertEquals(listOf(475_000L), report.outputSwapAmounts)
        assertEquals(listOf(change), report.outputChangeUtxos)
        assertEquals(listOf(swapOutput), report.outputSwapUtxos)
    }

    @Test
    fun enumAndErrorVariantsRemainDistinctAndKeepMessages() {
        assertEquals(3, TakerBehavior.entries.size)
        assertEquals(
            setOf(
                TakerBehavior.NORMAL,
                TakerBehavior.DROP_CONNECTION_AFTER_FULL_SETUP,
                TakerBehavior.BROADCAST_CONTRACT_AFTER_FULL_SETUP,
            ),
            TakerBehavior.entries.toSet(),
        )
        listOf(
            TakerException.Wallet("reason").msg,
            TakerException.Protocol("reason").msg,
            TakerException.Network("reason").msg,
            TakerException.General("reason").msg,
            TakerException.Io("reason").msg,
        ).forEach { assertEquals("reason", it) }
    }
}
