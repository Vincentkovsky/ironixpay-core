/**
 * Multi-chain block explorer URL utilities.
 *
 * Supports TRON, BSC, Ethereum, OP, Base, Arbitrum, Polygon.
 * Sandbox  → testnet explorer
 * Live     → mainnet explorer
 */
import { useEnvironmentStore } from '@/stores';

/** Explorer base URLs per network (mainnet / testnet) */
const EXPLORER_MAP: Record<string, { mainnet: string; testnet: string }> = {
    TRON: { mainnet: 'https://tronscan.org/#', testnet: 'https://nile.tronscan.org/#' },
    BSC: { mainnet: 'https://bscscan.com', testnet: 'https://testnet.bscscan.com' },
    ETHEREUM: { mainnet: 'https://etherscan.io', testnet: 'https://sepolia.etherscan.io' },
    OP: { mainnet: 'https://optimistic.etherscan.io', testnet: 'https://sepolia-optimism.etherscan.io' },
    BASE: { mainnet: 'https://basescan.org', testnet: 'https://sepolia.basescan.org' },
    ARBITRUM: { mainnet: 'https://arbiscan.io', testnet: 'https://sepolia.arbiscan.io' },
    POLYGON: { mainnet: 'https://polygonscan.com', testnet: 'https://amoy.polygonscan.com' },
    SOLANA: { mainnet: 'https://solscan.io', testnet: 'https://solscan.io?cluster=devnet' },
};

function getBaseUrl(network?: string): string {
    const envStore = useEnvironmentStore();
    const n = (network || '').toUpperCase();

    // Match by prefix/includes for flexibility (e.g. "BSC Mainnet" → BSC)
    for (const [key, urls] of Object.entries(EXPLORER_MAP)) {
        if (n.includes(key)) {
            return envStore.isSandbox ? urls.testnet : urls.mainnet;
        }
    }

    // Fallback: TRON
    return envStore.isSandbox
        ? 'https://nile.tronscan.org/#'
        : 'https://tronscan.org/#';
}
/** True if the network uses TRON-style explorer paths (/transaction/ instead of /tx/) */
function isTronNetwork(network?: string): boolean {
    const n = (network || '').toUpperCase();
    return n.includes('TRON');
}

/** True if the network is Solana (uses /account/ instead of /address/) */
function isSolanaNetwork(network?: string): boolean {
    return (network || '').toUpperCase().includes('SOLANA');
}

/** Full URL to a transaction on the explorer */
export function txUrl(hash: string, network?: string): string {
    const base = getBaseUrl(network);
    if (isTronNetwork(network)) {
        return `${base}/transaction/${hash}`;
    }
    // EVM + Solana both use /tx/
    return `${base}/tx/${hash}`;
}

/** Full URL to an address on the explorer */
export function addressUrl(addr: string, network?: string): string {
    const base = getBaseUrl(network);
    if (isSolanaNetwork(network)) {
        return `${base}/account/${addr}`;
    }
    return `${base}/address/${addr}`;
}

/** Open a transaction or address in a new tab */
export function openExplorer(
    value: string,
    type: 'tx' | 'address' = 'tx',
    network?: string,
): void {
    const url = type === 'address' ? addressUrl(value, network) : txUrl(value, network);
    window.open(url, '_blank', 'noopener');
}
