// content-scripts/instagram/content.js
// Instagram-specific DOM enforcement (based on IGPlus patterns)

(function() {
    'use strict';

    const DPB = window.DarkPatternBlocker;
    const SITE = 'instagram';

    // ─────────────────────────────────────────────────────────────────────────
    // Selectors (Instagram changes these frequently)
    // ─────────────────────────────────────────────────────────────────────────

    const SELECTORS = {
        // Reels
        reelsTab: 'a[href="/reels/"]',
        reelsSidebar: 'a[href="/reels/"] svg',
        
        // Explore
        exploreTab: 'a[href="/explore/"]',
        explorePage: 'main[role="main"]',
        
        // Stories
        storiesTray: 'div[role="menu"]',
        storyRing: 'canvas',
        
        // Suggested / Recommendations  
        suggestedPosts: 'article',
        suggestedUsers: 'div[data-testid="suggested_users"]',
        
        // Vanity metrics (likes, followers)
        vanityButton: 'button._a9ze',
        followerCount: 'span._ac2a',
        
        // Feed
        homeFeed: 'main[role="main"] > div',
    };

    // ─────────────────────────────────────────────────────────────────────────
    // Feature Implementations
    // ───────────────────────────────────────────-────────────────────────────

    // ─────────────────────────────────────────────────────-───────────────────
    // CSS Files (relative to extension root)
    // ─────────────────────────────────────────────────────-───────────────────

    const CSS  = {
        hideExplore: "/content-scripts/instagram/css/hide-explore.css",
        hideReels: "/content-scripts/instagram/css/hide-reels.css",
        hideStories: "/content-scripts/instagram/css/hide-stories.css",
        hideSuggested: "/content-scripts/instagram/css/hide-suggested.css",
        hideVanityMetrics: "/content-scripts/instagram/css/hide-vanity-metrics.css",
        grayscale: "/content-scripts/instagram/css/greyscale.css",
        hideFeed: "/content-scripts/instagram/css/hide-feed.css",
        hideComments: "/content-scripts/instagram/css/hide-comments.css"
    };

    function hideReels(enabled) {
        const ID = 'ig-hide-reels';
        DPB.clearInterval(ID);
        DPB.toggleCSS(CSS.hideReels, ID, enabled);
        
        if (enabled) {
            function enforce() {
                DPB.hideElements(SELECTORS.reelsTab);
                DPB.hideElements(SELECTORS.reelsSidebar);
                if (window.location.pathname.startsWith('/reels')) {
                    DPB.redirectIf(true, '/');
                }
            }
            enforce();
            DPB.setInterval(ID, enforce, 300);
        } else {
            DPB.showElements(SELECTORS.reelsTab);
            DPB.showElements(SELECTORS.reelsSidebar);
        }
    }

    function hideExplore(enabled) {
        const ID = 'ig-hide-explore';
        DPB.clearInterval(ID);
        DPB.toggleCSS(CSS.hideExplore, ID, enabled);

        if (enabled) {
            function enforce() {
                DPB.hideElements(SELECTORS.exploreTab);
                if (window.location.pathname.startsWith('/explore')) {
                    DPB.redirectIf(true, '/');
                }
            }
            enforce();
            DPB.setInterval(ID, enforce, 300);
        } else {
            DPB.showElements(SELECTORS.exploreTab);
        }
    }

    function hideStories(enabled) {
        const ID = 'ig-hide-stories';
        DPB.clearInterval(ID);
        DPB.toggleCSS(CSS.hideStories, ID, enabled);

        if (enabled) {
            function enforce() {
                DPB.hideElements(SELECTORS.storiesTray);
                if (window.location.pathname.includes('/stories/')) {
                    DPB.redirectIf(true, '/');
                }
            }
            enforce();
            DPB.setInterval(ID, enforce, 300);
        } else {
            DPB.showElements(SELECTORS.storiesTray);
        }
    }

    function hideSuggested(enabled) {
        const ID = 'ig-hide-suggested';
        DPB.clearInterval(ID);
        DPB.toggleCSS(CSS.hideSuggested, ID, enabled);

        if (enabled) {
            function enforce() {
                DPB.hideElements(SELECTORS.suggestedUsers);
            }
            enforce();
            DPB.setInterval(ID, enforce, 500);
        } else {
            DPB.showElements(SELECTORS.suggestedUsers);
        }
    }

    function hideVanityMetrics(enabled) {
        const ID = 'ig-hide-vanity';
        DPB.clearInterval(ID);
        DPB.toggleCSS(CSS.hideVanityMetrics, ID, enabled);

        if (enabled) {
            function enforce() {
                DPB.hideElements(SELECTORS.vanityButton);
                DPB.hideElements(SELECTORS.followerCount);
            }
            enforce();
            DPB.setInterval(ID, enforce, 300);
        } else {
            DPB.showElements(SELECTORS.vanityButton);
            DPB.showElements(SELECTORS.followerCount);
        }
    }

    function hideFeed(enabled) {
        const ID = 'ig-hide-feed';
        DPB.clearInterval(ID);
        DPB.toggleCSS(CSS.hideFeed, ID, enabled);

        if (enabled) {
            function enforce() {
                // Only run on the home page
                if (window.location.pathname !== '/') return;
                DPB.hideElements(SELECTORS.homeFeed);
            }
            enforce();
            DPB.setInterval(ID, enforce, 300);
        } else {
            DPB.showElements(SELECTORS.homeFeed);
        }
    }

    function applyGrayscale(enabled) {
        DPB.toggleCSS(CSS.grayscale, 'ig-grayscale', enabled);
    }

    function unhideAll() {
        DPB.clearAllIntervals();

        // Remove all injected CSS
        Object.keys(CSS).forEach(key => {
            const id = 'ig-' + key.replace(/([A-Z])/g, '-$1').toLowerCase();
            DPB.removeCSS(id);
        });
        
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
        if (!config || !config.enabled) {
            DPB.clearAllIntervals();
            document.documentElement.classList.remove('dpb-grayscale');
            unhideAll();
            return;
        }

        DPB.log('Applying Instagram config:', config);

        hideReels(config.hideReels);
        hideExplore(config.hideExplore);
        hideStories(config.hideStories);
        hideSuggested(config.hideSuggested);
        hideVanityMetrics(config.hideVanityMetrics);
        applyGrayscale(config.grayscale);
        hideFeed(config.hideFeed);
    };

    // ─────────────────────────────────────────────────────────────────────────
    // Initialize
    // ─────────────────────────────────────────────────────────────────────────

    async function init() {
        DPB.log('Initializing Instagram content script');
        
        DPB.hidePageUntilReady();
        
        await DPB.getConfig(SITE);
        DPB.applyConfig();

        // Re-apply on SPA navigation
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
