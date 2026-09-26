using Openswap.Native;
using Xunit;

namespace Openswap.Tests;

/// <summary>Network-free contracts for values and errors crossing the .NET FFI boundary.</summary>
public class ApiContractTest
{
    [Fact]
    public void TakerMethodSurfaceIsComplete()
    {
        string[] expected = [
            "Backup", "DisplayOffer", "FetchAllMakers", "FetchOffers", "GetBalances",
            "GetNextExternalAddress", "GetNextInternalAddresses", "GetTransactions",
            "GetWalletName", "ListAllUtxoSpendInfo", "LockUnspendableUtxos", "PollMaker",
            "PrepareOpenswap", "RecoverActiveSwap", "RemoveMaker", "SendToAddress",
            "SetupLogging", "StartOpenswap", "SyncAndSave", "SyncOfferbookAndWait",
            "VerifyDeniability",
        ];
        var actual = typeof(Taker).GetMethods()
            .Select(method => method.Name)
            .ToHashSet(StringComparer.Ordinal);

        Assert.Empty(expected.Where(method => !actual.Contains(method)));

        string[] expectedGlobals = [
            "CreateDefaultRpcConfig", "FetchMempoolFees", "IsWalletEncrypted",
            "OpenswapFfiVersion", "RestoreWalletGuiApp", "SetupLogging",
        ];
        var actualGlobals = typeof(global::Openswap.Native.Openswap).GetMethods()
            .Select(method => method.Name)
            .ToHashSet(StringComparer.Ordinal);
        Assert.Empty(expectedGlobals.Where(method => !actualGlobals.Contains(method)));
    }

    [Fact]
    public void NativeDefaultsAndValidationErrorsAreStable()
    {
        Assert.Equal("0.1.0", OpenswapClient.NativeVersion);
        Assert.Equal(
            new RpcConfig(
                Url: "http://127.0.0.1:38332",
                Username: "user",
                Password: "password",
                WalletName: "openswap_wallet"),
            OpenswapClient.DefaultRpcConfig());
        Assert.False(global::Openswap.Native.Openswap.IsWalletEncrypted(
            Path.Combine(Path.GetTempPath(), $"openswap-{Guid.NewGuid():N}")));

        var invalidBackend = new BackendConfig(
            Kind: "invalid", Url: null, Username: null, Password: null,
            WalletName: null, ZmqAddr: null, Socks5: null, Timeout: null,
            PollIntervalSecs: null, MaxRetries: null);
        var error = Assert.Throws<TakerException.General>(() => Taker.Init(
            null, null, null, null, null, "tcp://127.0.0.1:28332", null,
            Array.Empty<string>(), invalidBackend));
        Assert.Equal(
            "Invalid backend kind: invalid (expected rpc or electrum)",
            error.msg);
    }

    [Fact]
    public void ScalarConfigurationAndWalletRecordsPreserveEveryField()
    {
        Assert.Equal("bc1qexample", new Address("bc1qexample").Addr);
        Assert.Equal("P2TR", new AddressType("P2TR").AddrType);
        Assert.Equal(long.MinValue, new SignedAmountSats(long.MinValue).Sats);
        Assert.Equal("0051ff", new ScriptBuf("0051ff").Hex);

        var txid = new Txid(string.Concat(Enumerable.Repeat("01", 32)));
        var keyBytes = Enumerable.Range(0, 33).Select(i => (byte)i).ToArray();
        var publicKey = new PublicKey(Compressed: true, Inner: keyBytes);
        Assert.True(publicKey.Compressed);
        Assert.Equal(keyBytes, publicKey.Inner);

        var backend = new BackendConfig(
            Kind: "electrum",
            Url: "ssl://electrum.example:50002",
            Username: null,
            Password: null,
            WalletName: null,
            ZmqAddr: null,
            Socks5: "127.0.0.1:9050",
            Timeout: 120,
            PollIntervalSecs: 15,
            MaxRetries: 8);
        Assert.Equal("electrum", backend.Kind);
        Assert.Equal("ssl://electrum.example:50002", backend.Url);
        Assert.Equal("127.0.0.1:9050", backend.Socks5);
        Assert.Equal((byte?)120, backend.Timeout);
        Assert.Equal((ulong?)15, backend.PollIntervalSecs);
        Assert.Equal((byte?)8, backend.MaxRetries);

        var info = new WalletTxInfo(
            Confirmations: -1,
            Blockhash: string.Concat(Enumerable.Repeat("02", 32)),
            Blockindex: 3,
            Blocktime: 1_700_000_000,
            Blockheight: 250,
            Txid: txid,
            Time: 1_700_000_001,
            Timereceived: 1_700_000_002,
            Bip125Replaceable: "Yes",
            WalletConflicts: [new Txid(string.Concat(Enumerable.Repeat("03", 32)))]);
        var detail = new GetTransactionResultDetail(
            Address: new Address("bc1qtransaction"),
            Category: "Send",
            Amount: new SignedAmountSats(-50_000),
            Label: "payment",
            Vout: 2,
            Fee: new SignedAmountSats(-250),
            Abandoned: false);
        var transaction = new ListTransactionResult(info, detail, true, "memo");
        Assert.Equal(info, transaction.Info);
        Assert.Equal(detail, transaction.Detail);
        Assert.True(transaction.Trusted is true);
        Assert.Equal("memo", transaction.Comment);

        var unspent = new ListUnspentResultEntry(
            Txid: txid,
            Vout: 4,
            Address: "bc1qutxo",
            Label: "seed",
            ScriptPubKey: new ScriptBuf("0014"),
            Amount: new Amount(75_000),
            Confirmations: 6,
            RedeemScript: new ScriptBuf("51"),
            WitnessScript: null,
            Spendable: true,
            Solvable: true,
            Desc: "wpkh(...)",
            Safe: false);
        var spendInfo = new UtxoSpendInfo(
            SpendType: "FidelityBondCoin",
            Path: "m/84'/1'/0'/0/1",
            MultisigRedeemscript: null,
            InputValue: new Amount(75_000),
            Index: 9);
        var total = new TotalUtxoInfo(unspent, spendInfo);
        Assert.Equal(unspent, total.ListUnspentResultEntry);
        Assert.Equal(spendInfo, total.UtxoSpendInfo);

        var balances = new Balances(1, 2, 3, 4, 3);
        Assert.Equal(
            new long[] { 1, 2, 3, 4, 3 },
            new[] { balances.Regular, balances.Swap, balances.Contract, balances.Fidelity, balances.Spendable });
        var lockTime = new LockTime("Blocks", 144);
        Assert.Equal("Blocks", lockTime.LockType);
        Assert.Equal((uint)144, lockTime.Value);
        Assert.Equal("maker.onion:6102", new MakerAddress("maker.onion:6102").Address);
        var unavailable = new UnavailableState("NoOfferResponse", 100, 200, 7);
        var makerState = new MakerState("Unavailable", unavailable, null);
        Assert.Equal("Unavailable", makerState.StateType);
        Assert.Equal("NoOfferResponse", makerState.Unavailable?.Reason);
        Assert.Equal((ulong?)100, makerState.Unavailable?.SinceTs);
        Assert.Equal((ulong?)200, makerState.Unavailable?.LastAttemptTs);
        Assert.Equal((uint?)7, makerState.Unavailable?.Attempts);
        var banned = new MakerState(
            "Banned", null, new BanRecord("InvalidFidelityProof", 300));
        Assert.Equal("InvalidFidelityProof", banned.Ban?.Reason);
        Assert.Equal((ulong?)300, banned.Ban?.RecordedAtTs);
        Assert.Equal("Unified", new MakerProtocol("Unified").ProtocolType);
    }

    [Fact]
    public void OfferAndSwapRecordsPreserveTheCompleteNestedGraph()
    {
        var outpoint = new OutPoint(
            new Txid(string.Concat(Enumerable.Repeat("04", 32))), 5);
        var publicKey = new PublicKey(true, Enumerable.Repeat((byte)2, 33).ToArray());
        var bond = new FidelityBond(
            Outpoint: outpoint,
            Amount: new Amount(100_000),
            LockTime: new LockTime("Seconds", 500_000_000),
            Pubkey: publicKey,
            ConfHeight: 100,
            CertExpiry: null,
            IsSpent: false);
        var proof = new FidelityProof(
            Bond: bond,
            CertHash: Enumerable.Repeat((byte)5, 32).ToArray(),
            CertSig: Enumerable.Repeat((byte)6, 64).ToArray());
        var offer = new Offer(
            BaseFee: -5,
            AmountRelativeFeePct: 0.125,
            TimeRelativeFeePct: 0.25,
            RequiredConfirms: 2,
            MinimumLocktime: 48,
            MaxSize: 2_000_000,
            MinSize: 50_000,
            TweakablePoint: publicKey,
            Fidelity: proof);
        var candidate = new MakerOfferCandidate(
            Address: new MakerAddress("maker.onion:6102"),
            Offer: offer,
            State: new MakerState("Good", null, null),
            Protocol: new MakerProtocol("Taproot"));
        Assert.Equal(candidate, new OfferBook([candidate]).Makers.Single());
        Assert.Equal(Enumerable.Repeat((byte)6, 64), candidate.Offer!.Fidelity.CertSig);

        var parameters = new SwapParams(
            Protocol: "Taproot",
            SendAmount: 500_000,
            MakerCount: 2,
            TxCount: 3,
            RequiredConfirms: 4,
            ManuallySelectedOutpoints: [outpoint],
            PreferredMakers: ["maker.onion:6102"],
            PaymentAddress: null,
            MaxInputBudget: 5,
            Feerate: 6);
        Assert.Equal("Taproot", parameters.Protocol);
        Assert.Equal((ulong)500_000, parameters.SendAmount);
        Assert.Equal((uint)2, parameters.MakerCount);
        Assert.Equal((uint?)3, parameters.TxCount);
        Assert.Equal((uint?)4, parameters.RequiredConfirms);
        Assert.Equal(outpoint, parameters.ManuallySelectedOutpoints!.Single());
        Assert.Equal("maker.onion:6102", parameters.PreferredMakers!.Single());
        Assert.Null(parameters.PaymentAddress);
        Assert.Equal((uint?)5, parameters.MaxInputBudget);
        Assert.Equal((ulong?)6, parameters.Feerate);

        var fee = new MakerFeeInfo(1, "maker.onion:6102", 100, 200, 300, 600);
        var change = new UtxoWithAddress(25_000, "bc1qchange");
        var swapOutput = new UtxoWithAddress(475_000, "bc1qswap");
        var outgoingUtxo = new ReportUtxo("bc1qoutgoing", 510_000);
        var incomingUtxo = new ReportUtxo("bc1qincoming", 475_000);
        var report = new SwapReport(
            SwapId: "swap-1",
            Role: "Taker",
            Status: "SUCCESS",
            SwapDurationSeconds: 12.5,
            StartTimestamp: 1_700_000_000,
            EndTimestamp: 1_700_000_013,
            Network: "regtest",
            ErrorMessage: null,
            IncomingAmount: 500_000,
            OutgoingAmount: 510_000,
            FeePaid: -10_000,
            OutgoingUtxos: [outgoingUtxo],
            IncomingUtxos: [incomingUtxo],
            FundingTxids: [
                [string.Concat(Enumerable.Repeat("08", 32))],
                [string.Concat(Enumerable.Repeat("09", 32))],
            ],
            MakersCount: 2,
            MakerAddresses: ["maker-1", "maker-2"],
            TotalMakerFees: 8_000,
            MiningFee: 2_000,
            FeePercentage: 2,
            MakerFeeInfo: [fee],
            InputUtxos: [510_000],
            OutputChangeAmounts: [25_000],
            OutputSwapAmounts: [475_000],
            OutputChangeUtxos: [change],
            OutputSwapUtxos: [swapOutput]);
        Assert.Equal("swap-1", report.SwapId);
        Assert.Equal(-10_000, report.FeePaid);
        Assert.Equal(outgoingUtxo, report.OutgoingUtxos.Single());
        Assert.Equal(incomingUtxo, report.IncomingUtxos.Single());
        Assert.Equal(2, report.FundingTxids.Length);
        Assert.Equal((uint?)2, report.MakersCount);
        Assert.Equal(fee, report.MakerFeeInfo.Single());
        Assert.Equal(change, report.OutputChangeUtxos.Single());
        Assert.Equal(swapOutput, report.OutputSwapUtxos.Single());
    }

    [Fact]
    public void EnumAndErrorVariantsRemainDistinctAndKeepMessages()
    {
        Assert.Equal(3, Enum.GetValues<TakerBehavior>().Length);
        Assert.Equal("reason", new TakerException.Wallet("reason").msg);
        Assert.Equal("reason", new TakerException.Protocol("reason").msg);
        Assert.Equal("reason", new TakerException.Network("reason").msg);
        Assert.Equal("reason", new TakerException.General("reason").msg);
        Assert.Equal("reason", new TakerException.Io("reason").msg);
    }
}
