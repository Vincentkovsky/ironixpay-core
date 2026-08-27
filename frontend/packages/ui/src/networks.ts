/**
 * Network icon SVG assets.
 * Import from '@ironix-pay/ui/networks' to get URL paths to branded SVGs.
 *
 * Usage:
 *   import { networkIcons } from '@ironix-pay/ui'
 *   <img :src="networkIcons.TRON" alt="TRON" />
 */

import tron from './assets/networks/tron.svg?url'
import bsc from './assets/networks/bsc.svg?url'
import ethereum from './assets/networks/ethereum.svg?url'
import polygon from './assets/networks/polygon.svg?url'
import arb from './assets/networks/arb.svg?url'
import op from './assets/networks/op.svg?url'
import base from './assets/networks/base.svg?url'
import solana from './assets/networks/solana.svg?url'

/** Map canonical network enum values to their branded SVG icon URLs */
export const networkIcons: Record<string, string> = {
    TRON: tron,
    TRON_NILE: tron,
    BSC: bsc,
    ETHEREUM: ethereum,
    POLYGON: polygon,
    ARBITRUM: arb,
    OPTIMISM: op,
    BASE: base,
    SOLANA: solana,
}

/** Ordered list for display purposes */
export const networkList = [
    { key: 'TRON', name: 'TRON', icon: tron },
    { key: 'BSC', name: 'BSC', icon: bsc },
    { key: 'ETHEREUM', name: 'Ethereum', icon: ethereum },
    { key: 'POLYGON', name: 'Polygon', icon: polygon },
    { key: 'ARBITRUM', name: 'Arbitrum', icon: arb },
    { key: 'OPTIMISM', name: 'Optimism', icon: op },
    { key: 'BASE', name: 'Base', icon: base },
    { key: 'SOLANA', name: 'Solana', icon: solana },
]
