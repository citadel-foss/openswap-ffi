"""Network-free contract tests for the generated Python binding."""

import os
import sys
import tempfile
import unittest
import uuid


bindings_path = os.path.abspath(
    os.path.join(
        os.path.dirname(__file__),
        "..",
        "src",
        "openswap",
        "native",
        "linux-x86_64",
    )
)
sys.path.insert(0, bindings_path)

import openswap


class ApiContractTest(unittest.TestCase):
    def assert_record(self, record, **expected_fields):
        """Assert the complete public shape, including absent optional fields."""
        self.assertEqual(vars(record), expected_fields)

    def test_exported_callables_are_complete(self):
        globals_ = {
            "create_default_rpc_config",
            "fetch_mempool_fees",
            "is_wallet_encrypted",
            "openswap_ffi_version",
            "restore_wallet_gui_app",
            "setup_logging",
        }
        methods = {
            "backup",
            "display_offer",
            "fetch_all_makers",
            "fetch_offers",
            "get_balances",
            "get_next_external_address",
            "get_next_internal_addresses",
            "get_transactions",
            "get_wallet_name",
            "init",
            "list_all_utxo_spend_info",
            "lock_unspendable_utxos",
            "poll_maker",
            "prepare_openswap",
            "recover_active_swap",
            "remove_maker",
            "send_to_address",
            "setup_logging",
            "start_openswap",
            "sync_and_save",
            "sync_offerbook_and_wait",
            "verify_deniability",
        }

        self.assertEqual(
            {name for name in globals_ if not callable(getattr(openswap, name, None))},
            set(),
        )
        self.assertEqual(
            {name for name in methods if not callable(getattr(openswap.Taker, name, None))},
            set(),
        )

    def test_native_globals_and_typed_validation_errors(self):
        self.assertEqual(openswap.openswap_ffi_version(), "0.1.0")
        self.assert_record(
            openswap.create_default_rpc_config(),
            url="http://127.0.0.1:38332",
            username="user",
            password="password",
            wallet_name="openswap_wallet",
        )

        missing_wallet = os.path.join(
            tempfile.gettempdir(), f"openswap-missing-wallet-{uuid.uuid4().hex}"
        )
        # The native contract intentionally treats a missing wallet as not encrypted.
        self.assertFalse(openswap.is_wallet_encrypted(missing_wallet))

        invalid_backend = openswap.BackendConfig(
            kind="invalid",
            url=None,
            username=None,
            password=None,
            wallet_name=None,
            zmq_addr=None,
            socks5=None,
            timeout=None,
            poll_interval_secs=None,
            max_retries=None,
        )
        with self.assertRaises(openswap.TakerError.General) as error:
            openswap.Taker.init(
                None,
                None,
                None,
                None,
                None,
                "tcp://127.0.0.1:28332",
                None,
                [],
                invalid_backend,
                None,
            )
        self.assertEqual(
            error.exception.msg,
            "Invalid backend kind: invalid (expected rpc or electrum)",
        )

    def test_scalar_and_configuration_records_preserve_every_field(self):
        self.assert_record(openswap.Address(addr="bc1qexample"), addr="bc1qexample")
        self.assert_record(openswap.AddressType(addr_type="P2TR"), addr_type="P2TR")
        self.assert_record(openswap.Amount(sats=-1), sats=-1)
        self.assert_record(openswap.SignedAmountSats(sats=-(2**63)), sats=-(2**63))
        self.assert_record(openswap.Txid(value="ab" * 32), value="ab" * 32)
        self.assert_record(openswap.ScriptBuf(hex="0051ff"), hex="0051ff")
        self.assert_record(
            openswap.PublicKey(compressed=True, inner=bytes(range(33))),
            compressed=True,
            inner=bytes(range(33)),
        )
        self.assert_record(
            openswap.RpcConfig(
                url="http://node:18443",
                username="alice",
                password="secret",
                wallet_name="wallet-a",
            ),
            url="http://node:18443",
            username="alice",
            password="secret",
            wallet_name="wallet-a",
        )
        self.assert_record(
            openswap.BackendConfig(
                kind="electrum",
                url="ssl://electrum.example:50002",
                username=None,
                password=None,
                wallet_name=None,
                zmq_addr=None,
                socks5="127.0.0.1:9050",
                timeout=120,
                poll_interval_secs=15,
                max_retries=8,
            ),
            kind="electrum",
            url="ssl://electrum.example:50002",
            username=None,
            password=None,
            wallet_name=None,
            zmq_addr=None,
            socks5="127.0.0.1:9050",
            timeout=120,
            poll_interval_secs=15,
            max_retries=8,
        )
        self.assert_record(
            openswap.Balances(regular=1, swap=2, contract=3, fidelity=4, spendable=3),
            regular=1,
            swap=2,
            contract=3,
            fidelity=4,
            spendable=3,
        )
        self.assert_record(
            openswap.FeeRates(fastest=12.5, standard=6.25, economy=1.0),
            fastest=12.5,
            standard=6.25,
            economy=1.0,
        )
        self.assert_record(openswap.LockTime(lock_type="Blocks", value=144), lock_type="Blocks", value=144)
        self.assert_record(openswap.MakerAddress(address="maker.onion:6102"), address="maker.onion:6102")
        self.assert_record(openswap.MakerState(state_type="Unresponsive", retries=7), state_type="Unresponsive", retries=7)
        self.assert_record(openswap.MakerProtocol(protocol_type="Unified"), protocol_type="Unified")
        self.assert_record(openswap.UtxoWithAddress(amount=50_000, address="bc1qchange"), amount=50_000, address="bc1qchange")

    def test_transaction_and_utxo_records_preserve_nested_optional_data(self):
        txid = openswap.Txid(value="01" * 32)
        address = openswap.Address(addr="bc1qtransaction")
        info = openswap.WalletTxInfo(
            confirmations=-1,
            blockhash="02" * 32,
            blockindex=3,
            blocktime=1_700_000_000,
            blockheight=250,
            txid=txid,
            time=1_700_000_001,
            timereceived=1_700_000_002,
            bip125_replaceable="Yes",
            wallet_conflicts=[openswap.Txid(value="03" * 32)],
        )
        detail = openswap.GetTransactionResultDetail(
            address=address,
            category="Send",
            amount=openswap.SignedAmountSats(sats=-50_000),
            label="payment",
            vout=2,
            fee=openswap.SignedAmountSats(sats=-250),
            abandoned=False,
        )
        transaction = openswap.ListTransactionResult(
            info=info,
            detail=detail,
            trusted=True,
            comment="memo",
        )
        self.assert_record(transaction, info=info, detail=detail, trusted=True, comment="memo")

        unspent = openswap.ListUnspentResultEntry(
            txid=txid,
            vout=4,
            address="bc1qutxo",
            label="seed",
            script_pub_key=openswap.ScriptBuf(hex="0014"),
            amount=openswap.Amount(sats=75_000),
            confirmations=6,
            redeem_script=openswap.ScriptBuf(hex="51"),
            witness_script=None,
            spendable=True,
            solvable=True,
            desc="wpkh(...)",
            safe=False,
        )
        spend_info = openswap.UtxoSpendInfo(
            spend_type="FidelityBondCoin",
            path="m/84'/1'/0'/0/1",
            multisig_redeemscript=None,
            input_value=openswap.Amount(sats=75_000),
            index=9,
        )
        total = openswap.TotalUtxoInfo(
            list_unspent_result_entry=unspent,
            utxo_spend_info=spend_info,
        )
        self.assert_record(
            total,
            list_unspent_result_entry=unspent,
            utxo_spend_info=spend_info,
        )

    def test_offer_and_swap_records_preserve_the_complete_nested_graph(self):
        outpoint = openswap.OutPoint(txid=openswap.Txid(value="04" * 32), vout=5)
        public_key = openswap.PublicKey(compressed=True, inner=b"\x02" * 33)
        bond = openswap.FidelityBond(
            outpoint=outpoint,
            amount=openswap.Amount(sats=100_000),
            lock_time=openswap.LockTime(lock_type="Seconds", value=500_000_000),
            pubkey=public_key,
            conf_height=100,
            cert_expiry=None,
            is_spent=False,
        )
        proof = openswap.FidelityProof(
            bond=bond,
            cert_hash=b"\x05" * 32,
            cert_sig=b"\x06" * 64,
        )
        offer = openswap.Offer(
            base_fee=-5,
            amount_relative_fee_pct=0.125,
            time_relative_fee_pct=0.25,
            required_confirms=2,
            minimum_locktime=48,
            max_size=2_000_000,
            min_size=50_000,
            tweakable_point=public_key,
            fidelity=proof,
        )
        candidate = openswap.MakerOfferCandidate(
            address=openswap.MakerAddress(address="maker.onion:6102"),
            offer=offer,
            state=openswap.MakerState(state_type="Good", retries=None),
            protocol=openswap.MakerProtocol(protocol_type="Taproot"),
        )
        self.assert_record(openswap.OfferBook(makers=[candidate]), makers=[candidate])

        params = openswap.SwapParams(
            protocol="Taproot",
            send_amount=500_000,
            maker_count=2,
            tx_count=3,
            required_confirms=4,
            manually_selected_outpoints=[outpoint],
            preferred_makers=["maker.onion:6102"],
            payment_address=None,
        )
        self.assert_record(
            params,
            protocol="Taproot",
            send_amount=500_000,
            maker_count=2,
            tx_count=3,
            required_confirms=4,
            manually_selected_outpoints=[outpoint],
            preferred_makers=["maker.onion:6102"],
            payment_address=None,
        )

        fee = openswap.MakerFeeInfo(
            maker_index=1,
            maker_address="maker.onion:6102",
            base_fee=100.0,
            amount_relative_fee=200.0,
            time_relative_fee=300.0,
            total_fee=600.0,
        )
        change = openswap.UtxoWithAddress(amount=25_000, address="bc1qchange")
        swap_output = openswap.UtxoWithAddress(amount=475_000, address="bc1qswap")
        outgoing_utxo = openswap.ReportUtxo(address="bc1qoutgoing", value=510_000)
        incoming_utxo = openswap.ReportUtxo(address="bc1qincoming", value=475_000)
        report_fields = dict(
            swap_id="swap-1",
            role="Taker",
            status="SUCCESS",
            swap_duration_seconds=12.5,
            start_timestamp=1_700_000_000,
            end_timestamp=1_700_000_013,
            network="regtest",
            error_message=None,
            incoming_amount=500_000,
            outgoing_amount=510_000,
            fee_paid=-10_000,
            outgoing_utxos=[outgoing_utxo],
            incoming_utxos=[incoming_utxo],
            funding_txids=[["08" * 32], ["09" * 32]],
            makers_count=2,
            maker_addresses=["maker-1", "maker-2"],
            total_maker_fees=8_000,
            mining_fee=2_000,
            fee_percentage=2.0,
            maker_fee_info=[fee],
            input_utxos=[510_000],
            output_change_amounts=[25_000],
            output_swap_amounts=[475_000],
            output_change_utxos=[change],
            output_swap_utxos=[swap_output],
        )
        self.assert_record(openswap.SwapReport(**report_fields), **report_fields)

    def test_enum_and_error_variants_remain_distinct_and_keep_messages(self):
        self.assertEqual(
            list(openswap.TakerBehavior),
            [
                openswap.TakerBehavior.NORMAL,
                openswap.TakerBehavior.DROP_CONNECTION_AFTER_FULL_SETUP,
                openswap.TakerBehavior.BROADCAST_CONTRACT_AFTER_FULL_SETUP,
            ],
        )
        for error_type in (
            openswap.TakerError.Wallet,
            openswap.TakerError.Protocol,
            openswap.TakerError.Network,
            openswap.TakerError.General,
            openswap.TakerError.Io,
        ):
            self.assertEqual(error_type("reason").msg, "reason")


if __name__ == "__main__":
    unittest.main()
