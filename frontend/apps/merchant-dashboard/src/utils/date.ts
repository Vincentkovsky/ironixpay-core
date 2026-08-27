import dayjs from 'dayjs';

/**
 * Standard date formats for the merchant dashboard.
 *
 * Table/list views:  YYYY-MM-DD HH:mm     (compact, 24h)
 * Detail/audit views: YYYY-MM-DD HH:mm:ss  (with seconds)
 */
export function formatDateTime(value?: string | number | Date): string {
    if (!value) return '—';
    return dayjs(value).format('YYYY-MM-DD HH:mm');
}

export function formatDateTimeFull(value?: string | number | Date): string {
    if (!value) return '—';
    return dayjs(value).format('YYYY-MM-DD HH:mm:ss');
}
