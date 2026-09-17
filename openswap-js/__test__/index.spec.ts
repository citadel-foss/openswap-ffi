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
    'fetchMempoolFees',
    'initNativeLogging',
    'isWalletEncrypted',
    'restoreWalletGuiApp',
    'setupLogging',
  ])
  t.deepEqual(instanceMethods, [
    'backup',
    'displayOffer',
    'fetchAllMakers',
    'fetchOffers',
    'getBalances',
    'getName',
    'getNextExternalAddress',
    'getNextInternalAddresses',
    'getTransactions',
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
