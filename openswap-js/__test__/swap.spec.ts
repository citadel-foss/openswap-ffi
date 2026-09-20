import { execFileSync, spawnSync } from 'node:child_process'
import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'

import test from 'ava'

import { AddressType, type BackendConfig, type OfferBook, type RpcConfig, Taker } from '../index'

// FFI taker integration test: 4 takers x 2 makers.
//
// Mirrors the Rust/Python/Swift swap tests: four takers run sequentially
// against the Docker regtest stack (1 RPC maker + 1 Electrum maker), covering
// the full backend x protocol matrix -- legacy/taproot over rpc/electrum. Each
// taker funds a fresh wallet and runs a 2-maker openswap.
//
// Live-only: gated behind OPENSWAP_LIVE_TESTS=1 (needs the Docker stack + a
// built native addon), otherwise skipped.

const liveTestsEnabled = process.env.OPENSWAP_LIVE_TESTS === '1'

// Sats swapped per taker; funded with 4x this (1.0 BTC across 4 addresses).
const SWAP_AMOUNT = 500_000
const MAKER_COUNT = 2
const MAKER_READY_ATTEMPTS = 3
const MAKER_READY_RETRY_MS = 10_000
const MAKER_CONTAINERS = ['openswap-makerd1', 'openswap-makerd2']
const WALLET_PASSWORD = 'ffi-live-test-wallet-password'

const RPC_AUTH_ARGS = ['-regtest', '-rpcport=18442', '-rpcuser=user', '-rpcpassword=password']

function bitcoinCli(args: string[]): string {
  return execFileSync('docker', ['exec', 'openswap-bitcoind', 'bitcoin-cli', ...args], {
    encoding: 'utf8',
    stdio: ['ignore', 'pipe', 'pipe'],
  }).trim()
}

function cleanupWallet(walletName: string, dataDir: string) {
  fs.rmSync(dataDir, { recursive: true, force: true })
  try {
    bitcoinCli([...RPC_AUTH_ARGS, 'unloadwallet', walletName])
  } catch {
    // Ignore missing wallet errors.
  }
}

function localMakerAddresses(): string[] {
  const marker = 'Generated new Tor Hidden Service Hostname:'

  return MAKER_CONTAINERS.map((container) => {
    const result = spawnSync('docker', ['logs', container], { encoding: 'utf8' })
    if (result.status !== 0) {
      throw new Error(`${container}: failed to read maker logs: ${result.stderr}`)
    }

    const matches = `${result.stdout}\n${result.stderr}`
      .split('\n')
      .filter((line) => line.includes(marker))
      .map((line) => line.split(marker, 2)[1].trim().split(/\s+/, 1)[0])

    const address = matches.at(-1)
    if (!address) throw new Error(`${container}: maker onion address not found in logs`)
    return address
  })
}

function suitableMakers(offerbook: OfferBook, protocol: string) {
  return offerbook.makers.filter(
    (maker) =>
      maker.state.stateType === 'Good' &&
      maker.protocol !== undefined &&
      [protocol, 'Unified'].includes(maker.protocol.protocolType) &&
      maker.offer !== undefined &&
      maker.offer.minSize <= SWAP_AMOUNT &&
      SWAP_AMOUNT <= maker.offer.maxSize,
  )
}

async function waitForSuitableMakers(taker: Taker, name: string, protocol: string, makerAddresses: string[]) {
  let offerbook: OfferBook | undefined

  for (let attempt = 1; attempt <= MAKER_READY_ATTEMPTS; attempt += 1) {
    for (const address of makerAddresses) {
      try {
        console.log(`  polling local maker ${address}`)
        await taker.pollMakerAsync(address)
      } catch (error) {
        console.log(`  poll failed for ${address}: ${String(error)}`)
      }
    }

    offerbook = taker.fetchOffers()
    const suitable = suitableMakers(offerbook, protocol)
    console.log(
      `${name}: offerbook attempt ${attempt}/${MAKER_READY_ATTEMPTS}: ` +
        `${offerbook.makers.length} total, ${suitable.length} suitable ${protocol} makers`,
    )
    for (const maker of offerbook.makers) {
      const makerProtocol = maker.protocol?.protocolType ?? 'None'
      const amountRange = maker.offer ? `${maker.offer.minSize}..${maker.offer.maxSize} sats` : 'no offer'
      console.log(
        `  ${maker.address.address}: state=${maker.state.stateType}, ` +
          `protocol=${makerProtocol}, amount=${amountRange}`,
      )
    }

    if (suitable.length >= MAKER_COUNT) return
    if (attempt < MAKER_READY_ATTEMPTS) await sleep(MAKER_READY_RETRY_MS)
  }

  const count = offerbook ? suitableMakers(offerbook, protocol).length : 0
  throw new Error(
    `${name}: expected ${MAKER_COUNT} suitable ${protocol} makers for ` + `${SWAP_AMOUNT} sats, found ${count}`,
  )
}

function fund(address: string) {
  bitcoinCli([...RPC_AUTH_ARGS, '-rpcwallet=test', 'sendtoaddress', address, '0.25'])
}

function sleep(ms: number) {
  return new Promise((resolve) => setTimeout(resolve, ms))
}

// Sync until spendable reaches `target`, tolerating Electrum indexing lag.
async function waitForSpendable(taker: Taker, target: number) {
  let balances = taker.getBalances()
  for (let i = 0; i < 30; i += 1) {
    taker.syncAndSave()
    balances = taker.getBalances()
    if (balances.spendable >= target) return balances
    await sleep(3_000)
  }
  return balances
}

type Backend = 'rpc' | 'electrum'

type SwapCase = {
  name: string
  backend: Backend
  protocol: 'Legacy' | 'Taproot'
  addressType: AddressType
}

const CASES: SwapCase[] = [
  { name: 'legacy_rpc', backend: 'rpc', protocol: 'Legacy', addressType: AddressType.P2WPKH },
  { name: 'taproot_rpc', backend: 'rpc', protocol: 'Taproot', addressType: AddressType.P2TR },
  { name: 'legacy_electrum', backend: 'electrum', protocol: 'Legacy', addressType: AddressType.P2WPKH },
  { name: 'taproot_electrum', backend: 'electrum', protocol: 'Taproot', addressType: AddressType.P2TR },
]

const runOrSkip = liveTestsEnabled ? test.serial : test.serial.skip
const requestedCase = process.env.OPENSWAP_SWAP_CASE
const selectedCases = requestedCase ? CASES.filter(({ name }) => name === requestedCase) : CASES

if (liveTestsEnabled && selectedCases.length === 0) {
  throw new Error(
    `Unknown OPENSWAP_SWAP_CASE=${JSON.stringify(requestedCase)}; expected one of: ` +
      CASES.map(({ name }) => name).join(', '),
  )
}

for (const { name, backend, protocol, addressType } of selectedCases) {
  runOrSkip(name, async (t) => {
    t.timeout(10 * 60 * 1000)
    console.log(`\n=== ${name} (${backend} / ${protocol}) ===`)

    const dataDir = path.join(os.homedir(), '.openswap', 'taker', name)
    cleanupWallet(name, dataDir)

    const rpcConfig: RpcConfig | undefined =
      backend === 'rpc'
        ? { url: 'localhost:18442', username: 'user', password: 'password', walletName: name }
        : undefined
    const backendConfig: BackendConfig | undefined =
      backend === 'electrum' ? { kind: 'electrum', url: 'tcp://localhost:50001' } : undefined

    const makerAddresses = localMakerAddresses()

    const taker = new Taker(
      dataDir,
      name,
      rpcConfig,
      9051,
      'openswap',
      'tcp://127.0.0.1:28332',
      WALLET_PASSWORD,
      backendConfig,
    )

    await waitForSuitableMakers(taker, name, protocol, makerAddresses)

    // Fund with 0.25 BTC across 4 fresh external addresses (1.0 BTC total).
    for (let i = 0; i < 4; i += 1) {
      const address = taker.getNextExternalAddress(addressType)
      fund(address.address)
    }
    await sleep(1_000)

    const target = SWAP_AMOUNT * 2
    const funded = await waitForSpendable(taker, target)
    t.true(funded.spendable >= target, `${name}: spendable ${funded.spendable} < target ${target}`)

    const swapId = taker.prepareOpenswap({
      protocol,
      sendAmount: SWAP_AMOUNT,
      makerCount: 2,
      txCount: 1,
      maxInputBudget: 2,
      feerate: 1,
      requiredConfirms: 1,
      preferredMakers: makerAddresses,
      paymentAddress: undefined,
    })
    const report = taker.startOpenswap(swapId)

    t.is(report.makersCount ?? 0, 2, `${name}: should route through 2 makers`)
    t.true(report.status.toUpperCase().includes('SUCCESS'), `${name}: swap status was ${report.status}`)

    console.log(`✓ ${name} passed (swap_id ${report.swapId})`)
  })
}
