// background/service-worker.js
// Main background script - coordinates all extension functionality

import { NativeMessenger } from './native-messaging.js';
import { StateManager } from './state.js';
import { RequestBlocker } from './request-blocker.js';

// Initialize components
const state = new StateManager();
const messenger = new NativeMessenger(state);
const blocker = new RequestBlocker(state);

// URL patterns for managed sites (must match manifest content_scripts)
const SITE_PATTERNS = {
    instagram: '*://*.instagram.com/*',
    youtube: '*://*.youtube.com/*',
    twitter: '*://*.twitter.com/*',
    reddit: '*://*.reddit.com/*',
    facebook: '*://*.facebook.com/*',
    linkedin: '*://*.linkedin.com/*',
    netflix: '*://*.netflix.com/*',
};

// Listen for messages from content scripts and popup
chrome.runtime.onMessage.addListener((message, sender, sendResponse) => {
    switch (message.type) {
        case 'GET_STATE':
            state.getState().then(sendResponse);
            return true;

        case 'GET_SITE_CONFIG':
            state.getSiteConfig(message.site).then(sendResponse);
            return true;

        case 'START_SESSION':
            messenger.startSession(message.duration).then(sendResponse);
            return true;

        case 'STOP_SESSION':
            messenger.stopSession().then(sendResponse);
            return true;

        case 'UPDATE_SITE_CONFIG':
            state.updateSiteConfig(message.site, message.config).then(sendResponse);
            return true;

        case 'status':
            state.updateSession(message.active, message.sites, message.time_left).then(sendResponse);
            return true;

        // Forward CRUD commands to daemon via native host
        case 'DAEMON_COMMAND':
            messenger.sendMessage(message.payload);
            sendResponse({ sent: true });
            return false;

        default:
            console.warn('Unknown message type:', message.type);
    }
});

// Handle extension install/update
chrome.runtime.onInstalled.addListener((details) => {
    if (details.reason === 'install') {
        state.initializeDefaults();
        console.log('Dark Pattern Blocker installed');
    } else if (details.reason === 'update') {
        console.log('Dark Pattern Blocker updated to', chrome.runtime.getManifest().version);
    }
});

// Handle tab updates - redirect blocked sites on navigation
chrome.tabs.onUpdated.addListener(async (tabId, changeInfo, tab) => {
    if (changeInfo.status === 'loading' && tab.url) {
        const shouldBlock = await blocker.shouldBlockUrl(tab.url);
        if (shouldBlock) {
            chrome.tabs.update(tabId, {
                url: chrome.runtime.getURL('blocked/blocked.html')
            });
        }
    }
});

// ─────────────────────────────────────────────────────────────────────────────
// Immediate enforcement on state changes (policy pushes, session start/stop)
//
// Two things happen:
// 1. Tabs on blocked sites get redirected immediately (no refresh needed)
// 2. Content scripts on managed sites get their config pushed via
//    chrome.scripting so DOM rules apply instantly
// ─────────────────────────────────────────────────────────────────────────────

state.addListener(async (newState) => {
    console.log('State changed, enforcing:', {
        blockedSites: newState.blockedSites?.length || 0,
        domRules: newState.domRules?.length || 0,
    });

    // ── 1. Redirect tabs on blocked sites ────────────────────────────────────

    const blockedSites = newState.blockedSites || [];
    if (blockedSites.length > 0) {
        const allTabs = await chrome.tabs.query({});
        for (const tab of allTabs) {
            if (!tab.url || tab.url.startsWith('chrome')) continue;
            try {
                const hostname = new URL(tab.url).hostname;
                const isBlocked = blockedSites.some(site =>
                    hostname === site || hostname.endsWith('.' + site)
                );
                if (isBlocked) {
                    chrome.tabs.update(tab.id, {
                        url: chrome.runtime.getURL('blocked/blocked.html')
                    });
                }
            } catch { /* invalid URL */ }
        }
    }

    // ── 2. Push DOM config to already-open managed tabs ──────────────────────
    //
    // chrome.storage.onChanged in base.js handles *most* cases, but:
    //   - Service worker might have been idle when storage changed
    //   - Content script might have loaded before storage was ready
    //   - Belt-and-suspenders: directly call applyConfig on every managed tab
    //
    // We use chrome.scripting.executeScript with world: 'MAIN' to access the
    // DarkPatternBlocker object that the content script created on the page.

    for (const [site, pattern] of Object.entries(SITE_PATTERNS)) {
        const siteConfig = newState.sites?.[site];
        if (!siteConfig) continue;

        try {
            const tabs = await chrome.tabs.query({ url: pattern });
            for (const tab of tabs) {
                if (!tab.url || tab.url.startsWith('chrome-extension://')) continue;

                chrome.scripting.executeScript({
                    target: { tabId: tab.id },
                    world: 'MAIN',
                    args: [siteConfig],
                    func: (config) => {
                        // This runs in the page's JS context where DarkPatternBlocker lives
                        const DPB = window.DarkPatternBlocker;
                        if (DPB) {
                            console.log('[DarkPatternBlocker] Policy push → applying config:', config);
                            DPB.state.config = config;
                            DPB.applyConfig();
                        }
                    },
                }).catch((err) => {
                    // Tab not ready, special page, etc. — that's fine
                    console.debug(`Could not inject into tab ${tab.id}:`, err.message);
                });
            }
        } catch { /* no tabs matched */ }
    }
});

console.log('Dark Pattern Blocker service worker started');