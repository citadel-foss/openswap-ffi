#!/usr/bin/env ruby
# frozen_string_literal: true

# FFI taker integration test: 4 takers x 2 makers.
#
# Mirrors the Rust/Python/Swift swap tests: one run drives four takers
# sequentially against the Docker regtest stack (1 RPC maker + 1 Electrum
# maker), covering the full backend x protocol matrix -- legacy/taproot over
# rpc/electrum. Each taker funds a fresh wallet and runs a 2-maker openswap.

require 'fileutils'
require 'open3'

# Add parent directory to load path for the openswap module
lib_path = File.expand_path('..', __dir__)
$LOAD_PATH.unshift(lib_path) unless $LOAD_PATH.include?(lib_path)

require 'openswap'

# Amount swapped by each taker, in sats. The taker is funded with 4x this.
SWAP_AMOUNT = 500_000
MAKER_COUNT = 2
MAKER_READY_ATTEMPTS = 3
MAKER_READY_RETRY_SECONDS = 10
MAKER_CONTAINERS = %w[openswap-makerd1 openswap-makerd2].freeze
WALLET_PASSWORD = 'ffi-live-test-wallet-password'

# (name, backend, protocol, addr_type)
SWAPS = [
  ['legacy_rpc',       'rpc',      'Legacy',  'P2WPKH'],
  ['taproot_rpc',      'rpc',      'Taproot', 'P2TR'],
  ['legacy_electrum',  'electrum', 'Legacy',  'P2WPKH'],
  ['taproot_electrum', 'electrum', 'Taproot', 'P2TR']
].freeze

def cleanup_wallet(wallet_name, data_dir)
  FileUtils.rm_rf(data_dir)

  begin
    system('docker', 'exec', 'openswap-bitcoind', 'bitcoin-cli', '-regtest',
           '-rpcport=18442', '-rpcuser=user', '-rpcpassword=password',
           'unloadwallet', wallet_name,
           out: File::NULL, err: File::NULL)
  rescue StandardError
    # Ignore missing wallet errors.
  end
end

def fund(address)
  stdout, stderr, status = Open3.capture3(
    'docker', 'exec', 'openswap-bitcoind', 'bitcoin-cli',
    '-regtest', '-rpcport=18442', '-rpcwallet=test',
    '-rpcuser=user', '-rpcpassword=password',
    'sendtoaddress', address, '0.25'
  )
  return if status.success?

  raise "Could not send BTC to #{address}: #{stdout}#{stderr}"
end

def local_maker_addresses
  marker = 'Generated new Tor Hidden Service Hostname:'

  MAKER_CONTAINERS.map do |container|
    stdout, stderr, status = Open3.capture3('docker', 'logs', container)
    raise "#{container}: failed to read maker logs: #{stderr}" unless status.success?

    matches = "#{stdout}\n#{stderr}".lines.filter_map do |line|
      line.split(marker, 2)[1]&.strip&.split&.first if line.include?(marker)
    end
    matches.last || raise("#{container}: maker onion address not found in logs")
  end
end

def wait_for_spendable(taker, target)
  # Sync until spendable reaches `target`, tolerating Electrum indexing lag.
  30.times do
    taker.sync_and_save
    balances = taker.get_balances
    return balances if balances.spendable >= target

    sleep(3)
  end
  taker.get_balances
end

def suitable_makers(offerbook, protocol)
  offerbook.makers.select do |maker|
    maker.state.state_type == 'Good' &&
      !maker.protocol.nil? && [protocol, 'Unified'].include?(maker.protocol.protocol_type) &&
      !maker.offer.nil? && maker.offer.min_size <= SWAP_AMOUNT && SWAP_AMOUNT <= maker.offer.max_size
  end
end

def wait_for_suitable_makers(taker, name, protocol, maker_addresses)
  offerbook = nil

  1.upto(MAKER_READY_ATTEMPTS) do |attempt|
    maker_addresses.each do |address|
      puts "  polling local maker #{address}"
      taker.poll_maker(address)
    rescue StandardError => e
      puts "  poll failed for #{address}: #{e.message}"
    end

    offerbook = taker.fetch_offers
    suitable = suitable_makers(offerbook, protocol)
    puts "#{name}: offerbook attempt #{attempt}/#{MAKER_READY_ATTEMPTS}: " \
         "#{offerbook.makers.length} total, #{suitable.length} suitable #{protocol} makers"
    offerbook.makers.each do |maker|
      maker_protocol = maker.protocol&.protocol_type || 'None'
      amount_range = maker.offer ? "#{maker.offer.min_size}..#{maker.offer.max_size} sats" : 'no offer'
      puts "  #{maker.address.address}: state=#{maker.state.state_type}, " \
           "protocol=#{maker_protocol}, amount=#{amount_range}"
    end
    STDOUT.flush

    return if suitable.length >= MAKER_COUNT
    sleep(MAKER_READY_RETRY_SECONDS) if attempt < MAKER_READY_ATTEMPTS
  end

  count = offerbook.nil? ? 0 : suitable_makers(offerbook, protocol).length
  raise "#{name}: expected #{MAKER_COUNT} suitable #{protocol} makers for " \
        "#{SWAP_AMOUNT} sats, found #{count}"
end

def run_swap(name, backend, protocol, addr_type)
  puts "\n=== #{name} (#{backend} / #{protocol} / #{addr_type}) ==="
  data_dir = File.expand_path("~/.openswap/taker/#{name}")
  cleanup_wallet(name, data_dir)

  rpc_config =
    if backend == 'rpc'
      Openswap::RpcConfig.new(
        url: 'localhost:18442',
        username: 'user',
        password: 'password',
        wallet_name: name
      )
    end

  backend_config =
    if backend == 'electrum'
      Openswap::BackendConfig.new(
        kind: 'electrum', url: 'tcp://localhost:50001',
        username: nil, password: nil, wallet_name: nil, zmq_addr: nil,
        socks5: nil, timeout: nil, poll_interval_secs: nil, max_retries: nil
      )
    end

  maker_addresses = local_maker_addresses

  taker = Openswap::Taker.init(
    data_dir,                  # taker data directory
    name,                      # wallet file name
    rpc_config,                # Bitcoin Core RPC settings (nil for electrum)
    9051,                      # Tor control port
    'openswap',                # Tor control password
    'tcp://127.0.0.1:28332',   # Bitcoin Core ZMQ endpoint
    WALLET_PASSWORD,           # wallet encryption password
    [],                        # poll only this CI job's local makers
    backend_config,            # backend selection (nil for rpc)
    nil                        # use taker config's blocklist setting
  )

  wait_for_suitable_makers(taker, name, protocol, maker_addresses)

  # Fund with 0.25 BTC across 4 fresh external addresses (1.0 BTC total).
  4.times do
    addr = taker.get_next_external_address(
      Openswap::AddressType.new(addr_type: addr_type)
    ).addr
    fund(addr)
  end

  target = SWAP_AMOUNT * 2
  funded = wait_for_spendable(taker, target)
  raise "#{name}: spendable #{funded.spendable} < target #{target}" unless funded.spendable >= target

  swap_params = Openswap::SwapParams.new(
    protocol: protocol,
    send_amount: SWAP_AMOUNT,
    maker_count: 2,
    tx_count: 1,
    required_confirms: 1,
    manually_selected_outpoints: nil,
    preferred_makers: maker_addresses,
    payment_address: nil
  )
  swap_id = taker.prepare_openswap(swap_params)
  report = taker.start_openswap(swap_id)

  raise "#{name}: openswap should return a swap report" if report.nil?
  raise "#{name}: should route through 2 makers, got #{report.makers_count}" unless report.makers_count == 2
  raise "#{name}: swap status was #{report.status}" unless report.status.upcase.include?('SUCCESS')

  puts "✓ #{name} passed (swap_id #{report.swap_id})"
end

def main
  requested = ARGV.fetch(0) do
    raise "swap case is required; expected one of: #{SWAPS.map(&:first).join(', ')}"
  end
  swap = SWAPS.find { |name,| name == requested }
  raise "unknown swap case #{requested.inspect}; expected one of: #{SWAPS.map(&:first).join(', ')}" if swap.nil?

  run_swap(*swap)
  puts "\n✓ #{requested} completed a 2-maker swap"
  STDOUT.flush
  exit!(0)
rescue StandardError => e
  puts "\n✗ Error: #{e.class.name}: #{e.message}"
  puts e.backtrace.join("\n")
  STDOUT.flush
  STDERR.flush
  exit!(1)
end

main if __FILE__ == $PROGRAM_NAME
