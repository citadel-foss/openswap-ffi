# frozen_string_literal: true

require 'minitest/autorun'
require 'securerandom'
require 'tmpdir'

$LOAD_PATH.unshift(File.expand_path('..', __dir__))
require 'openswap'

# Network-free contracts for values and errors crossing the Ruby FFI boundary.
class ApiContractTest < Minitest::Test
  def test_native_defaults_and_validation_errors
    assert_equal '0.1.0', Openswap.openswap_ffi_version
    assert_equal Openswap::RpcConfig.new(
      url: 'http://127.0.0.1:38332',
      username: 'user',
      password: 'password',
      wallet_name: 'openswap_wallet'
    ), Openswap.create_default_rpc_config
    missing_wallet = File.join(Dir.tmpdir, "openswap-#{SecureRandom.hex(12)}")
    refute Openswap.is_wallet_encrypted(missing_wallet)

    backend = Openswap::BackendConfig.new(
      kind: 'invalid', url: nil, username: nil, password: nil,
      wallet_name: nil, zmq_addr: nil, socks5: nil, timeout: nil,
      poll_interval_secs: nil, max_retries: nil
    )
    error = assert_raises(Openswap::TakerError::General) do
      Openswap::Taker.init(
        nil, nil, nil, nil, nil, 'tcp://127.0.0.1:28332', nil, [], backend, nil
      )
    end
    assert_equal 'Invalid backend kind: invalid (expected rpc or electrum)', error.msg
  end

  def test_scalar_configuration_and_wallet_records_preserve_every_field
    assert_equal 'bc1qexample', Openswap::Address.new(addr: 'bc1qexample').addr
    assert_equal 'P2TR', Openswap::AddressType.new(addr_type: 'P2TR').addr_type
    assert_equal(-(2**63), Openswap::SignedAmountSats.new(sats: -(2**63)).sats)
    assert_equal '0051ff', Openswap::ScriptBuf.new(hex: '0051ff').hex

    txid = Openswap::Txid.new(value: '01' * 32)
    key_bytes = (0...33).to_a.pack('C*')
    public_key = Openswap::PublicKey.new(compressed: true, inner: key_bytes)
    assert public_key.compressed
    assert_equal key_bytes, public_key.inner

    backend = Openswap::BackendConfig.new(
      kind: 'electrum', url: 'ssl://electrum.example:50002',
      username: nil, password: nil, wallet_name: nil, zmq_addr: nil,
      socks5: '127.0.0.1:9050', timeout: 120,
      poll_interval_secs: 15, max_retries: 8
    )
    assert_equal 'electrum', backend.kind
    assert_equal 'ssl://electrum.example:50002', backend.url
    assert_equal '127.0.0.1:9050', backend.socks5
    assert_equal 120, backend.timeout
    assert_equal 15, backend.poll_interval_secs
    assert_equal 8, backend.max_retries

    info = Openswap::WalletTxInfo.new(
      confirmations: -1, blockhash: '02' * 32, blockindex: 3,
      blocktime: 1_700_000_000, blockheight: 250, txid: txid,
      time: 1_700_000_001, timereceived: 1_700_000_002,
      bip125_replaceable: 'Yes',
      wallet_conflicts: [Openswap::Txid.new(value: '03' * 32)]
    )
    detail = Openswap::GetTransactionResultDetail.new(
      address: Openswap::Address.new(addr: 'bc1qtransaction'),
      category: 'Send', amount: Openswap::SignedAmountSats.new(sats: -50_000),
      label: 'payment', vout: 2,
      fee: Openswap::SignedAmountSats.new(sats: -250), abandoned: false
    )
    transaction = Openswap::ListTransactionResult.new(
      info: info, detail: detail, trusted: true, comment: 'memo'
    )
    assert_equal info, transaction.info
    assert_equal detail, transaction.detail
    assert transaction.trusted
    assert_equal 'memo', transaction.comment

    unspent = Openswap::ListUnspentResultEntry.new(
      txid: txid, vout: 4, address: 'bc1qutxo', label: 'seed',
      script_pub_key: Openswap::ScriptBuf.new(hex: '0014'),
      amount: Openswap::Amount.new(sats: 75_000), confirmations: 6,
      redeem_script: Openswap::ScriptBuf.new(hex: '51'), witness_script: nil,
      spendable: true, solvable: true, desc: 'wpkh(...)', safe: false
    )
    spend_info = Openswap::UtxoSpendInfo.new(
      spend_type: 'FidelityBondCoin', path: "m/84'/1'/0'/0/1",
      multisig_redeemscript: nil, input_value: Openswap::Amount.new(sats: 75_000),
      index: 9
    )
    total = Openswap::TotalUtxoInfo.new(
      list_unspent_result_entry: unspent, utxo_spend_info: spend_info
    )
    assert_equal unspent, total.list_unspent_result_entry
    assert_equal spend_info, total.utxo_spend_info

    balances = Openswap::Balances.new(regular: 1, swap: 2, contract: 3, fidelity: 4, spendable: 3)
    assert_equal [1, 2, 3, 4, 3],
                 [balances.regular, balances.swap, balances.contract, balances.fidelity, balances.spendable]
    fee_rates = Openswap::FeeRates.new(fastest: 12.5, standard: 6.25, economy: 1.0)
    assert_equal [12.5, 6.25, 1.0], [fee_rates.fastest, fee_rates.standard, fee_rates.economy]
    lock_time = Openswap::LockTime.new(lock_type: 'Blocks', value: 144)
    assert_equal ['Blocks', 144], [lock_time.lock_type, lock_time.value]
    maker_state = Openswap::MakerState.new(state_type: 'Unresponsive', retries: 7)
    assert_equal ['Unresponsive', 7], [maker_state.state_type, maker_state.retries]
    assert_equal 'Unified', Openswap::MakerProtocol.new(protocol_type: 'Unified').protocol_type
  end

  def test_offer_and_swap_records_preserve_the_complete_nested_graph
    outpoint = Openswap::OutPoint.new(txid: Openswap::Txid.new(value: '04' * 32), vout: 5)
    public_key = Openswap::PublicKey.new(compressed: true, inner: "\x02".b * 33)
    bond = Openswap::FidelityBond.new(
      outpoint: outpoint, amount: Openswap::Amount.new(sats: 100_000),
      lock_time: Openswap::LockTime.new(lock_type: 'Seconds', value: 500_000_000),
      pubkey: public_key, conf_height: 100, cert_expiry: nil, is_spent: false
    )
    proof = Openswap::FidelityProof.new(
      bond: bond, cert_hash: "\x05".b * 32, cert_sig: "\x06".b * 64
    )
    offer = Openswap::Offer.new(
      base_fee: -5, amount_relative_fee_pct: 0.125, time_relative_fee_pct: 0.25,
      required_confirms: 2, minimum_locktime: 48, max_size: 2_000_000,
      min_size: 50_000, tweakable_point: public_key, fidelity: proof
    )
    candidate = Openswap::MakerOfferCandidate.new(
      address: Openswap::MakerAddress.new(address: 'maker.onion:6102'),
      offer: offer, state: Openswap::MakerState.new(state_type: 'Good', retries: nil),
      protocol: Openswap::MakerProtocol.new(protocol_type: 'Taproot')
    )
    assert_equal candidate, Openswap::OfferBook.new(makers: [candidate]).makers.first
    assert_equal "\x06".b * 64, candidate.offer.fidelity.cert_sig

    params = Openswap::SwapParams.new(
      protocol: 'Taproot', send_amount: 500_000, maker_count: 2,
      tx_count: 3, required_confirms: 4,
      manually_selected_outpoints: [outpoint],
      preferred_makers: ['maker.onion:6102'], payment_address: nil
    )
    assert_equal 'Taproot', params.protocol
    assert_equal 500_000, params.send_amount
    assert_equal 2, params.maker_count
    assert_equal 3, params.tx_count
    assert_equal 4, params.required_confirms
    assert_equal [outpoint], params.manually_selected_outpoints
    assert_equal ['maker.onion:6102'], params.preferred_makers
    assert_nil params.payment_address

    fee = Openswap::MakerFeeInfo.new(
      maker_index: 1, maker_address: 'maker.onion:6102', base_fee: 100,
      amount_relative_fee: 200, time_relative_fee: 300, total_fee: 600
    )
    change = Openswap::UtxoWithAddress.new(amount: 25_000, address: 'bc1qchange')
    swap_output = Openswap::UtxoWithAddress.new(amount: 475_000, address: 'bc1qswap')
    outgoing_utxo = Openswap::ReportUtxo.new(address: 'bc1qoutgoing', value: 510_000)
    incoming_utxo = Openswap::ReportUtxo.new(address: 'bc1qincoming', value: 475_000)
    report = Openswap::SwapReport.new(
      swap_id: 'swap-1', role: 'Taker', status: 'SUCCESS', swap_duration_seconds: 12.5,
      start_timestamp: 1_700_000_000, end_timestamp: 1_700_000_013,
      network: 'regtest', error_message: nil, incoming_amount: 500_000,
      outgoing_amount: 510_000, fee_paid: -10_000,
      outgoing_utxos: [outgoing_utxo], incoming_utxos: [incoming_utxo],
      funding_txids: [['08' * 32], ['09' * 32]], makers_count: 2,
      maker_addresses: %w[maker-1 maker-2], total_maker_fees: 8_000,
      mining_fee: 2_000, fee_percentage: 2.0, maker_fee_info: [fee],
      input_utxos: [510_000], output_change_amounts: [25_000],
      output_swap_amounts: [475_000], output_change_utxos: [change],
      output_swap_utxos: [swap_output]
    )
    assert_equal 'swap-1', report.swap_id
    assert_equal(-10_000, report.fee_paid)
    assert_equal [outgoing_utxo], report.outgoing_utxos
    assert_equal [incoming_utxo], report.incoming_utxos
    assert_equal 2, report.funding_txids.length
    assert_equal [fee], report.maker_fee_info
    assert_equal [change], report.output_change_utxos
    assert_equal [swap_output], report.output_swap_utxos
  end

  def test_enum_error_and_taker_method_surfaces_are_complete
    expected_globals = %i[
      create_default_rpc_config fetch_mempool_fees is_wallet_encrypted
      openswap_ffi_version restore_wallet_gui_app setup_logging
    ]
    assert_empty expected_globals - Openswap.methods

    assert_equal [1, 2, 3], [
      Openswap::TakerBehavior::NORMAL,
      Openswap::TakerBehavior::DROP_CONNECTION_AFTER_FULL_SETUP,
      Openswap::TakerBehavior::BROADCAST_CONTRACT_AFTER_FULL_SETUP
    ]
    errors = [
      Openswap::TakerError::Wallet,
      Openswap::TakerError::Protocol,
      Openswap::TakerError::Network,
      Openswap::TakerError::General,
      Openswap::TakerError::Io
    ]
    errors.each { |error| assert_equal 'reason', error.new('reason').msg }

    expected_methods = %i[
      backup display_offer fetch_all_makers fetch_offers get_balances
      get_next_external_address get_next_internal_addresses get_transactions
      get_wallet_name list_all_utxo_spend_info lock_unspendable_utxos poll_maker
      prepare_openswap recover_active_swap remove_maker send_to_address setup_logging
      start_openswap sync_and_save sync_offerbook_and_wait verify_deniability
    ]
    assert_respond_to Openswap::Taker, :init
    assert_empty expected_methods - Openswap::Taker.instance_methods(false)
  end
end
