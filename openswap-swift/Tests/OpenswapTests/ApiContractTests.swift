import Foundation
import Openswap
import XCTest

/// Network-free checks for the values and errors that cross the Swift FFI boundary.
final class ApiContractTests: XCTestCase {
    func testNativeDefaultsAndValidationErrors() throws {
        XCTAssertEqual(openswapFfiVersion(), "0.1.0")
        XCTAssertEqual(
            createDefaultRpcConfig(),
            RpcConfig(
                url: "http://127.0.0.1:38332",
                username: "user",
                password: "password",
                walletName: "openswap_wallet"
            )
        )
        let missingWallet = URL(fileURLWithPath: NSTemporaryDirectory())
            .appendingPathComponent("openswap-\(UUID().uuidString)").path
        XCTAssertFalse(try isWalletEncrypted(walletPath: missingWallet))

        let invalidBackend = BackendConfig(
            kind: "invalid", url: nil, username: nil, password: nil,
            walletName: nil, zmqAddr: nil, socks5: nil, timeout: nil,
            pollIntervalSecs: nil, maxRetries: nil)
        XCTAssertThrowsError(try Taker.`init`(
            dataDir: nil,
            walletFileName: nil,
            rpcConfig: nil,
            controlPort: nil,
            torAuthPassword: nil,
            zmqAddr: "tcp://127.0.0.1:28332",
            password: nil,
            nostrRelays: [],
            backendConfig: invalidBackend
        )) { error in
            XCTAssertEqual(
                error as? TakerError,
                .General(msg: "Invalid backend kind: invalid (expected rpc or electrum)"))
        }
    }

    func testScalarAndWalletRecordsPreserveEveryField() {
        XCTAssertEqual(Address(addr: "bc1qexample").addr, "bc1qexample")
        XCTAssertEqual(AddressType(addrType: "P2TR").addrType, "P2TR")
        XCTAssertEqual(SignedAmountSats(sats: .min).sats, .min)
        XCTAssertEqual(ScriptBuf(hex: "0051ff").hex, "0051ff")

        let txid = Txid(value: String(repeating: "01", count: 32))
        let keyBytes = Data((0..<33).map { UInt8($0) })
        XCTAssertEqual(PublicKey(compressed: true, inner: keyBytes).inner, keyBytes)

        let backend = BackendConfig(
            kind: "electrum",
            url: "ssl://electrum.example:50002",
            username: nil,
            password: nil,
            walletName: nil,
            zmqAddr: nil,
            socks5: "127.0.0.1:9050",
            timeout: 120,
            pollIntervalSecs: 15,
            maxRetries: 8)
        XCTAssertEqual(backend.kind, "electrum")
        XCTAssertEqual(backend.url, "ssl://electrum.example:50002")
        XCTAssertEqual(backend.socks5, "127.0.0.1:9050")
        XCTAssertEqual(backend.timeout, 120)
        XCTAssertEqual(backend.pollIntervalSecs, 15)
        XCTAssertEqual(backend.maxRetries, 8)

        let info = WalletTxInfo(
            confirmations: -1,
            blockhash: String(repeating: "02", count: 32),
            blockindex: 3,
            blocktime: 1_700_000_000,
            blockheight: 250,
            txid: txid,
            time: 1_700_000_001,
            timereceived: 1_700_000_002,
            bip125Replaceable: "Yes",
            walletConflicts: [Txid(value: String(repeating: "03", count: 32))])
        let detail = GetTransactionResultDetail(
            address: Address(addr: "bc1qtransaction"),
            category: "Send",
            amount: SignedAmountSats(sats: -50_000),
            label: "payment",
            vout: 2,
            fee: SignedAmountSats(sats: -250),
            abandoned: false)
        XCTAssertEqual(
            ListTransactionResult(info: info, detail: detail, trusted: true, comment: "memo"),
            ListTransactionResult(info: info, detail: detail, trusted: true, comment: "memo"))

        let unspent = ListUnspentResultEntry(
            txid: txid,
            vout: 4,
            address: "bc1qutxo",
            label: "seed",
            scriptPubKey: ScriptBuf(hex: "0014"),
            amount: Amount(sats: 75_000),
            confirmations: 6,
            redeemScript: ScriptBuf(hex: "51"),
            witnessScript: nil,
            spendable: true,
            solvable: true,
            desc: "wpkh(...)",
            safe: false)
        let spendInfo = UtxoSpendInfo(
            spendType: "FidelityBondCoin",
            path: "m/84'/1'/0'/0/1",
            multisigRedeemscript: nil,
            inputValue: Amount(sats: 75_000),
            index: 9)
        let total = TotalUtxoInfo(
            listUnspentResultEntry: unspent, utxoSpendInfo: spendInfo)
        XCTAssertEqual(total.listUnspentResultEntry, unspent)
        XCTAssertEqual(total.utxoSpendInfo, spendInfo)

        let balances = Balances(regular: 1, swap: 2, contract: 3, fidelity: 4, spendable: 3)
        XCTAssertEqual(
            [balances.regular, balances.swap, balances.contract,
             balances.fidelity, balances.spendable],
            [1, 2, 3, 4, 3])
        let feeRates = FeeRates(fastest: 12.5, standard: 6.25, economy: 1.0)
        XCTAssertEqual([feeRates.fastest, feeRates.standard, feeRates.economy],
                       [12.5, 6.25, 1.0])
        let lockTime = LockTime(lockType: "Blocks", value: 144)
        XCTAssertEqual(lockTime.lockType, "Blocks")
        XCTAssertEqual(lockTime.value, 144)
        XCTAssertEqual(MakerAddress(address: "maker.onion:6102").address,
                       "maker.onion:6102")
        let makerState = MakerState(stateType: "Unresponsive", retries: 7)
        XCTAssertEqual(makerState.stateType, "Unresponsive")
        XCTAssertEqual(makerState.retries, 7)
        XCTAssertEqual(MakerProtocol(protocolType: "Unified").protocolType, "Unified")
    }

    func testOfferAndSwapRecordsPreserveTheCompleteNestedGraph() {
        let outpoint = OutPoint(
            txid: Txid(value: String(repeating: "04", count: 32)), vout: 5)
        let publicKey = PublicKey(compressed: true, inner: Data(repeating: 2, count: 33))
        let bond = FidelityBond(
            outpoint: outpoint,
            amount: Amount(sats: 100_000),
            lockTime: LockTime(lockType: "Seconds", value: 500_000_000),
            pubkey: publicKey,
            confHeight: 100,
            certExpiry: nil,
            isSpent: false)
        let proof = FidelityProof(
            bond: bond,
            certHash: Data(repeating: 5, count: 32),
            certSig: Data(repeating: 6, count: 64))
        let offer = Offer(
            baseFee: -5,
            amountRelativeFeePct: 0.125,
            timeRelativeFeePct: 0.25,
            requiredConfirms: 2,
            minimumLocktime: 48,
            maxSize: 2_000_000,
            minSize: 50_000,
            tweakablePoint: publicKey,
            fidelity: proof)
        let candidate = MakerOfferCandidate(
            address: MakerAddress(address: "maker.onion:6102"),
            offer: offer,
            state: MakerState(stateType: "Good", retries: nil),
            protocol: MakerProtocol(protocolType: "Taproot"))
        XCTAssertEqual(OfferBook(makers: [candidate]).makers, [candidate])
        XCTAssertEqual(candidate.offer?.fidelity.certSig, Data(repeating: 6, count: 64))

        let params = SwapParams(
            protocol: "Taproot",
            sendAmount: 500_000,
            makerCount: 2,
            txCount: 3,
            requiredConfirms: 4,
            manuallySelectedOutpoints: [outpoint],
            preferredMakers: ["maker.onion:6102"],
            paymentAddress: nil,
            maxInputBudget: 5,
            feerate: 6)
        XCTAssertEqual(params.protocol, "Taproot")
        XCTAssertEqual(params.sendAmount, 500_000)
        XCTAssertEqual(params.makerCount, 2)
        XCTAssertEqual(params.txCount, 3)
        XCTAssertEqual(params.requiredConfirms, 4)
        XCTAssertEqual(params.manuallySelectedOutpoints, [outpoint])
        XCTAssertEqual(params.preferredMakers, ["maker.onion:6102"])
        XCTAssertNil(params.paymentAddress)
        XCTAssertEqual(params.maxInputBudget, 5)
        XCTAssertEqual(params.feerate, 6)

        let fee = MakerFeeInfo(
            makerIndex: 1,
            makerAddress: "maker.onion:6102",
            baseFee: 100,
            amountRelativeFee: 200,
            timeRelativeFee: 300,
            totalFee: 600)
        let change = UtxoWithAddress(amount: 25_000, address: "bc1qchange")
        let swapOutput = UtxoWithAddress(amount: 475_000, address: "bc1qswap")
        let outgoingUtxo = ReportUtxo(address: "bc1qoutgoing", value: 510_000)
        let incomingUtxo = ReportUtxo(address: "bc1qincoming", value: 475_000)
        let report = SwapReport(
            swapId: "swap-1",
            role: "Taker",
            status: "SUCCESS",
            swapDurationSeconds: 12.5,
            startTimestamp: 1_700_000_000,
            endTimestamp: 1_700_000_013,
            network: "regtest",
            errorMessage: nil,
            incomingAmount: 500_000,
            outgoingAmount: 510_000,
            feePaid: -10_000,
            outgoingUtxos: [outgoingUtxo],
            incomingUtxos: [incomingUtxo],
            fundingTxids: [[String(repeating: "08", count: 32)],
                           [String(repeating: "09", count: 32)]],
            makersCount: 2,
            makerAddresses: ["maker-1", "maker-2"],
            totalMakerFees: 8_000,
            miningFee: 2_000,
            feePercentage: 2,
            makerFeeInfo: [fee],
            inputUtxos: [510_000],
            outputChangeAmounts: [25_000],
            outputSwapAmounts: [475_000],
            outputChangeUtxos: [change],
            outputSwapUtxos: [swapOutput])
        XCTAssertEqual(report.swapId, "swap-1")
        XCTAssertEqual(report.feePaid, -10_000)
        XCTAssertEqual(report.outgoingUtxos, [outgoingUtxo])
        XCTAssertEqual(report.incomingUtxos, [incomingUtxo])
        XCTAssertEqual(report.fundingTxids.count, 2)
        XCTAssertEqual(report.makersCount, 2)
        XCTAssertEqual(report.makerFeeInfo, [fee])
        XCTAssertEqual(report.outputChangeUtxos, [change])
        XCTAssertEqual(report.outputSwapUtxos, [swapOutput])
    }

    func testEnumAndErrorVariantsRemainDistinctAndKeepMessages() {
        let behaviors: Set<TakerBehavior> = [
            .normal,
            .dropConnectionAfterFullSetup,
            .broadcastContractAfterFullSetup,
        ]
        XCTAssertEqual(behaviors.count, 3)

        let errors: Set<TakerError> = [
            .Wallet(msg: "reason"),
            .Protocol(msg: "reason"),
            .Network(msg: "reason"),
            .General(msg: "reason"),
            .Io(msg: "reason"),
        ]
        XCTAssertEqual(errors.count, 5)
    }
}
