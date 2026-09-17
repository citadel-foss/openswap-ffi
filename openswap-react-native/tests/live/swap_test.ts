import { execFileSync, spawnSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";

import {
  setupLogging,
  Taker,
  type BackendConfig,
  type OfferBook,
  type RpcConfig,
  type TakerLike,
} from "../../src/generated";

const SWAP_AMOUNT = 500_000n;
const MAKER_COUNT = 2;
const MAKER_READY_ATTEMPTS = 3;
const MAKER_READY_RETRY_MS = 10_000;
const MAKER_CONTAINERS = ["openswap-makerd1", "openswap-makerd2"];
const WALLET_PASSWORD = "ffi-live-test-wallet-password";
const RPC_AUTH_ARGS = [
  "-regtest",
  "-rpcport=18442",
  "-rpcuser=user",
  "-rpcpassword=password",
];

type Backend = "rpc" | "electrum";

type SwapCase = {
  name: string;
  backend: Backend;
  protocol: "Legacy" | "Taproot";
  addressType: "P2WPKH" | "P2TR";
};

const CASES: SwapCase[] = [
  {
    name: "legacy_rpc",
    backend: "rpc",
    protocol: "Legacy",
    addressType: "P2WPKH",
  },
  {
    name: "taproot_rpc",
    backend: "rpc",
    protocol: "Taproot",
    addressType: "P2TR",
  },
  {
    name: "legacy_electrum",
    backend: "electrum",
    protocol: "Legacy",
    addressType: "P2WPKH",
  },
  {
    name: "taproot_electrum",
    backend: "electrum",
    protocol: "Taproot",
    addressType: "P2TR",
  },
];

// Keep the native object alive until process.exit bypasses its blocking
// upstream destructor at the end of this dedicated one-swap process.
const LIVE_TEST_TAKERS: TakerLike[] = [];

function bitcoinCli(args: string[]): string {
  return execFileSync(
    "docker",
    ["exec", "openswap-bitcoind", "bitcoin-cli", ...args],
    {
      encoding: "utf8",
      stdio: ["ignore", "pipe", "pipe"],
    },
  ).trim();
}

function cleanupWallet(walletName: string, dataDir: string) {
  fs.rmSync(dataDir, { recursive: true, force: true });
  try {
    bitcoinCli([...RPC_AUTH_ARGS, "unloadwallet", walletName]);
  } catch {
    // Ignore missing wallet errors.
  }
}

function localMakerAddresses(): string[] {
  const marker = "Generated new Tor Hidden Service Hostname:";

  return MAKER_CONTAINERS.map((container) => {
    const result = spawnSync("docker", ["logs", container], {
      encoding: "utf8",
    });
    if (result.status !== 0) {
      throw new Error(
        `${container}: failed to read maker logs: ${result.stderr}`,
      );
    }

    const matches = `${result.stdout}\n${result.stderr}`
      .split("\n")
      .filter((line) => line.includes(marker))
      .map((line) => line.split(marker, 2)[1].trim().split(/\s+/, 1)[0]);

    const address = matches.at(-1);
    if (!address)
      throw new Error(`${container}: maker onion address not found in logs`);
    return address;
  });
}

function suitableMakers(offerbook: OfferBook, protocol: string) {
  return offerbook.makers.filter(
    (maker) =>
      maker.state.stateType === "Good" &&
      maker.protocol !== undefined &&
      [protocol, "Unified"].includes(maker.protocol.protocolType) &&
      maker.offer !== undefined &&
      maker.offer.minSize <= SWAP_AMOUNT &&
      SWAP_AMOUNT <= maker.offer.maxSize,
  );
}

function sleep(ms: number) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

async function waitForSuitableMakers(
  taker: TakerLike,
  name: string,
  protocol: string,
  makerAddresses: string[],
) {
  let offerbook: OfferBook | undefined;

  for (let attempt = 1; attempt <= MAKER_READY_ATTEMPTS; attempt += 1) {
    for (const address of makerAddresses) {
      try {
        console.log(`  polling local maker ${address}`);
        taker.pollMaker(address);
      } catch (error) {
        console.log(`  poll failed for ${address}: ${String(error)}`);
      }
    }

    offerbook = taker.fetchOffers();
    const suitable = suitableMakers(offerbook, protocol);
    console.log(
      `${name}: offerbook attempt ${attempt}/${MAKER_READY_ATTEMPTS}: ` +
        `${offerbook.makers.length} total, ${suitable.length} suitable ${protocol} makers`,
    );
    for (const maker of offerbook.makers) {
      const makerProtocol = maker.protocol?.protocolType ?? "None";
      const amountRange = maker.offer
        ? `${maker.offer.minSize}..${maker.offer.maxSize} sats`
        : "no offer";
      console.log(
        `  ${maker.address.address}: state=${maker.state.stateType}, ` +
          `protocol=${makerProtocol}, amount=${amountRange}`,
      );
    }

    if (suitable.length >= MAKER_COUNT) return;
    if (attempt < MAKER_READY_ATTEMPTS) await sleep(MAKER_READY_RETRY_MS);
  }

  const count = offerbook ? suitableMakers(offerbook, protocol).length : 0;
  throw new Error(
    `${name}: expected ${MAKER_COUNT} suitable ${protocol} makers for ` +
      `${SWAP_AMOUNT} sats, found ${count}`,
  );
}

async function waitForSpendable(taker: TakerLike, target: bigint) {
  let balances = taker.getBalances();
  for (let attempt = 0; attempt < 30; attempt += 1) {
    taker.syncAndSave();
    balances = taker.getBalances();
    if (balances.spendable >= target) return balances;
    await sleep(3_000);
  }
  return balances;
}

async function runSwap(swap: SwapCase) {
  const { name, backend, protocol, addressType } = swap;
  console.log(`\n=== ${name} (${backend} / ${protocol}) ===`);

  const dataDir = path.join(os.tmpdir(), "openswap-react-native-live", name);
  cleanupWallet(name, dataDir);

  const rpcConfig: RpcConfig | undefined =
    backend === "rpc"
      ? {
          url: "localhost:18442",
          username: "user",
          password: "password",
          walletName: name,
        }
      : undefined;
  const backendConfig: BackendConfig | undefined =
    backend === "electrum"
      ? { kind: "electrum", url: "tcp://localhost:50001" }
      : undefined;
  const makerAddresses = localMakerAddresses();

  setupLogging(undefined, "Info", true);
  const taker = Taker.init(
    dataDir,
    name,
    rpcConfig,
    9051,
    "openswap",
    "tcp://127.0.0.1:28332",
    WALLET_PASSWORD,
    [],
    backendConfig,
    undefined,
  );
  LIVE_TEST_TAKERS.push(taker);

  await waitForSuitableMakers(taker, name, protocol, makerAddresses);

  for (let index = 0; index < 4; index += 1) {
    const address = taker.getNextExternalAddress({ addrType: addressType });
    bitcoinCli([
      ...RPC_AUTH_ARGS,
      "-rpcwallet=test",
      "sendtoaddress",
      address.addr,
      "0.25",
    ]);
  }

  const target = SWAP_AMOUNT * 2n;
  const funded = await waitForSpendable(taker, target);
  if (funded.spendable < target) {
    throw new Error(
      `${name}: spendable ${funded.spendable} < target ${target}`,
    );
  }

  const swapId = taker.prepareOpenswap({
    protocol,
    sendAmount: SWAP_AMOUNT,
    makerCount: MAKER_COUNT,
    txCount: 1,
    requiredConfirms: 1,
    preferredMakers: makerAddresses,
  });
  const report = taker.startOpenswap(swapId);

  if (report.makersCount !== MAKER_COUNT) {
    throw new Error(
      `${name}: expected ${MAKER_COUNT} makers, got ${report.makersCount}`,
    );
  }
  if (!report.status.toUpperCase().includes("SUCCESS")) {
    throw new Error(`${name}: swap status was ${report.status}`);
  }

  console.log(`✓ ${name} passed (swap_id ${report.swapId})`);
}

async function main() {
  const requested = process.env.OPENSWAP_SWAP_CASE;
  const swap = CASES.find(({ name }) => name === requested);
  if (!swap) {
    throw new Error(
      `OPENSWAP_SWAP_CASE must be one of: ${CASES.map(({ name }) => name).join(", ")}; ` +
        `received ${JSON.stringify(requested)}`,
    );
  }

  await runSwap(swap);
}

main().then(
  () => process.exit(0),
  (error: unknown) => {
    console.error(error);
    process.exit(1);
  },
);
