// content-scripts/twitter/content.js
// Twitter/X-specific DOM enforcement

(function() {
    'use strict';

    const DPB = window.DarkPatternBlocker;
    const SITE = 'twitter';

    // ─────────────────────────────────────────────────────────────────────────
    // Selectors (X changes these frequently)
    // ─────────────────────────────────────────────────────────────────────────

    const SELECTORS = {
        // "For You" vs "Following" tabs
        forYouTab: 'a[href="/home"][role="tab"]',
        followingTab: 'a[href="/home/following"]',
        
        // Trends / What's happening
        trends: '[data-testid="trend"]',
        trendsSection: 'section[aria-labelledby*="accessible-list"]',
        
        // Who to follow
        whoToFollow: '[data-testid="UserCell"]',
        
        // Promoted tweets
        promoted: 'article:has([data-testid="placementTracking"])',
        
        // Explore page
        explorePage: 'a[href="/explore"]',
    };

    // ─────────────────────────────────────────────────────────────────────────
    // Feature Implementations
    // ─────────────────────────────────────────────────────────────────────────

    function hideForYou(enabled) {
        DPB.clearInterval('hideForYou');
        
        if (!enabled) return;

        function redirect() {
            // Redirect from For You to Following
            if (window.location.pathname === '/home' && 
                !window.location.pathname.includes('following')) {
                // Check if we're on For You tab
                const forYouTab = document.querySelector('[role="tab"][aria-selected="true"]');
                if (forYouTab && forYouTab.textContent.includes('For you')) {
                    DPB.redirectIf(true, '/home/following');
                }
            }
        }

        redirect();
        DPB.setInterval('hideForYou', redirect, 500);
    }

    function hideTrends(enabled) {
        DPB.clearInterval('hideTrends');
        
        if (!enabled) return;

        function hide() {
            // Hide "What's happening" section
            document.querySelectorAll('[data-testid="trend"]').forEach(el => {
                const section = el.closest('section');
                if (section) section.style.display = 'none';
            });
        }

        hide();
        DPB.setInterval('hideTrends', hide, 500);
    }

    function hideWhoToFollow(enabled) {
        DPB.clearInterval('hideWhoToFollow');
        
        if (!enabled) return;

        function hide() {
            document.querySelectorAll('aside').forEach(aside => {
                if (aside.textContent.includes('Who to follow')) {
                    aside.style.display = 'none';
                }
            });
        }

        hide();
        DPB.setInterval('hideWhoToFollow', hide, 500);
    }

    function hidePromoted(enabled) {
        DPB.clearInterval('hidePromoted');
        
        if (!enabled) return;

        function hide() {
            document.querySelectorAll('article').forEach(article => {
                if (article.textContent.includes('Promoted') || 
                    article.textContent.includes('Ad')) {
                    article.style.display = 'none';
                }
            });
        }

        hide();
        DPB.setInterval('hidePromoted', hide, 500);
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Apply Config
    // ─────────────────────────────────────────────────────────────────────────

    DPB.applyConfig = function() {
        const config = DPB.state.config;
        if (!config || !config.enabled) {
            DPB.clearAllIntervals();
            return;
        }

        DPB.log('Applying Twitter config:', config);

        hideForYou(config.hideForYou);
        hideTrends(config.hideTrends);
        hideWhoToFollow(config.hideWhoToFollow);
        hidePromoted(config.hidePromoted);
    };

    // ─────────────────────────────────────────────────────────────────────────
    // Initialize
    // ─────────────────────────────────────────────────────────────────────────

    async function init() {
        DPB.log('Initializing Twitter content script');
        
        DPB.hidePageUntilReady();
        
        await DPB.getConfig(SITE);
        DPB.applyConfig();

        let lastUrl = location.href;
        DPB.observe(() => {
            if (location.href !== lastUrl) {
                lastUrl = location.href;
                DPB.applyConfig();
            }
        });
    }

    init();

})();
