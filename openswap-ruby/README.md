<div align="center">

# Openswap Ruby

Ruby bindings for the Openswap Bitcoin privacy protocol

</div>

## Overview

`openswap-ruby` exposes the shared UniFFI taker API to Ruby through a generated `openswap.rb` file and a native shared library.

## Supported Platforms

| Runtime platform | Native library |
| --- | --- |
| Linux x86_64 | `libopenswap_ffi.so` |
| Linux aarch64 | `libopenswap_ffi.so` |
| macOS x86_64 | `libopenswap_ffi.dylib` |
| macOS arm64 | `libopenswap_ffi.dylib` |

## Build Workflow

```bash
# Linux host
bash ./build-scripts/development/build-dev-linux-x86_64.sh
bash ./build-scripts/release/build-release-linux-x86_64.sh
bash ./build-scripts/release/build-release-linux-aarch64.sh

# macOS host
bash ./build-scripts/development/build-dev-macos-x86_64.sh
bash ./build-scripts/release/build-release-macos-x86_64.sh
bash ./build-scripts/release/build-release-macos-aarch64.sh
```

Each script builds the Rust library from `ffi-commons`, regenerates `openswap.rb`, and stages the native library at the package root so Ruby FFI can load it directly.

## Installation

### Direct usage

```ruby
$LOAD_PATH.unshift('/path/to/openswap-ffi/openswap-ruby')
require 'openswap'
```

### Bundler-managed application

```ruby
source 'https://rubygems.org'

gem 'ffi', '~> 1.15'
```

## Basic Usage

```ruby
require 'openswap'

# Bitcoin Core RPC settings used by the taker.
rpc_config = Openswap::RpcConfig.new(
  url: 'http://127.0.0.1:18442',           # Bitcoin Core RPC endpoint
  username: 'user',                        # Bitcoin Core RPC username
  password: 'password',                    # Bitcoin Core RPC password
  wallet_name: 'taker_wallet',             # Bitcoin Core wallet name
)

# Create or load the taker wallet.
taker = Openswap::Taker.init(
  '/path/to/data',                         # taker data directory; nil uses the default taker dir
  'taker_wallet',                          # taker wallet file to load or create
  rpc_config,                              # Bitcoin Core RPC settings
  9051,                                    # Tor control port
  'openswap',                              # Tor control password
  'tcp://127.0.0.1:28332',                 # Bitcoin Core ZMQ endpoint
  '',                                      # optional wallet encryption password
)

# Configure logging, sync wallet state, and wait for the offer book.
taker.setup_logging(
  '/path/to/data',                         # directory used for file logging
  'info',                                  # trace | debug | info | warn | error
)
taker.sync_and_save
taker.sync_offerbook_and_wait

# Inspect balances and derive a new receive address.
balances = taker.get_balances
receive_address = taker.get_next_external_address(
  Openswap::AddressType.new(
    addr_type: 'P2WPKH',                   # external address format to derive
  ),
)

puts "regular: #{balances.regular} sats"
puts "swap: #{balances.swap} sats"
puts "contract: #{balances.contract} sats"
puts "fidelity: #{balances.fidelity} sats"
puts "spendable: #{balances.spendable} sats"
puts "receive to: #{receive_address.addr}"

# Build the swap request exactly as the taker API expects it.
swap_params = Openswap::SwapParams.new(
  protocol: nil,                           # optional protocol hint; nil uses the backend default
  send_amount: 1_000_000,                  # total sats to swap
  maker_count: 2,                          # number of maker hops
  tx_count: 2,                             # maximum funding transaction splits per hop
  max_input_budget: 2,                     # inputs per forwarding tx whose fee the taker covers
  feerate: 1,                              # sats/vB used by every swap transaction
  required_confirms: 1,                    # minimum funding confirmations
  manually_selected_outpoints: nil,        # optional explicit wallet UTXOs
  preferred_makers: nil,                   # optional maker addresses to prefer
)

# Prepare the swap first, then start it with the returned swap id.
swap_id = taker.prepare_openswap(
  swap_params,                             # fully populated swap request
)
report = taker.start_openswap(
  swap_id,                                 # identifier returned by prepare_openswap
)

puts "swap id: #{report.swap_id}"
puts "status: #{report.status}"
puts "outgoing amount: #{report.outgoing_amount} sats"
puts "fee paid: #{report.fee_paid_or_earned.abs} sats"
```

## API Reference

### RpcConfig

```ruby
rpc_config = Openswap::RpcConfig.new(
  url: rpc_url,                            # Bitcoin Core RPC endpoint
  username: rpc_username,                  # Bitcoin Core RPC username
  password: rpc_password,                  # Bitcoin Core RPC password
  wallet_name: wallet_name,                # Bitcoin Core wallet name
)
```

### SwapParams

```ruby
swap_params = Openswap::SwapParams.new(
  protocol: protocol_hint,                 # optional protocol hint string
  send_amount: send_amount_sats,           # total sats to swap
  maker_count: maker_count,                # number of maker hops
  tx_count: tx_count,                      # maximum funding transaction splits per hop
  max_input_budget: max_input_budget,      # inputs per forwarding tx whose fee the taker covers
  feerate: feerate,                        # sats/vB used by every swap transaction
  required_confirms: required_confirms,    # minimum funding confirmations
  manually_selected_outpoints: outpoints,  # optional explicit wallet UTXOs
  preferred_makers: preferred_makers,      # optional maker addresses to prefer
)
```

### Taker

```ruby
taker = Openswap::Taker.init(
  data_dir,                                # taker data directory
  wallet_file_name,                        # taker wallet file to load or create
  rpc_config,                              # Bitcoin Core RPC settings
  control_port,                            # Tor control port
  tor_auth_password,                       # Tor control password
  zmq_addr,                                # Bitcoin Core ZMQ endpoint
  password,                                # optional wallet encryption password
)

taker.setup_logging(data_dir, log_level)                                      # configure taker logging
swap_id = taker.prepare_openswap(swap_params)                                 # prepare a swap and return the swap id
report = taker.start_openswap(swap_id)                                        # execute a prepared swap
txs = taker.get_transactions(count, skip)                                     # recent wallet transactions
internal = taker.get_next_internal_addresses(count, address_type)             # derive internal HD addresses
external = taker.get_next_external_address(address_type)                      # derive an external receive address
utxos = taker.list_all_utxo_spend_info                                        # wallet UTXOs plus spend metadata
taker.backup(destination_path, backup_password)                               # write a wallet backup JSON file
taker.lock_unspendable_utxos                                                  # lock fidelity and live-contract UTXOs
txid = taker.send_to_address(address, amount, fee_rate, outpoints)            # send sats to an external address
balances = taker.get_balances                                                 # read wallet balances
taker.sync_and_save                                                           # sync wallet state and persist it
taker.sync_offerbook_and_wait                                                 # block until the offer book is synchronized
offerbook = taker.fetch_offers                                                # read the current offer book
rendered_offer = taker.display_offer(offer)                                   # format a maker offer for display
wallet_name = taker.get_wallet_name                                           # read the wallet name
taker.recover_active_swap                                                     # resume recovery for a failed active swap
makers = taker.fetch_all_makers                                               # read maker addresses across all states
```

### AddressType, Balances, and SwapReport

```ruby
address_type = Openswap::AddressType.new(
  addr_type: 'P2WPKH',                   # external address format to derive
)

balances.regular                         # single-signature seed balance in sats
balances.swap                            # swap-coin balance in sats
balances.contract                        # live contract balance in sats
balances.fidelity                        # fidelity bond balance in sats
balances.spendable                       # regular + swap balance in sats

report.swap_id                           # unique swap identifier
report.role                              # report creator, usually Taker
report.status                            # swap terminal state
report.swap_duration_seconds             # execution duration in seconds
report.recovery_duration_seconds         # recovery duration in seconds
report.start_timestamp                   # unix start timestamp
report.end_timestamp                     # unix end timestamp
report.network                           # bitcoin network name
report.error_message                     # error detail, if present
report.incoming_amount                   # sats received by the taker
report.outgoing_amount                   # sats sent by the taker
report.fee_paid_or_earned                # negative when paid, positive when earned
report.funding_txids                     # funding txids grouped by hop
report.recovery_txids                    # recovery txids, if any
report.timelock                          # contract timelock in blocks
report.makers_count                      # maker hop count used in the swap
report.maker_addresses                   # maker addresses used in the route
report.total_maker_fees                  # aggregate maker fees in sats
report.mining_fee                        # mining fees in sats
report.fee_percentage                    # total fee as a percentage of amount
report.maker_fee_info                    # per-maker fee breakdown
report.input_utxos                       # input UTXO amounts in sats
report.output_change_amounts             # output change amounts in sats
report.output_swap_amounts               # output swap amounts in sats
report.output_change_utxos               # change outputs with amount and address
report.output_swap_utxos                 # swap outputs with amount and address
```

## Requirements

- Ruby 2.7 or newer.
- `ffi` gem.
- Bitcoin Core with RPC enabled, fully synced, non-pruned, and `-txindex` enabled.
- Tor daemon for live taker workflows.

## Support

- [Main Openswap Repository](https://github.com/citadel-foss/openswap)
- [FFI Commons](../ffi-commons)

## License

MIT License - see [LICENSE](../LICENSE) for details
