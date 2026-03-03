// content-scripts/youtube/content.js
// YouTube-specific DOM enforcement

(function() {
    'use strict';

    const DPB = window.DarkPatternBlocker;
    const SITE = 'youtube';

    // ─────────────────────────────────────────────────────────────────────────
    // Selectors (update these when YouTube changes their DOM)
    // ─────────────────────────────────────────────────────────────────────────

    const SELECTORS = {
        // Shorts
        shortsShelf: 'ytd-rich-shelf-renderer[is-shorts], ytd-reel-shelf-renderer',
        shortsTab: 'ytd-mini-guide-entry-renderer[aria-label="Shorts"]',
        shortsSidebar: 'ytd-guide-entry-renderer a[title="Shorts"]',
        shortsNavItem: 'ytd-guide-entry-renderer:has(a[title="Shorts"])',
        
        // Home feed
        homeFeed: 'ytd-rich-grid-renderer, ytd-two-column-browse-results-renderer',
        chipBar: 'ytd-feed-filter-chip-bar-renderer, #chips-wrapper',
        
        // Recommendations (sidebar on watch page)
        recommendations: '#secondary, #related, ytd-watch-next-secondary-results-renderer',
        
        // Autoplay
        autoplayToggle: '.ytp-autonav-toggle-button',
        
        // Comments
        comments: '#comments, ytd-comments',
        
        // End cards / annotations
        endCards: '.ytp-ce-element, .ytp-ce-covering-overlay',
        endScreen: '.ytp-endscreen-content, .ytp-endscreen-previous, .ytp-endscreen-next',
        
        // Cards that pop up during video
        infoCards: '.ytp-cards-teaser, .ytp-cards-button',
    };

    // ─────────────────────────────────────────────────────────────────────────
    // CSS Files (relative to extension root)
    // ─────────────────────────────────────────────────────────────────────────

    const CSS = {
        hideShorts: '/content-scripts/youtube/css/hide-shorts.css',
        hideHomeFeed: '/content-scripts/youtube/css/hide-home-feed.css',
        hideRecommendations: '/content-scripts/youtube/css/hide-recommendations.css',
        hideComments: '/content-scripts/youtube/css/hide-comments.css',
        hideEndCards: '/content-scripts/youtube/css/hide-end-cards.css',
        grayscale: '/content-scripts/youtube/css/grayscale.css',
    };

    // ─────────────────────────────────────────────────────────────────────────
    // Feature Implementations
    // ─────────────────────────────────────────────────────────────────────────

    function hideShorts(enabled) {
        const ID = 'yt-hide-shorts';
        DPB.clearInterval(ID);
        
        // CSS handles most of it
        DPB.toggleCSS(CSS.hideShorts, ID, enabled);
        
        if (enabled) {
            // JS needed for redirect and dynamic elements
            function enforce() {
                // Hide any shorts elements that CSS missed
                DPB.hideElements(SELECTORS.shortsShelf);
                DPB.hideElements(SELECTORS.shortsTab);
                DPB.hideElements(SELECTORS.shortsSidebar);
                DPB.hideElements(SELECTORS.shortsNavItem);
                
                // Redirect away from /shorts pages
                if (window.location.pathname.startsWith('/shorts')) {
                    DPB.redirectIf(true, '/subscriptions');
                }
            }
            enforce();
            DPB.setInterval(ID, enforce, 500);
        } else {
            // Restore shorts elements
            DPB.showElements(SELECTORS.shortsShelf);
            DPB.showElements(SELECTORS.shortsTab);
            DPB.showElements(SELECTORS.shortsSidebar);
            DPB.showElements(SELECTORS.shortsNavItem);
        }
    }

    function hideHomeFeed(enabled) {
        const ID = 'yt-hide-home';
        DPB.clearInterval(ID);
        
        DPB.toggleCSS(CSS.hideHomeFeed, ID, enabled);
        
        if (enabled) {
            function enforce() {
                // Only on home page
                if (window.location.pathname !== '/') return;
                
                DPB.hideElements(SELECTORS.homeFeed);
                DPB.hideElements(SELECTORS.chipBar);
            }
            enforce();
            DPB.setInterval(ID, enforce, 500);
        } else {
            // Restore home feed elements
            DPB.showElements(SELECTORS.homeFeed);
            DPB.showElements(SELECTORS.chipBar);
        }
    }

    function hideRecommendations(enabled) {
        const ID = 'yt-hide-recs';
        DPB.clearInterval(ID);
        
        DPB.toggleCSS(CSS.hideRecommendations, ID, enabled);
        
        if (enabled) {
            function enforce() {
                // Only on watch pages
                if (!window.location.pathname.startsWith('/watch')) return;
                DPB.hideElements(SELECTORS.recommendations);
            }
            enforce();
            DPB.setInterval(ID, enforce, 500);
        } else {
            DPB.showElements(SELECTORS.recommendations);
        }
    }

    function disableAutoplay(enabled) {
        const ID = 'yt-no-autoplay';
        DPB.clearInterval(ID);
        
        if (!enabled) return;

        function enforce() {
            const toggle = document.querySelector(SELECTORS.autoplayToggle);
            if (toggle && toggle.getAttribute('aria-checked') === 'true') {
                toggle.click();
                DPB.log('Disabled autoplay');
            }
        }
        
        enforce();
        DPB.setInterval(ID, enforce, 1000);
    }

    function hideComments(enabled) {
        const ID = 'yt-hide-comments';
        DPB.clearInterval(ID);
        
        DPB.toggleCSS(CSS.hideComments, ID, enabled);
        
        if (enabled) {
            function enforce() {
                DPB.hideElements(SELECTORS.comments);
            }
            enforce();
            DPB.setInterval(ID, enforce, 500);
        } else {
            DPB.showElements(SELECTORS.comments);
        }
    }

    function hideEndCards(enabled) {
        const ID = 'yt-hide-endcards';
        DPB.clearInterval(ID);
        
        DPB.toggleCSS(CSS.hideEndCards, ID, enabled);
        
        if (enabled) {
            function enforce() {
                DPB.hideElements(SELECTORS.endCards);
                DPB.hideElements(SELECTORS.endScreen);
                DPB.hideElements(SELECTORS.infoCards);
            }
            enforce();
            DPB.setInterval(ID, enforce, 500);
        } else {
            DPB.showElements(SELECTORS.endCards);
            DPB.showElements(SELECTORS.endScreen);
            DPB.showElements(SELECTORS.infoCards);
        }
    }

    function applyGrayscale(enabled) {
        DPB.toggleCSS(CSS.grayscale, 'yt-grayscale', enabled);
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Cleanup
    // ─────────────────────────────────────────────────────────────────────────

    function cleanup() {
        DPB.clearAllIntervals();
        
        // Remove all injected CSS
        Object.keys(CSS).forEach(key => {
            DPB.removeCSS('yt-' + key.replace(/([A-Z])/g, '-$1').toLowerCase());
        });
        DPB.removeCSS('yt-hide-shorts');
        DPB.removeCSS('yt-hide-home');
        DPB.removeCSS('yt-hide-recs');
        DPB.removeCSS('yt-hide-comments');
        DPB.removeCSS('yt-hide-endcards');
        DPB.removeCSS('yt-grayscale');
        
        // Restore all hidden elements
        Object.values(SELECTORS).forEach(selector => {
            DPB.showElements(selector);
        });
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Apply Config
    // ─────────────────────────────────────────────────────────────────────────

    DPB.applyConfig = function() {
        const config = DPB.state.config;
        
        // If disabled, clean up everything
        if (!config || !config.enabled) {
            DPB.log('YouTube enforcement disabled, cleaning up');
            cleanup();
            return;
        }

        DPB.log('Applying YouTube config:', config);

        // Apply each feature based on config
        hideShorts(config.hideShorts ?? false);
        hideHomeFeed(config.hideHomeFeed ?? false);
        hideRecommendations(config.hideRecommendations ?? false);
        disableAutoplay(config.disableAutoplay ?? false);
        hideComments(config.hideComments ?? false);
        hideEndCards(config.hideEndCards ?? false);
        applyGrayscale(config.grayscale ?? false);
    };

    // ─────────────────────────────────────────────────────────────────────────
    // SPA Navigation Detection
    // ─────────────────────────────────────────────────────────────────────────

    function watchNavigation() {
        let lastUrl = location.href;
        let lastPathname = location.pathname;
        
        // YouTube uses History API for navigation
        const observer = DPB.observe(() => {
            if (location.href !== lastUrl) {
                const oldPathname = lastPathname;
                lastUrl = location.href;
                lastPathname = location.pathname;
                
                DPB.log(`Navigation: ${oldPathname} → ${lastPathname}`);
                
                // Re-apply config on navigation
                // Small delay to let YouTube render the new page
                setTimeout(() => DPB.applyConfig(), 100);
            }
        });
        
        // Also listen for yt-navigate-finish (YouTube's custom event)
        document.addEventListener('yt-navigate-finish', () => {
            DPB.log('yt-navigate-finish event');
            setTimeout(() => DPB.applyConfig(), 100);
        });
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Initialize
    // ─────────────────────────────────────────────────────────────────────────

    async function init() {
        DPB.log('Initializing YouTube content script');
        
        // Get config and apply
        await DPB.getConfig(SITE);
        DPB.applyConfig();
        
        // Watch for SPA navigation
        watchNavigation();
    }

    // Start when DOM is ready or immediately if already loaded
    if (document.readyState === 'loading') {
        document.addEventListener('DOMContentLoaded', init);
    } else {
        init();
    }

})();