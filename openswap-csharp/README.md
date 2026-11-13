<div align="center">

# Openswap C# Bindings

C# / .NET bindings for the Openswap taker API

[![License](https://img.shields.io/badge/license-MIT-blue.svg)](../ffi-commons/LICENSE)
[![.NET 8](https://img.shields.io/badge/.NET-8.0-512BD4.svg)](https://dotnet.microsoft.com/)

</div>

## Overview

`openswap-csharp` packages the shared [`ffi-commons`](../ffi-commons) Rust core as a .NET library. The managed surface is generated with [NordSecurity/uniffi-bindgen-cs](https://github.com/NordSecurity/uniffi-bindgen-cs) and wrapped by a small, stable hand-written API.

Like the other language packages in this repo, the build model is **package-local**: this directory owns its build scripts, generated bindings, staged native libraries, and packaging.

## Layout

```
openswap-csharp/
├── Openswap/
│   ├── Openswap.csproj          # class library, PackageId CitadelTech.Openswap
│   ├── OpenswapClient.cs        # hand-written stable public wrapper
│   ├── Generated/openswap.cs    # generated bindings (namespace Openswap.Native) — git-ignored
│   └── runtimes/<rid>/native/   # staged native libs (git-ignored; built per platform)
├── Openswap.Tests/              # xUnit live swap test
├── build-scripts/
│   ├── build-macos.sh
│   ├── build-linux.sh
│   └── build-windows.sh
└── README.md
```

## Requirements

- **.NET SDK 8.0+**
- **Rust 1.88+** — required by `uniffi-bindgen-cs`; higher than the repo-wide 1.75 minimum.
- **UniFFI 0.31** — `ffi-commons` is pinned to `uniffi 0.31`. The C# generator's version **must match** the `uniffi` crate version, so use the `v0.11.0+v0.31.0` generator tag.

Install the generator:

```bash
cargo install uniffi-bindgen-cs \
  --git https://github.com/NordSecurity/uniffi-bindgen-cs \
  --tag v0.11.0+v0.31.0
```

## Build & test

```bash
# 1. Build the native library for your host and stage it under runtimes/<rid>/native
build-scripts/build-macos.sh          # or build-linux.sh / build-windows.sh

# 2. (Re)generate the managed bindings from that library. Run from ffi-commons so
#    the generator can resolve external packages via `cargo metadata`.
(cd ../ffi-commons && uniffi-bindgen-cs \
  --library target/aarch64-apple-darwin/release-smaller/libopenswap_ffi.dylib \
  --config uniffi.toml --out-dir ../openswap-csharp/Openswap/Generated --no-format)

# 3. Build and run the tests (needs the docker regtest stack running)
dotnet test Openswap.Tests/Openswap.Tests.csproj
```

The live swap test drives a full 2-maker Legacy openswap against the docker regtest stack (start it with `cd ../ffi-commons && ./ffi-docker-setup start 4`), mirroring `openswap-python/test/standard_swap.py`.

## Usage

Consumers should target the hand-written `Openswap.OpenswapClient`, not the generated `Openswap.Native.*` surface (which is regenerated whenever `ffi-commons` changes).

```csharp
using Openswap;

Console.WriteLine(OpenswapClient.NativeVersion);

var rpc = OpenswapClient.DefaultRpcConfig() with { Password = "secret" };

using var client = OpenswapClient.Init(
    zmqAddr: "tcp://127.0.0.1:28332",
    rpcConfig: rpc);

// Every taker call is a blocking Rust call; use the *Async helpers off a UI thread.
await client.SyncWalletAsync();
await client.SyncOfferBookAsync();

var balances = await client.GetBalancesAsync();
Console.WriteLine($"regular={balances.Regular} swap={balances.Swap}");
```

PSA : `OpenswapClient` is `IDisposable` — always dispose it (a `using` statement is simplest). Relying on the finalizer can leave the wallet directory locked and Tor/RPC connections alive.

### Seed phrase and restore

A wallet made before phrases were stored has none to read. Attach the phrase the user already has first:

```csharp
client.AttachSeedPhrase(walletPassword, usersOriginalPhrase);
```

Then read it back and confirm the backup:

```csharp
var phrase = client.GetSeedPhrase(walletPassword);   // show the 12 words to the user
client.ConfirmSeedBackup(walletPassword);            // user wrote the words down
var backedUp = client.IsSeedBackedUp();              // has the user confirmed the backup

OpenswapClient.ValidateSeedPhrase(phrase);           // check words and checksum
OpenswapClient.RestoreWalletFromSeedPhrase(dataDir, "restored", backendConfig, phrase, password);
OpenswapClient.RestoreWalletFromBackup(dataDir, null, backendConfig, backupPath, password);
```

Restore throws `TakerException.Wallet` when the restore fails and `TakerException.General` for invalid backend settings or bond hints. Show the seed phrase once and never log or store it. .NET keeps its own copy of the string, which this library cannot wipe. Restore needs a reachable node and cannot bring back open swaps.

### Async note

The exported Rust API is synchronous, so the generated methods block. The `*Async` helpers offload the work to the thread pool so callers are not blocked, but they do **not** make the underlying operation cancellable — a `CancellationToken` only stops the task from being observed, the native call runs to completion. If genuine cancellable async is needed, the right fix is to expose `async` functions from `ffi-commons` (UniFFI maps those to C# `Task<T>` directly).

## Packaging (NuGet)

Native libraries are packed under `runtimes/<rid>/native/` so NuGet's native asset resolution drops the correct `.dylib`/`.so`/`.dll` next to a consuming app. The generated bindings reference the neutral name `openswap_ffi`; the .NET native loader maps it to the platform extension automatically.

```bash
dotnet pack Openswap/Openswap.csproj -c Release   # produces CitadelTech.Openswap.<version>.nupkg
```

Supported runtime identifiers: `osx-arm64`, `osx-x64`, `linux-x64`, `linux-arm64`, `win-x64`, `win-arm64`. Stage each with the matching build script before packing a cross-platform package.

## Resources

- [ffi-commons](../ffi-commons) — shared Rust/UniFFI core
- [UniFFI](https://mozilla.github.io/uniffi-rs/)
- [uniffi-bindgen-cs](https://github.com/NordSecurity/uniffi-bindgen-cs)
