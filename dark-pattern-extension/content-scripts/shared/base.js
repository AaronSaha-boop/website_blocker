// content-scripts/shared/base.js
// Shared utilities for all content scripts

window.DarkPatternBlocker = window.DarkPatternBlocker || {};

(function(DPB) {
    'use strict';

    // ─────────────────────────────────────────────────────────────────────────
    // State Management
    // ─────────────────────────────────────────────────────────────────────────

    DPB.state = {
        config: null,
        intervals: {},
        observers: []
    };

    DPB.getConfig = async function(site) {
        return new Promise((resolve) => {
            chrome.storage.local.get('state', (result) => {
                const config = result.state?.sites?.[site] || null;
                DPB.state.config = config;
                resolve(config);
            });
        });
    };

    // Listen for config changes from storage (policy pushes, user toggles).
    // This fires when state.js calls chrome.storage.local.set().
    // Works as the primary reactive path for config updates.
    chrome.storage.local.onChanged.addListener((changes) => {
        if (changes.state) {
            const site = DPB.getCurrentSite();
            if (site) {
                const newConfig = changes.state.newValue?.sites?.[site];
                if (newConfig) {
                    DPB.log('Storage changed for', site, '→ re-applying config');
                    DPB.state.config = newConfig;
                    DPB.applyConfig();
                }
            }
        }
    });

    // ─────────────────────────────────────────────────────────────────────────
    // Site Detection
    // ─────────────────────────────────────────────────────────────────────────

    DPB.getCurrentSite = function() {
        const hostname = window.location.hostname;
        
        if (hostname.includes('instagram.com')) return 'instagram';
        if (hostname.includes('youtube.com')) return 'youtube';
        if (hostname.includes('twitter.com') || hostname.includes('x.com')) return 'twitter';
        if (hostname.includes('reddit.com')) return 'reddit';
        if (hostname.includes('facebook.com')) return 'facebook';
        if (hostname.includes('linkedin.com')) return 'linkedin';
        if (hostname.includes('netflix.com')) return 'netflix';
        
        return null;
    };

    // ─────────────────────────────────────────────────────────────────────────
    // CSS Injection
    // ─────────────────────────────────────────────────────────────────────────

    DPB.injectCSS = function(path, id) {
        if (document.getElementById(id)) return;

        return fetch(chrome.runtime.getURL(path))
            .then(response => {
                if (!response.ok) throw new Error(`Failed to load CSS: ${response.statusText}`);
                return response.text();
            })
            .then(css => {
                const style = document.createElement('style');
                style.id = id;
                style.textContent = css;
                (document.head || document.documentElement).appendChild(style);
            })
            .catch(err => console.error("CSS Injection Error:", err));
    };


    DPB.removeCSS = function(id) {
        const element = document.getElementById(id);
        if (element) {
            element.remove();
        }
    };

    DPB.toggleCSS = function(path, id, enabled) {
        if (enabled) {
            DPB.injectCSS(path, id);
        } else {
            DPB.removeCSS(id);
        }
    };

    // ─────────────────────────────────────────────────────────────────────────
    // DOM Manipulation
    // ─────────────────────────────────────────────────────────────────────────

    // Hide by setting display: none
    DPB.hideElements = function(selector) {
        document.querySelectorAll(selector).forEach(el => {
            el.dataset.dpbHidden = 'true';  // Mark as hidden by us
            el.style.display = 'none';
        });
    };

    // Restore elements we hid
    DPB.showElements = function(selector) {
        document.querySelectorAll(selector).forEach(el => {
            if (el.dataset.dpbHidden === 'true') {
                el.style.display = '';  // Remove our override
                delete el.dataset.dpbHidden;
            }
        });
    };

    DPB.removeElements = function(selector) {
        document.querySelectorAll(selector).forEach(el => {
            el.remove();
        });
    };

    DPB.addClassToElements = function(selector, className) {
        document.querySelectorAll(selector).forEach(el => {
            el.classList.add(className);
        });
    };

    // ─────────────────────────────────────────────────────────────────────────
    // Interval Management (IGPlus pattern)
    // ─────────────────────────────────────────────────────────────────────────

    DPB.setInterval = function(name, callback, ms) {
        DPB.clearInterval(name);
        DPB.state.intervals[name] = setInterval(callback, ms);
    };

    DPB.clearInterval = function(name) {
        if (DPB.state.intervals[name]) {
            clearInterval(DPB.state.intervals[name]);
            delete DPB.state.intervals[name];
        }
    };

    DPB.clearAllIntervals = function() {
        Object.keys(DPB.state.intervals).forEach(name => {
            DPB.clearInterval(name);
        });
    };

    // ─────────────────────────────────────────────────────────────────────────
    // Mutation Observer (more efficient than intervals for some cases)
    // ─────────────────────────────────────────────────────────────────────────

    DPB.observe = function(callback, options = {}) {
        const observer = new MutationObserver((mutations) => {
            callback(mutations);
        });

        const target = options.target || document.body;
        const config = {
            childList: true,
            subtree: true,
            ...options
        };

        // Wait for body to exist
        if (target === document.body && !document.body) {
            document.addEventListener('DOMContentLoaded', () => {
                observer.observe(document.body, config);
            });
        } else {
            observer.observe(target, config);
        }

        DPB.state.observers.push(observer);
        return observer;
    };

    DPB.disconnectAllObservers = function() {
        DPB.state.observers.forEach(obs => obs.disconnect());
        DPB.state.observers = [];
    };

    // ─────────────────────────────────────────────────────────────────────────
    // URL Redirect
    // ─────────────────────────────────────────────────────────────────────────

    DPB.redirectIf = function(condition, destination) {
        if (condition && window.location.href !== destination) {
            if (window.location.assign) {
                window.location.assign(destination);
            } else {
                window.location.href = destination;
            }
            return true;
        }
        return false;
    };

    DPB.redirectIfPathIncludes = function(path, destination) {
        return DPB.redirectIf(window.location.href.includes(path), destination);
    };

    // ─────────────────────────────────────────────────────────────────────────
    // Initialization
    // ─────────────────────────────────────────────────────────────────────────

    // Placeholder - each site's content.js should override this
    DPB.applyConfig = function() {
        console.warn('applyConfig not implemented for this site');
    };

    // Hide page briefly to prevent flash of unwanted content
    DPB.hidePageUntilReady = function() {
    // DISABLED - causes white screen when script loads late
    // The flash of content is acceptable
    return;
    };

    // Log for debugging
    DPB.log = function(...args) {
        console.log('[DarkPatternBlocker]', ...args);
    };

})(window.DarkPatternBlocker);