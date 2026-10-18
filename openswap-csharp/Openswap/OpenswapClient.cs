using Openswap.Native;

namespace Openswap;

/// <summary>
/// Stable, .NET-friendly wrapper over the UniFFI-generated Openswap taker surface.
/// Consumers should target this instead of the generated <c>Openswap.Native.*</c> types,
/// which are regenerated (and may change shape) whenever <c>ffi-commons</c> changes.
/// Every taker call is a blocking Rust call; the <c>*Async</c> helpers offload them to the
/// thread pool but do not make the underlying work cancellable.
/// </summary>
public sealed class OpenswapClient : IDisposable
{
    private readonly Taker _inner;
    private bool _disposed;

    private OpenswapClient(Taker inner) => _inner = inner;

    /// <summary>Version string of the underlying openswap-ffi native library.</summary>
    public static string NativeVersion => Native.Openswap.OpenswapFfiVersion();

    /// <summary>Returns the default regtest/signet RPC configuration.</summary>
    public static RpcConfig DefaultRpcConfig() => Native.Openswap.CreateDefaultRpcConfig();

    /// <summary>
    /// Initializes a taker; any argument except <paramref name="zmqAddr"/> may be null to use the
    /// Rust-side default. <paramref name="nostrRelays"/> overrides the default discovery relays
    /// (pass a local relay to avoid public-relay rate limits).
    /// </summary>
    public static OpenswapClient Init(
        string zmqAddr,
        string? dataDir = null,
        string? walletFileName = null,
        RpcConfig? rpcConfig = null,
        ushort? controlPort = null,
        string? torAuthPassword = null,
        string? password = null,
        IEnumerable<string>? nostrRelays = null,
        BackendConfig? backendConfig = null)
    {
        var taker = Taker.Init(
            dataDir, walletFileName, rpcConfig, controlPort, torAuthPassword, zmqAddr, password,
            nostrRelays?.ToArray(), backendConfig);
        return new OpenswapClient(taker);
    }

    /// <summary>Configures the taker's logger (level: Trace/Debug/Info/Warn/Error).</summary>
    public void SetupLogging(string logLevel = "Info", string? dataDir = null) =>
        _inner.SetupLogging(dataDir, logLevel);

    /// <summary>Syncs the wallet with the chain and persists it to disk.</summary>
    public void SyncWallet() => _inner.SyncAndSave();

    /// <summary>Returns the next unused external receive address of the given type (e.g. "P2WPKH", "P2TR").</summary>
    public string GetNextExternalAddress(string addrType) =>
        _inner.GetNextExternalAddress(new AddressType(addrType)).Addr;

    /// <summary>Runs a full offerbook sync cycle and blocks until it completes.</summary>
    public void SyncOfferBook() => _inner.SyncOfferbookAndWait();

    /// <summary>Polls a single maker, verifies its fidelity proof, and returns its final state.</summary>
    public MakerOfferCandidate PollMaker(string address) => _inner.PollMaker(address);

    /// <summary>Removes a maker from the offerbook by address; returns true if an entry was removed.</summary>
    public bool RemoveMaker(string address) => _inner.RemoveMaker(address);

    /// <summary>Returns the current offerbook.</summary>
    public OfferBook FetchOffers() => _inner.FetchOffers();

    /// <summary>Returns the wallet's spendable/swap/contract/fidelity balances.</summary>
    public Balances GetBalances() => _inner.GetBalances();

    /// <summary>Returns the loaded wallet's name.</summary>
    public string GetWalletName() => _inner.GetWalletName();

    /// <summary>Renders a maker offer as a human-readable string.</summary>
    public string DisplayOffer(Offer makerOffer) => _inner.DisplayOffer(makerOffer);

    /// <summary>Selects makers and prepares a swap; returns the swap id.</summary>
    public string PrepareOpenswap(SwapParams swapParams) => _inner.PrepareOpenswap(swapParams);

    /// <summary>Executes the prepared swap and returns its report.</summary>
    public SwapReport StartOpenswap(string swapId) => _inner.StartOpenswap(swapId);

    /// <summary>Recovers coins from an interrupted or failed swap.</summary>
    public void RecoverActiveSwap() => _inner.RecoverActiveSwap();

    /// <summary>Verifies the deniability proof for a completed swap.</summary>
    public bool VerifyDeniability(string swapId) => _inner.VerifyDeniability(swapId);

    /// <summary>Locks UTXOs that cannot currently be spent.</summary>
    public void LockUnspendableUtxos() => _inner.LockUnspendableUtxos();

    /// <summary>Writes an encrypted wallet backup using the backup password and current wallet password.</summary>
    public void Backup(string destinationPath, string password, string? walletPassword) =>
        _inner.Backup(destinationPath, password, walletPassword);

    /// <summary>Off-thread <see cref="SyncWallet"/>.</summary>
    public Task SyncWalletAsync(CancellationToken ct = default) => Task.Run(SyncWallet, ct);

    /// <summary>Off-thread <see cref="SyncOfferBook"/>.</summary>
    public Task SyncOfferBookAsync(CancellationToken ct = default) => Task.Run(SyncOfferBook, ct);

    /// <summary>Off-thread <see cref="PollMaker"/>.</summary>
    public Task<MakerOfferCandidate> PollMakerAsync(string address, CancellationToken ct = default) =>
        Task.Run(() => PollMaker(address), ct);

    /// <summary>Off-thread <see cref="GetBalances"/>.</summary>
    public Task<Balances> GetBalancesAsync(CancellationToken ct = default) => Task.Run(GetBalances, ct);

    /// <summary>Off-thread <see cref="StartOpenswap"/>.</summary>
    public Task<SwapReport> StartOpenswapAsync(string swapId, CancellationToken ct = default) =>
        Task.Run(() => StartOpenswap(swapId), ct);

    /// <summary>Releases the native taker handle; call promptly rather than relying on the finalizer.</summary>
    public void Dispose()
    {
        if (_disposed) return;
        _disposed = true;
        _inner.Dispose();
    }
}
