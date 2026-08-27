/**
 * Extract the machine-readable `error.code` from an Axios error response.
 *
 * Backend returns: `{ error: { code: "insufficient_balance", message: "..." } }`
 */
export function getErrorCode(err: unknown): string {
    return (err as any)?.response?.data?.error?.code || '';
}

/**
 * Resolve a user-facing error message from an Axios error.
 *
 * Priority: error.code → `error.api.${code}` i18n key → raw backend message → fallback i18n key.
 */
export function resolveErrorMessage(
    err: unknown,
    t: (key: string) => string,
    fallbackKey: string,
): string {
    const code = getErrorCode(err);
    const i18nKey = code ? `error.api.${code}` : '';
    // Don't use te() — fails with flat dot-notation keys in legacy:false mode.
    const translated = i18nKey ? t(i18nKey) : '';
    if (translated && translated !== i18nKey) return translated;

    // Fallback: raw backend message (already i18n-resolved by interceptor if available)
    const raw: string =
        (err as any)?._backendMessage ||
        (err as any)?.response?.data?.error?.message ||
        '';
    if (raw) return raw;

    return t(fallbackKey);
}
