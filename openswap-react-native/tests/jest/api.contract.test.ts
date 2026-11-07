const mockSetupLogging = jest.fn()
const mockInit = jest.fn()
const mockValidateSeedPhrase = jest.fn()
const mockRestoreFromBackup = jest.fn()
const mockRestoreFromSeedPhrase = jest.fn()
const mockNativeTaker = {
  uniffiDestroy: jest.fn(),
  syncOfferbookAndWait: jest.fn(),
  syncAndSave: jest.fn(),
  getBalances: jest.fn(),
  getNextExternalAddress: jest.fn(),
  prepareOpenswap: jest.fn(),
  startOpenswap: jest.fn(),
  getSeedPhrase: jest.fn(),
  confirmSeedBackup: jest.fn(),
  attachSeedPhrase: jest.fn(),
  isSeedBackedUp: jest.fn(),
}

jest.mock(
  '../../src/NativeOpenswapReactNative',
  () => ({
    __esModule: true,
    default: { installRustCrate: jest.fn(() => true) },
  }),
  { virtual: true },
)

jest.mock(
  '../../src/generated/openswap',
  () => ({
    __esModule: true,
    default: {},
    setupLogging: mockSetupLogging,
    validateSeedPhrase: mockValidateSeedPhrase,
    restoreWalletFromBackup: mockRestoreFromBackup,
    restoreWalletFromSeedPhrase: mockRestoreFromSeedPhrase,
    Taker: { init: mockInit },
  }),
  { virtual: true },
)

import { AddressType, OpenswapTaker } from '../../src'

describe('React Native public API contract', () => {
  beforeEach(() => {
    jest.clearAllMocks()
    mockInit.mockReturnValue(mockNativeTaker)
  })

  test('exposes only the supported wallet address types', () => {
    expect(AddressType).toEqual({ P2WPKH: 'P2WPKH', P2TR: 'P2TR' })
  })

  test('forwards every initialization option in canonical FFI order', async () => {
    const rpcConfig = {
      url: 'http://node:8332',
      username: 'alice',
      password: 'secret',
      walletName: 'wallet',
    }
    const backendConfig = { kind: 'electrum', url: 'ssl://electrum.example:50002' }

    await OpenswapTaker.init({
      dataDir: '/tmp/taker',
      walletFileName: 'wallet.json',
      rpcConfig,
      controlPort: 9051,
      torAuthPassword: 'tor-secret',
      zmqAddr: 'tcp://node:28332',
      password: 'wallet-secret',
      nostrRelays: ['ws://relay.example'],
      backendConfig,
      checkBlocklist: true,
    })

    expect(mockInit).toHaveBeenCalledWith(
      '/tmp/taker',
      'wallet.json',
      rpcConfig,
      9051,
      'tor-secret',
      'tcp://node:28332',
      'wallet-secret',
      ['ws://relay.example'],
      backendConfig,
      true,
    )
  })

  test('normalizes optional values and forwards every wrapper method', async () => {
    mockNativeTaker.getBalances.mockReturnValue({ spendable: 50_000n })
    mockNativeTaker.getNextExternalAddress.mockReturnValue({ addr: 'bc1qexample' })
    mockNativeTaker.prepareOpenswap.mockReturnValue('swap-1')
    mockNativeTaker.startOpenswap.mockReturnValue({ swapId: 'swap-1', status: 'SUCCESS' })

    await OpenswapTaker.setupLogging(null, 'info', true)
    const taker = await OpenswapTaker.init({ zmqAddr: 'tcp://node:28332' })
    await taker.syncOfferbookAndWait()
    await taker.syncAndSave()
    await expect(taker.getBalances()).resolves.toEqual({ spendable: 50_000n })
    await expect(taker.getNextExternalAddress(AddressType.P2TR)).resolves.toEqual({
      addr: 'bc1qexample',
    })
    const params = { protocol: 'Taproot', sendAmount: 50_000n, makerCount: 2 }
    await expect(taker.prepareOpenswap(params)).resolves.toBe('swap-1')
    await expect(taker.startOpenswap('swap-1')).resolves.toEqual({
      swapId: 'swap-1',
      status: 'SUCCESS',
    })
    await taker.dispose()

    expect(mockSetupLogging).toHaveBeenCalledWith(undefined, 'info', true)
    expect(mockInit).toHaveBeenCalledWith(
      undefined,
      undefined,
      undefined,
      undefined,
      undefined,
      'tcp://node:28332',
      undefined,
      undefined,
      undefined,
      undefined,
    )
    expect(mockNativeTaker.syncOfferbookAndWait).toHaveBeenCalledTimes(1)
    expect(mockNativeTaker.syncAndSave).toHaveBeenCalledTimes(1)
    expect(mockNativeTaker.getNextExternalAddress).toHaveBeenCalledWith({ addrType: 'P2TR' })
    expect(mockNativeTaker.prepareOpenswap).toHaveBeenCalledWith(params)
    expect(mockNativeTaker.startOpenswap).toHaveBeenCalledWith('swap-1')
    expect(mockNativeTaker.uniffiDestroy).toHaveBeenCalledTimes(1)
  })

  test('forwards seed phrase calls and restores with normalized options', async () => {
    mockNativeTaker.getSeedPhrase.mockReturnValue('abandon about')
    mockNativeTaker.isSeedBackedUp.mockReturnValue(true)
    const taker = await OpenswapTaker.init({ zmqAddr: 'tcp://node:28332' })

    await expect(taker.getSeedPhrase('wallet-secret')).resolves.toBe('abandon about')
    await taker.confirmSeedBackup('wallet-secret')
    await taker.attachSeedPhrase('wallet-secret', 'abandon about')
    await expect(taker.isSeedBackedUp()).resolves.toBe(true)
    expect(mockNativeTaker.getSeedPhrase).toHaveBeenCalledWith('wallet-secret')
    expect(mockNativeTaker.confirmSeedBackup).toHaveBeenCalledWith('wallet-secret')
    expect(mockNativeTaker.attachSeedPhrase).toHaveBeenCalledWith('wallet-secret', 'abandon about')

    await OpenswapTaker.validateSeedPhrase('abandon about')
    expect(mockValidateSeedPhrase).toHaveBeenCalledWith('abandon about')

    const backendConfig = { kind: 'electrum', url: 'ssl://electrum.example:50002' }
    await OpenswapTaker.restoreWalletFromSeedPhrase({
      walletFileName: 'restored',
      backendConfig,
      phrase: 'abandon about',
      password: 'new-secret',
    })
    expect(mockRestoreFromSeedPhrase).toHaveBeenCalledWith(
      undefined,
      'restored',
      backendConfig,
      'abandon about',
      'new-secret',
      undefined,
    )

    const options = { birthday: 800_000n, bondHints: [{ index: 1, locktime: 900 }] }
    await OpenswapTaker.restoreWalletFromBackup({
      dataDir: '/tmp/taker',
      backendConfig,
      backupFilePath: '/tmp/backup.json',
      password: 'backup-secret',
      options,
    })
    expect(mockRestoreFromBackup).toHaveBeenCalledWith(
      '/tmp/taker',
      undefined,
      backendConfig,
      '/tmp/backup.json',
      'backup-secret',
      options,
    )
  })

  test('restore errors reject instead of resolving', async () => {
    mockRestoreFromSeedPhrase.mockImplementation(() => {
      throw new Error('Restore error: a passphrase is required')
    })
    await expect(
      OpenswapTaker.restoreWalletFromSeedPhrase({
        walletFileName: 'restored',
        backendConfig: { kind: 'rpc' },
        phrase: 'abandon about',
        password: '',
      }),
    ).rejects.toThrow('passphrase')
  })
})
