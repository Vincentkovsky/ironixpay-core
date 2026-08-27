/**
 * IronixPay — WooCommerce Blocks Checkout Integration
 *
 * Registers the IronixPay payment method with the Block-based checkout.
 * Supports both USDT and USDC with dynamic network filtering.
 * Uses vanilla JS (no build step required).
 */
(function () {
    'use strict';

    var registerPaymentMethod = window.wc.wcBlocksRegistry.registerPaymentMethod;
    var el = window.wp.element.createElement;
    var useState = window.wp.element.useState;
    var useEffect = window.wp.element.useEffect;
    var getSetting = window.wc.wcSettings.getSetting;
    var decodeEntities = window.wp.htmlEntities.decodeEntities;

    // Settings from PHP
    var settings = getSetting('ironixpay_data', {});
    var title = decodeEntities(settings.title || 'Pay with Crypto');
    var description = decodeEntities(settings.description || '');
    var networks = settings.supported_networks || {};
    var currencies = settings.supported_currencies || {};
    var usdcUnsupported = settings.usdc_unsupported_networks || [];
    var environment = settings.environment || 'sandbox';
    var logoUrl = settings.logo_url || '';
    var icons = settings.network_icons || {};
    var currencyKeys = Object.keys(currencies);
    var allNetworkKeys = Object.keys(networks);

    /**
     * Get available networks for a given currency.
     */
    function getNetworksForCurrency(currency) {
        if (!currency) return allNetworkKeys;
        if (currency !== 'USDC') return allNetworkKeys;
        // Filter out USDC-unsupported networks
        return allNetworkKeys.filter(function (k) {
            return usdcUnsupported.indexOf(k) === -1;
        });
    }

    // --- Styles ---
    var styles = {
        label: { display: 'flex', alignItems: 'center', gap: '8px' },
        desc: { margin: '0 0 14px', color: '#6b7280', fontSize: '13px', lineHeight: '1.5' },
        sectionLabel: { margin: '0 0 8px', fontSize: '13px', fontWeight: '600', color: '#374151' },
        grid: { display: 'grid', gridTemplateColumns: 'repeat(auto-fill, minmax(140px, 1fr))', gap: '8px' },
        card: function (sel) {
            return {
                display: 'flex', alignItems: 'center', gap: '10px',
                padding: '10px 14px', borderRadius: '10px', cursor: 'pointer',
                border: sel ? '2px solid #2563eb' : '1.5px solid #e5e7eb',
                background: sel ? 'linear-gradient(135deg, #eff6ff 0%, #f0f4ff 100%)' : '#fafafa',
                transition: 'all 0.2s ease',
                boxShadow: sel ? '0 0 0 3px rgba(37,99,235,0.1)' : 'none'
            };
        },
        icon: { width: '22px', height: '22px', flexShrink: '0', borderRadius: '50%' },
        cardName: { fontSize: '13px', fontWeight: '600', color: '#1f2937', lineHeight: '1.3' },
        singleWrap: {
            display: 'inline-flex', alignItems: 'center', gap: '8px',
            padding: '8px 14px', borderRadius: '8px',
            background: '#f3f4f6', border: '1px solid #e5e7eb',
            fontSize: '13px', fontWeight: '500', color: '#374151'
        },
        currencyBtn: function (sel) {
            return {
                padding: '8px 20px', borderRadius: '8px', cursor: 'pointer',
                border: sel ? '2px solid #2563eb' : '1.5px solid #e5e7eb',
                background: sel ? '#eff6ff' : '#fafafa',
                fontSize: '14px', fontWeight: '600', color: sel ? '#2563eb' : '#374151',
                transition: 'all 0.2s ease'
            };
        },
        note: { fontSize: '11px', color: '#9ca3af', margin: '6px 0 0', lineHeight: '1.4' }
    };

    /**
     * Label component.
     */
    var Label = function () {
        return el('span', { style: styles.label },
            logoUrl ? el('img', { src: logoUrl, alt: 'IronixPay', style: { height: '20px' } }) : null,
            el('span', { style: { fontWeight: '500' } }, title)
        );
    };

    /**
     * Content component — currency + network selector with chain icons.
     */
    var Content = function (props) {
        var eventRegistration = props.eventRegistration;
        var emitResponse = props.emitResponse;
        var onPaymentSetup = eventRegistration.onPaymentSetup;

        // Default currency: first available
        var defaultCurrency = currencyKeys.length === 1 ? currencyKeys[0] : '';
        var currencyState = useState(defaultCurrency);
        var selectedCurrency = currencyState[0];
        var setSelectedCurrency = currencyState[1];

        // Compute available networks based on selected currency
        var availableNetworkKeys = getNetworksForCurrency(selectedCurrency);
        var defaultNetwork = availableNetworkKeys.length === 1 ? availableNetworkKeys[0] : '';
        var networkState = useState(defaultNetwork);
        var selectedNetwork = networkState[0];
        var setSelectedNetwork = networkState[1];

        // Reset network when currency changes and current selection is invalid
        useEffect(function () {
            var validKeys = getNetworksForCurrency(selectedCurrency);
            if (selectedNetwork && validKeys.indexOf(selectedNetwork) === -1) {
                // Current selection is no longer valid — auto-select if only one remains
                setSelectedNetwork(validKeys.length === 1 ? validKeys[0] : '');
            } else if (!selectedNetwork && validKeys.length === 1) {
                // No selection yet and only one option — auto-select
                setSelectedNetwork(validKeys[0]);
            }
        }, [selectedCurrency]);

        useEffect(function () {
            var unsubscribe = onPaymentSetup(function () {
                if (currencyKeys.length > 1 && !selectedCurrency) {
                    return {
                        type: emitResponse.responseTypes.ERROR,
                        message: 'Please select a payment currency.'
                    };
                }
                if (!selectedNetwork) {
                    return {
                        type: emitResponse.responseTypes.ERROR,
                        message: 'Please select a payment network.'
                    };
                }
                var currency = selectedCurrency || currencyKeys[0] || 'USDT';
                return {
                    type: emitResponse.responseTypes.SUCCESS,
                    meta: {
                        paymentMethodData: {
                            ironixpay_network: selectedNetwork,
                            ironixpay_currency: currency
                        }
                    }
                };
            });
            return unsubscribe;
        }, [onPaymentSetup, selectedNetwork, selectedCurrency, emitResponse.responseTypes]);

        var elements = [];

        if (description) {
            elements.push(el('p', { key: 'desc', style: styles.desc }, description));
        }

        // --- Currency selector (only if multiple currencies) ---
        if (currencyKeys.length > 1) {
            elements.push(
                el('p', { key: 'curr-lbl', style: styles.sectionLabel }, 'Select Currency')
            );
            var currBtns = currencyKeys.map(function (key) {
                var isSel = selectedCurrency === key;
                // Disable USDC in sandbox
                var disabled = key === 'USDC' && environment === 'sandbox';
                return el('button', {
                    key: key,
                    type: 'button',
                    style: Object.assign({}, styles.currencyBtn(isSel), disabled ? { opacity: 0.4, cursor: 'not-allowed' } : {}),
                    onClick: disabled ? undefined : function () { setSelectedCurrency(key); },
                    disabled: disabled
                }, key);
            });
            elements.push(el('div', { key: 'curr-btns', style: { display: 'flex', gap: '8px', marginBottom: '14px' } }, currBtns));

            if (environment === 'sandbox') {
                elements.push(
                    el('p', { key: 'sandbox-note', style: styles.note }, 'USDC is not available in Sandbox mode.')
                );
            }
        }

        // --- Network selector ---
        if (availableNetworkKeys.length === 1) {
            // Single network: auto-select
            var k = availableNetworkKeys[0];
            elements.push(
                el('div', { key: 'single', style: { margin: '4px 0' } },
                    el('span', { style: styles.singleWrap },
                        icons[k] ? el('img', { src: icons[k], alt: k, style: styles.icon }) : null,
                        'Payment on ', el('strong', null, networks[k])
                    )
                )
            );
        } else if (availableNetworkKeys.length > 1) {
            elements.push(
                el('p', { key: 'net-lbl', style: styles.sectionLabel }, 'Select Network')
            );

            var cards = availableNetworkKeys.map(function (key) {
                var isSel = selectedNetwork === key;
                // Show "(USDT only)" hint if both currencies are enabled and this network doesn't support USDC
                var label = networks[key] || key;
                if (currencyKeys.length > 1 && usdcUnsupported.indexOf(key) !== -1) {
                    label += ' (USDT only)';
                }
                return el('div', {
                    key: key,
                    style: styles.card(isSel),
                    onClick: function () { setSelectedNetwork(key); },
                    role: 'radio',
                    'aria-checked': isSel ? 'true' : 'false',
                    tabIndex: '0',
                    onKeyDown: function (e) { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); setSelectedNetwork(key); } }
                },
                    icons[key]
                        ? el('img', { src: icons[key], alt: key, style: styles.icon })
                        : null,
                    el('span', { style: styles.cardName }, label)
                );
            });

            elements.push(el('div', { key: 'grid', style: styles.grid }, cards));
        }

        return el('div', null, elements);
    };

    registerPaymentMethod({
        name: 'ironixpay',
        label: el(Label, null),
        content: el(Content, null),
        edit: el(Content, null),
        canMakePayment: function () { return true; },
        ariaLabel: title,
        supports: { features: settings.supports || ['products'] },
    });
})();
