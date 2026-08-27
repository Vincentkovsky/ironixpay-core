/**
 * Currency display utilities
 *
 * Backend returns amounts as human-readable decimal strings (e.g., "10.5").
 * These utilities format them for display with proper decimal trimming.
 *
 * Smart formatting: show up to 6 decimal places, minimum 2, trim trailing zeros.
 *   300.5      → "300.50"
 *   1.505      → "1.505"
 *   0.123456   → "0.123456"
 */

/**
 * Format a display-ready number with smart decimal trimming.
 * Shows up to `maxDec` decimals but trims trailing zeros, keeping at least `minDec`.
 *
 * @param value - Numeric value already in display units
 * @param maxDec - Maximum decimal places (default: 6)
 * @param minDec - Minimum decimal places (default: 2)
 */
export function fmtAmt(
    value: number | null | undefined,
    maxDec = 6,
    minDec = 2,
): string {
    if (value === null || value === undefined) return '0.00';
    const fixed = value.toFixed(maxDec);
    const [int, dec = ''] = fixed.split('.');
    const trimmed = (dec || '').replace(/0+$/, '').padEnd(minDec, '0');
    return `${int}.${trimmed}`;
}

/**
 * Format an amount for display (with thousands separators).
 * Smart trimming: min 2, max 6 decimal places.
 *
 * Backend already returns human-readable strings, so no division is needed.
 *
 * @param amount - Amount as number, string, or bigint (already in standard units)
 * @returns Formatted string (e.g., "1,234.50")
 */
export function formatUsdt(
    amount: number | string | bigint | null | undefined,
): string {
    if (amount === null || amount === undefined) {
        return '0.00';
    }
    const num = Number(amount);
    if (isNaN(num)) return String(amount);
    return num.toLocaleString('en-US', {
        minimumFractionDigits: 2,
        maximumFractionDigits: 6,
    });
}

/**
 * Convert amount to number (for calculations)
 * @param amount - Amount as number, string, or bigint (already in standard units)
 * @returns Decimal number (e.g., 1234.56)
 */
export function toNumber(
    amount: number | string | bigint | null | undefined,
): number {
    if (amount === null || amount === undefined) {
        return 0;
    }
    return Number(amount);
}

/**
 * Format with + or - prefix for transaction amounts (with thousands separators).
 * Smart trimming: min 2, max 6 decimal places.
 *
 * @param amount - Amount (positive or negative, already in standard units)
 * @returns Formatted string with sign (e.g., "+1,234.50" or "-100.00")
 */
export function formatUsdtSigned(
    amount: number | string | bigint | null | undefined,
): string {
    if (amount === null || amount === undefined) {
        return '+0.00';
    }
    const num = Number(amount);
    if (isNaN(num)) return String(amount);
    const prefix = num >= 0 ? '+' : '';
    return (
        prefix +
        num.toLocaleString('en-US', {
            minimumFractionDigits: 2,
            maximumFractionDigits: 6,
        })
    );
}
