"""FFI taker integration test: one taker × 2 makers per process.

CI invokes this script once for each backend × protocol scenario. Each fresh
Python process prevents a completed native Taker from retaining Tor resources
needed by a later scenario.
"""

import os
import subprocess
import sys
import time

bindings_path = os.path.abspath(
    os.path.join(os.path.dirname(__file__), '..', 'src', 'openswap', 'native', 'linux-x86_64')
)
sys.path.insert(0, bindings_path)

from openswap import Taker, SwapParams, RpcConfig, AddressType, BackendConfig

# Amount swapped by each taker, in sats. The taker is funded with 2×.
SWAP_AMOUNT = 500_000
MAKER_COUNT = 2
MAKER_READY_ATTEMPTS = 3
MAKER_READY_RETRY_SECS = 10
MAKER_CONTAINERS = ("openswap-makerd1", "openswap-makerd2")
WALLET_PASSWORD = "ffi-live-test-wallet-password"
# Retain the native handle until os._exit bypasses its blocking upstream
# destructor at the end of this dedicated live-test process.
LIVE_TEST_TAKERS = []


def fund(address):
    """Fund `address` with 0.25 BTC from the Docker bitcoind `test` wallet."""
    subprocess.run(
        [
            'docker', 'exec', 'openswap-bitcoind',
            'bitcoin-cli', '-regtest', '-rpcport=18442',
            '-rpcwallet=test', '-rpcuser=user', '-rpcpassword=password',
            'sendtoaddress', address, '0.25',
        ],
        capture_output=True, text=True, check=True,
    )


def local_maker_addresses():
    """Read the two onion addresses belonging to this job's Docker stack."""
    marker = "Generated new Tor Hidden Service Hostname:"
    addresses = []
    for container in MAKER_CONTAINERS:
        result = subprocess.run(
            ["docker", "logs", container],
            capture_output=True, text=True, check=True,
        )
        logs = result.stdout + "\n" + result.stderr
        matches = [
            line.split(marker, 1)[1].strip().split()[0]
            for line in logs.splitlines()
            if marker in line
        ]
        if not matches:
            raise AssertionError(f"{container}: maker onion address not found in logs")
        addresses.append(matches[-1])
    return addresses


def wait_for_spendable(taker, target):
    """Sync until spendable reaches `target`, tolerating Electrum indexing lag."""
    for _ in range(30):
        taker.sync_and_save()
        balances = taker.get_balances()
        if balances.spendable >= target:
            return balances
        time.sleep(3)
    return taker.get_balances()


def suitable_makers(offerbook, protocol):
    """Return healthy makers that support this swap and amount."""
    return [
        maker for maker in offerbook.makers
        if maker.state.state_type == "Good"
        and maker.protocol is not None
        and maker.protocol.protocol_type in (protocol, "Unified")
        and maker.offer is not None
        and maker.offer.min_size <= SWAP_AMOUNT <= maker.offer.max_size
    ]


def print_offerbook(name, protocol, offerbook, attempt):
    """Print enough state to diagnose a failed maker-readiness check in CI."""
    suitable = suitable_makers(offerbook, protocol)
    print(
        f"{name}: offerbook attempt {attempt}/{MAKER_READY_ATTEMPTS}: "
        f"{len(offerbook.makers)} total, {len(suitable)} suitable {protocol} makers",
        flush=True,
    )
    for maker in offerbook.makers:
        maker_protocol = maker.protocol.protocol_type if maker.protocol else "None"
        amount_range = (
            f"{maker.offer.min_size}..{maker.offer.max_size} sats"
            if maker.offer else "no offer"
        )
        print(
            f"  {maker.address.address}: state={maker.state.state_type}, "
            f"protocol={maker_protocol}, amount={amount_range}",
            flush=True,
        )
    return suitable


def wait_for_suitable_makers(taker, name, protocol, maker_addresses):
    """Wait for two usable offers and explicitly re-poll transient failures."""
    last_offerbook = None

    for attempt in range(1, MAKER_READY_ATTEMPTS + 1):
        for address in maker_addresses:
            try:
                print(f"  polling local maker {address}", flush=True)
                taker.poll_maker(address)
            except Exception as error:
                print(f"  poll failed for {address}: {error}", flush=True)

        last_offerbook = taker.fetch_offers()
        suitable = print_offerbook(name, protocol, last_offerbook, attempt)
        if len(suitable) >= MAKER_COUNT:
            return

        if attempt < MAKER_READY_ATTEMPTS:
            time.sleep(MAKER_READY_RETRY_SECS)

    raise AssertionError(
        f"{name}: expected {MAKER_COUNT} suitable {protocol} makers for "
        f"{SWAP_AMOUNT} sats, found {len(suitable_makers(last_offerbook, protocol))}"
    )


def run_swap(name, data_dir, backend, protocol, addr_type):
    """Run one taker end-to-end: init → fund → sync → 2-maker openswap → assert."""
    print(f"\n=== {name} ({protocol}) ===")

    rpc_config = (
        RpcConfig(url="localhost:18442", username="user", password="password", wallet_name=f"python_{name}")
        if backend == "rpc" else None
    )
    backend_config = (
        BackendConfig(
            kind="electrum",
            url="tcp://localhost:50001",
            username=None,
            password=None,
            wallet_name=None,
            zmq_addr=None,
            socks5=None,
            timeout=None,
            poll_interval_secs=None,
            max_retries=None,
        )
        if backend == "electrum" else None
    )

    maker_addresses = local_maker_addresses()
    taker = Taker.init(
        data_dir=data_dir,
        wallet_file_name=name,
        rpc_config=rpc_config,
        control_port=9051,
        tor_auth_password="openswap",
        zmq_addr="tcp://localhost:28332",
        password=WALLET_PASSWORD,
        # Every CI job has its own regtest chain. Public discovery can return
        # makers from unrelated jobs, so poll this stack's onion addresses.
        nostr_relays=[],
        backend_config=backend_config,
        check_blocklist=None,
    )
    LIVE_TEST_TAKERS.append(taker)

    wait_for_suitable_makers(taker, name, protocol, maker_addresses)

    # Fund with 2× the swap amount across 4 fresh external addresses.
    for _ in range(4):
        addr = taker.get_next_external_address(AddressType(addr_type=addr_type)).addr
        fund(addr)

    target = SWAP_AMOUNT * 2
    funded = wait_for_spendable(taker, target)
    assert funded.spendable >= target, (
        f"{name}: spendable {funded.spendable} < target {target}"
    )

    swap_id = taker.prepare_openswap(
        swap_params=SwapParams(
            protocol=protocol,
            send_amount=SWAP_AMOUNT,
            maker_count=MAKER_COUNT,
            tx_count=1,
            required_confirms=1,
            manually_selected_outpoints=None,
            preferred_makers=maker_addresses,
            payment_address=None,
        )
    )
    report = taker.start_openswap(swap_id=swap_id)
    assert report is not None, f"{name}: openswap should return a swap report"
    assert report.makers_count == 2, f"{name}: should route through 2 makers, got {report.makers_count}"
    assert "SUCCESS" in report.status.upper(), f"{name}: swap status was {report.status}"

    print(f"✓ {name} passed (swap_id {report.swap_id})")


SWAPS = [
    # (name, backend, protocol, addr_type)
    ("legacy_rpc", "rpc", "Legacy", "P2WPKH"),
    ("taproot_rpc", "rpc", "Taproot", "P2TR"),
    ("legacy_electrum", "electrum", "Legacy", "P2WPKH"),
    ("taproot_electrum", "electrum", "Taproot", "P2TR"),
]


def main():
    if len(sys.argv) != 2:
        choices = ", ".join(swap[0] for swap in SWAPS)
        print(f"usage: {sys.argv[0]} <case>\nvalid cases: {choices}", file=sys.stderr)
        sys.exit(2)

    requested = sys.argv[1]
    selected = next((swap for swap in SWAPS if swap[0] == requested), None)
    if selected is None:
        choices = ", ".join(swap[0] for swap in SWAPS)
        print(
            f"unknown swap case {requested!r}; expected one of: {choices}",
            file=sys.stderr,
        )
        sys.exit(2)

    base_dir = os.path.expanduser("~/.openswap/taker")
    exit_code = 0
    try:
        name, backend, protocol, addr_type = selected
        data_dir = os.path.join(base_dir, name)
        run_swap(name, data_dir, backend, protocol, addr_type)
    except Exception as e:
        print(f"\n✗ Error: {type(e).__name__}: {e}")
        import traceback
        traceback.print_exc()
        exit_code = 1
    finally:
        # This is a dedicated live-test process. Bypass Python finalizers so
        # native taker destruction cannot block on the upstream watcher thread;
        # the OS reclaims all test-only handles at process exit.
        sys.stdout.flush()
        sys.stderr.flush()
        os._exit(exit_code)


if __name__ == "__main__":
    main()
