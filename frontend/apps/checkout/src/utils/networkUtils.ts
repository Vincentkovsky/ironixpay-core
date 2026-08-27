/**
 * Network display name utilities.
 *
 * Backend returns raw network identifiers (TRON, BSC) —
 * consistent between API responses, webhooks, and database.
 * This module provides the UI-only display formatting.
 */

interface NetworkMeta {
    mainnet: string;
    testnet: string;
    tokenStandard: string;
    /** Block explorer address page base URL (mainnet) */
    explorerAddress: string;
}

const NETWORKS: Record<string, NetworkMeta> = {
    TRON: { mainnet: 'TRON Mainnet', testnet: 'TRON Nile', tokenStandard: 'TRC20', explorerAddress: 'https://tronscan.org/#/contract' },
    BSC: { mainnet: 'BSC Mainnet', testnet: 'BSC Testnet', tokenStandard: 'BEP20', explorerAddress: 'https://bscscan.com/address' },
    ETHEREUM: { mainnet: 'Ethereum Mainnet', testnet: 'Ethereum Sepolia', tokenStandard: 'ERC20', explorerAddress: 'https://etherscan.io/address' },
    POLYGON: { mainnet: 'Polygon Mainnet', testnet: 'Polygon Amoy', tokenStandard: 'ERC20', explorerAddress: 'https://polygonscan.com/address' },
    ARBITRUM: { mainnet: 'Arbitrum One', testnet: 'Arbitrum Sepolia', tokenStandard: 'ERC20', explorerAddress: 'https://arbiscan.io/address' },
    BASE: { mainnet: 'Base Mainnet', testnet: 'Base Sepolia', tokenStandard: 'ERC20', explorerAddress: 'https://basescan.org/address' },
    OPTIMISM: { mainnet: 'OP Mainnet', testnet: 'OP Sepolia', tokenStandard: 'ERC20', explorerAddress: 'https://optimistic.etherscan.io/address' },
    SOLANA: { mainnet: 'Solana Mainnet', testnet: 'Solana Devnet', tokenStandard: 'SPL', explorerAddress: 'https://solscan.io/account' },
};

export function networkDisplayName(network: string, isSandbox: boolean): string {
    const meta = NETWORKS[network];
    if (!meta) return network;
    return isSandbox ? meta.testnet : meta.mainnet;
}

export function tokenStandard(network: string): string {
    return NETWORKS[network]?.tokenStandard ?? 'USDT';
}

export function isEvmNetwork(network: string): boolean {
    return !!network && network !== 'TRON' && network !== 'SOLANA';
}

/** Build block explorer URL for a contract address */
export function contractExplorerUrl(network: string, contract: string): string | null {
    const meta = NETWORKS[network];
    if (!meta || !contract) return null;
    return `${meta.explorerAddress}/${contract}`;
}
