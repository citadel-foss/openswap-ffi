import installer from './NativeOpenswapReactNative'
import openswapBindings, {
  Taker,
  setupLogging as generatedSetupLogging,
  type Address,
  type BackendConfig,
  type Balances,
  type RpcConfig,
  type SwapParams,
  type SwapReport,
  type TakerLike,
} from './generated/openswap'

export type { RpcConfig, BackendConfig, Balances, SwapReport, Address, SwapParams }

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
