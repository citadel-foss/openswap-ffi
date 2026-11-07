import installer from './NativeOpenswapReactNative'
import openswapBindings, {
  Taker,
  restoreWalletFromBackup as generatedRestoreWalletFromBackup,
  restoreWalletFromSeedPhrase as generatedRestoreWalletFromSeedPhrase,
  setupLogging as generatedSetupLogging,
  validateSeedPhrase as generatedValidateSeedPhrase,
  type Address,
  type BackendConfig,
  type Balances,
  type BondHint,
  type RestoreOptions,
  type RpcConfig,
  type SwapParams,
  type SwapReport,
  type TakerLike,
} from './generated/openswap'

export type {
  RpcConfig,
  BackendConfig,
  Balances,
  SwapReport,
  Address,
  SwapParams,
  RestoreOptions,
  BondHint,
}

export const AddressType = {
  P2WPKH: 'P2WPKH',
  P2TR: 'P2TR',
} as const
export type AddressType = (typeof AddressType)[keyof typeof AddressType]

export type TakerInitConfig = {
  dataDir?: string | null
  walletFileName?: string | null
  rpcConfig?: RpcConfig | null
  controlPort?: number | null
  torAuthPassword?: string | null
  zmqAddr: string
  password?: string | null
  nostrRelays?: string[] | null
  backendConfig?: BackendConfig | null
  checkBlocklist?: boolean | null
}

export type RestoreFromBackupConfig = {
  dataDir?: string | null
  walletFileName?: string | null
  backendConfig: BackendConfig
  backupFilePath: string
  /** Backup password. It also becomes the restored wallet's password. */
  password: string
  options?: RestoreOptions | null
}

export type RestoreFromSeedPhraseConfig = {
  dataDir?: string | null
  walletFileName: string
  backendConfig: BackendConfig
  phrase: string
  /** Password for the restored wallet. */
  password: string
  options?: RestoreOptions | null
}

export class OpenswapTaker {
  private constructor(private readonly taker: TakerLike) {}

  static async setupLogging(
    dataDir: string | null | undefined,
    level: string,
    toStdout: boolean = false,
  ): Promise<void> {
    generatedSetupLogging(dataDir ?? undefined, level, toStdout)
  }

  static async init(config: TakerInitConfig): Promise<OpenswapTaker> {

    const taker = Taker.init(
      config.dataDir ?? undefined,
      config.walletFileName ?? undefined,
      config.rpcConfig ?? undefined,
      config.controlPort ?? undefined,
      config.torAuthPassword ?? undefined,
      config.zmqAddr,
      config.password ?? undefined,
      config.nostrRelays ?? undefined,
      config.backendConfig ?? undefined,
      config.checkBlocklist ?? undefined,
    )
    return new OpenswapTaker(taker)
  }

  /** Checks the words and checksum of a seed phrase. Rejects if it is invalid. */
  static async validateSeedPhrase(phrase: string): Promise<void> {
    generatedValidateSeedPhrase(phrase)
  }

  /** Restores a wallet from an encrypted backup file. Rejects if the restore fails. */
  static async restoreWalletFromBackup(config: RestoreFromBackupConfig): Promise<void> {
    generatedRestoreWalletFromBackup(
      config.dataDir ?? undefined,
      config.walletFileName ?? undefined,
      config.backendConfig,
      config.backupFilePath,
      config.password,
      config.options ?? undefined,
    )
  }

  /** Restores a wallet from a seed phrase. Rejects if the restore fails. */
  static async restoreWalletFromSeedPhrase(config: RestoreFromSeedPhraseConfig): Promise<void> {
    generatedRestoreWalletFromSeedPhrase(
      config.dataDir ?? undefined,
      config.walletFileName,
      config.backendConfig,
      config.phrase,
      config.password,
      config.options ?? undefined,
    )
  }

  /**
   * Returns the wallet's seed phrase to show the user. Needs the wallet password
   * on every call. Show it once, then drop it. Never log or store it.
   */
  async getSeedPhrase(walletPassword: string): Promise<string> {
    return this.taker.getSeedPhrase(walletPassword)
  }

  /** Records that the user wrote the seed phrase down. */
  async confirmSeedBackup(walletPassword: string): Promise<void> {
    this.taker.confirmSeedBackup(walletPassword)
  }

  /** Stores the user's original seed phrase in a wallet that has none. */
  async attachSeedPhrase(walletPassword: string, phrase: string): Promise<void> {
    this.taker.attachSeedPhrase(walletPassword, phrase)
  }

  async isSeedBackedUp(): Promise<boolean> {
    return this.taker.isSeedBackedUp()
  }

  async dispose(): Promise<void> {
    const disposable = this.taker as unknown as { uniffiDestroy?: () => void }
    disposable.uniffiDestroy?.()
  }

  async syncOfferbookAndWait(): Promise<void> {
    this.taker.syncOfferbookAndWait()
  }

  async syncAndSave(): Promise<void> {
    this.taker.syncAndSave()
  }

  async getBalances(): Promise<Balances> {
    return this.taker.getBalances()
  }

  async getNextExternalAddress(addressType: AddressType): Promise<Address> {
    return this.taker.getNextExternalAddress({ addrType: addressType })
  }

  async prepareOpenswap(swapParams: SwapParams): Promise<string> {
    return this.taker.prepareOpenswap(swapParams)
  }

  async startOpenswap(swapId: string): Promise<SwapReport> {
    return this.taker.startOpenswap(swapId)
  }
}
