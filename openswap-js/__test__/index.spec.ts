import test from 'ava'
import * as nativeBinding from '../index'

const importedBinding = nativeBinding as unknown as Record<string, unknown>
// Node exposes CommonJS modules through an ESM namespace with an interop-only
// `default` key. Assert against the native module itself, not that wrapper.
const binding = (importedBinding.default ?? importedBinding) as Record<string, unknown>

test('native module exports the complete runtime surface', (t) => {
  const publicExports = Object.keys(binding).filter((key) => key !== '__napiBindingTarget')
  t.deepEqual(publicExports.sort(), ['AddressType', 'Taker', 'TakerBehavior', 'TakerError'])
  const addressTypes = binding.AddressType as Record<string, number>
  t.deepEqual(Object.getOwnPropertyNames(addressTypes), ['P2WPKH', 'P2TR'])
  t.is(addressTypes.P2WPKH, 0)
  t.is(addressTypes.P2TR, 1)

  const behaviors = binding.TakerBehavior as Record<string, number>
  t.deepEqual(Object.getOwnPropertyNames(behaviors), [
    'Normal',
    'DropConnectionAfterFullSetup',
    'BroadcastContractAfterFullSetup',
  ])
  t.is(behaviors.Normal, 0)
  t.is(behaviors.DropConnectionAfterFullSetup, 1)
  t.is(behaviors.BroadcastContractAfterFullSetup, 2)

  const errors = binding.TakerError as Record<string, number>
  t.deepEqual(Object.getOwnPropertyNames(errors), ['Wallet', 'Protocol', 'Network', 'General', 'IO'])
  t.is(errors.Wallet, 0)
  t.is(errors.Protocol, 1)
  t.is(errors.Network, 2)
  t.is(errors.General, 3)
  t.is(errors.IO, 4)
})

test('Taker exposes every documented static and instance method', (t) => {
  const taker = binding.Taker as {
    prototype: object
    [key: string]: unknown
  }
  const staticMethods = Object.getOwnPropertyNames(taker)
    .filter((name) => typeof taker[name] === 'function')
    .sort()
  const instanceMethods = Object.getOwnPropertyNames(taker.prototype)
    .filter((name) => name !== 'constructor')
    .sort()

  t.deepEqual(staticMethods, [
    'initNativeLogging',
    'isWalletEncrypted',
    'restoreWalletFromBackup',
    'restoreWalletFromSeedPhrase',
    'restoreWalletGuiApp',
    'setupLogging',
    'validateSeedPhrase',
  ])
  t.deepEqual(instanceMethods, [
    'attachSeedPhrase',
    'backup',
    'confirmSeedBackup',
    'displayOffer',
    'fetchAllMakers',
    'fetchOffers',
    'getBalances',
    'getName',
    'getNextExternalAddress',
    'getNextInternalAddresses',
    'getSeedPhrase',
    'getTransactions',
    'isSeedBackedUp',
    'listAllUtxoSpendInfo',
    'lockUnspendableUtxos',
    'pollMakerAsync',
    'prepareOpenswap',
    'recoverActiveSwap',
    'removeMaker',
    'sendToAddress',
    'startOpenswap',
    'syncAndSave',
    'syncOfferbookAndWait',
    'syncOfferbookAndWaitAsync',
    'verifyDeniability',
  ])
})

test('seed phrase validation and restore errors reach JS callers', (t) => {
  const taker = binding.Taker as {
    validateSeedPhrase(phrase: string): void
    restoreWalletFromSeedPhrase(...args: unknown[]): void
    restoreWalletFromBackup(...args: unknown[]): void
    restoreWalletGuiApp(...args: unknown[]): void
  }
  const valid = `${'abandon '.repeat(11)}about`
  t.notThrows(() => taker.validateSeedPhrase(valid))
  t.throws(() => taker.validateSeedPhrase('abandon abandon abandon'), {
    message: /Invalid seed phrase/,
  })

  const backend = { kind: 'rpc' }
  // An empty password is refused before any file or node is touched. The
  // restore must throw instead of silently logging the failure.
  t.throws(() => taker.restoreWalletFromSeedPhrase(null, 'wallet', backend, valid, ''), {
    message: /passphrase/,
  })
  t.throws(
    () => taker.restoreWalletFromBackup(null, null, backend, '/nonexistent/backup.json', ''),
    { message: /passphrase/ },
  )
  t.throws(
    () =>
      taker.restoreWalletGuiApp(
        null,
        null,
        { url: 'http://127.0.0.1:1', username: 'u', password: 'p', walletName: 'w' },
        '/nonexistent/backup.json',
        null,
      ),
    { message: /passphrase/ },
  )
  t.throws(
    () =>
      taker.restoreWalletFromSeedPhrase(null, 'wallet', backend, valid, 'pw', {
        bondHints: [{ index: 2 ** 31, locktime: 900 }],
      }),
    { message: /bond hint index/ },
  )
})
