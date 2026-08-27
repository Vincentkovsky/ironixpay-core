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
}

const NETWORKS: Record<string, NetworkMeta> = {
    TRON: { mainnet: 'TRON Mainnet', testnet: 'TRON Nile', tokenStandard: 'TRC20' },
    BSC: { mainnet: 'BSC Mainnet', testnet: 'BSC Testnet', tokenStandard: 'BEP20' },
    ETHEREUM: { mainnet: 'Ethereum Mainnet', testnet: 'Ethereum Sepolia', tokenStandard: 'ERC20' },
    POLYGON: { mainnet: 'Polygon Mainnet', testnet: 'Polygon Amoy', tokenStandard: 'ERC20' },
    ARBITRUM: { mainnet: 'Arbitrum One', testnet: 'Arbitrum Sepolia', tokenStandard: 'ERC20' },
    BASE: { mainnet: 'Base Mainnet', testnet: 'Base Sepolia', tokenStandard: 'ERC20' },
    OPTIMISM: { mainnet: 'OP Mainnet', testnet: 'OP Sepolia', tokenStandard: 'ERC20' },
    SOLANA: { mainnet: 'Solana Mainnet', testnet: 'Solana Devnet', tokenStandard: 'SPL' },
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
