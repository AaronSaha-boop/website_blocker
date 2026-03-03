// content-scripts/facebook/content.js
// Facebook-specific DOM enforcement (based on Fluff Busting Purity patterns)

(function() {
    'use strict';

    const DPB = window.DarkPatternBlocker;
    const SITE = 'facebook';

    // ─────────────────────────────────────────────────────────────────────────
    // Selectors (new Facebook / Meta design)
    // ─────────────────────────────────────────────────────────────────────────

    const SELECTORS = {
        // Reels
        reelsNav: 'div[role="banner"] a[href="/reel/"], div[role="banner"] a[href="/watch/"]',
        reelsSidebar: 'div[role="navigation"] a[href*="/reel"]',
        reelsPosts: 'div[aria-label*="Reel"], div[aria-label*="reel"]',

        // Stories
        storiesTray: 'div[aria-label="Stories"], div[data-pagelet="Stories"]',
        createStory: 'div[aria-label="Create a story"], div[aria-label*="Your story"], div[aria-label*="Your Story"]',

        // Suggested / PYMK
        pymk: 'div[aria-label="People You May Know"], div[aria-label="People you may know"]',
        suggestedPosts: 'div[aria-label*="Suggested for you"], div[aria-label*="suggested for you"]',
        suggestedGroups: 'div[aria-label*="Suggested groups"]',

        // Marketplace
        marketplaceNav: 'div[role="banner"] a[href="/marketplace/"], div[role="banner"] a[aria-label="Marketplace"]',
        marketplaceSidebar: 'div[role="navigation"] a[href*="/marketplace"]',

        // Watch
        watchNav: 'div[role="banner"] a[href="/watch/"], div[role="banner"] a[aria-label="Watch"]',
        watchSidebar: 'div[role="navigation"] a[href*="/watch"]',

        // Sponsored — detected by JS scanning post text for "Sponsored" label
        sponsoredLabel: 'a[aria-label="Advertiser"]',
        adSlots: 'div[data-ad], div[data-ad^="{"]',
        sidebarAds: '#pagelet_ego_pane, #pagelet_ads, #pagelet_side_ads, #sidebar_ads',

        // Gaming
        gamingNav: 'div[role="banner"] a[href*="/gaming/"], div[role="banner"] a[aria-label="Gaming"]',
        gamingSidebar: 'div[role="navigation"] a[href*="/gaming/"]',
        gameRequests: 'div[aria-label*="Game requests"], div[data-gamesrankedimp]',

        // Top nav buttons
        notificationsBtn: 'div[role="banner"] div[aria-label^="Notifications"], div[role="banner"] a[aria-label^="Notifications"]',
        messengerBtn: 'div[role="banner"] div[aria-label="Messenger"], div[role="banner"] a[aria-label="Messenger"]',
        groupsBtn: 'div[role="banner"] a[href="/groups/"], div[role="banner"] a[aria-label="Groups"]',

        // Meta AI
        metaAI: 'div[role="navigation"] a[href*="/ai"], div[aria-label*="Meta AI"]',

        // Newsfeed
        newsfeed: 'div[role="feed"]',
    };

    // ─────────────────────────────────────────────────────────────────────────
    // CSS Files
    // ─────────────────────────────────────────────────────────────────────────

    const CSS = {
        hideReels:          '/content-scripts/facebook/css/hide-reels.css',
        hideStories:        '/content-scripts/facebook/css/hide-stories.css',
        hideSuggested:      '/content-scripts/facebook/css/hide-suggested.css',
        hideMarketplace:    '/content-scripts/facebook/css/hide-marketplace.css',
        hideWatch:          '/content-scripts/facebook/css/hide-watch.css',
        hideSponsored:      '/content-scripts/facebook/css/hide-sponsored.css',
        hideGaming:         '/content-scripts/facebook/css/hide-gaming.css',
        hideNotifications:  '/content-scripts/facebook/css/hide-notifications.css',
        hideMessengerBtn:   '/content-scripts/facebook/css/hide-messenger-btn.css',
        hideGroupsBtn:      '/content-scripts/facebook/css/hide-groups-btn.css',
        hideMetaAI:         '/content-scripts/facebook/css/hide-meta-ai.css',
        hideNewsfeed:       '/content-scripts/facebook/css/hide-newsfeed.css',
    };

    // ─────────────────────────────────────────────────────────────────────────
    // Sponsored post detection (JS-based, like FBP)
    //
    // Facebook doesn't mark sponsored posts with a simple attribute.
    // They use a "Sponsored" text label inside the post header that's
    // rendered via multiple nested spans to evade ad blockers.
    // We scan post headers for this text and hide the ancestor post.
    // ─────────────────────────────────────────────────────────────────────────

    function findAndHideSponsoredPosts() {
        // Facebook renders "Sponsored" across multiple spans
        // Look for the link that contains the sponsored label
        const links = document.querySelectorAll('a[href*="about/ads"], a[aria-label="Advertiser"], a[href*="ads/about"]');
        links.forEach(link => {
            // Walk up to find the post container (role="article" ancestor)
            let post = link.closest('div[role="article"]');
            if (!post) {
                // Try a few more levels up for different FB layouts
                let el = link;
                for (let i = 0; i < 15; i++) {
                    el = el.parentElement;
                    if (!el) break;
                    if (el.dataset?.dpbHidden) break;
                    // Look for the post boundary
                    if (el.getAttribute('data-pagelet')?.startsWith('FeedUnit')) {
                        post = el;
                        break;
                    }
                }
            }
            if (post && !post.dataset.dpbHidden) {
                post.dataset.dpbHidden = 'true';
                post.style.display = 'none';
            }
        });
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Feature Implementations
    // ─────────────────────────────────────────────────────────────────────────

    function hideReels(enabled) {
        const ID = 'fb-hide-reels';
        DPB.clearInterval(ID);
        DPB.toggleCSS(CSS.hideReels, ID, enabled);

        if (enabled) {
            function enforce() {
                DPB.hideElements(SELECTORS.reelsNav);
                DPB.hideElements(SELECTORS.reelsSidebar);
                DPB.hideElements(SELECTORS.reelsPosts);
                if (window.location.pathname.startsWith('/reel')) {
                    DPB.redirectIf(true, '/');
                }
            }
            enforce();
            DPB.setInterval(ID, enforce, 500);
        } else {
            DPB.showElements(SELECTORS.reelsNav);
            DPB.showElements(SELECTORS.reelsSidebar);
            DPB.showElements(SELECTORS.reelsPosts);
        }
    }

    function hideStories(enabled) {
        const ID = 'fb-hide-stories';
        DPB.clearInterval(ID);
        DPB.toggleCSS(CSS.hideStories, ID, enabled);

        if (enabled) {
            function enforce() {
                DPB.hideElements(SELECTORS.storiesTray);
                DPB.hideElements(SELECTORS.createStory);
            }
            enforce();
            DPB.setInterval(ID, enforce, 500);
        } else {
            DPB.showElements(SELECTORS.storiesTray);
            DPB.showElements(SELECTORS.createStory);
        }
    }

    function hideSuggested(enabled) {
        const ID = 'fb-hide-suggested';
        DPB.clearInterval(ID);
        DPB.toggleCSS(CSS.hideSuggested, ID, enabled);

        if (enabled) {
            function enforce() {
                DPB.hideElements(SELECTORS.pymk);
                DPB.hideElements(SELECTORS.suggestedPosts);
                DPB.hideElements(SELECTORS.suggestedGroups);
            }
            enforce();
            DPB.setInterval(ID, enforce, 500);
        } else {
            DPB.showElements(SELECTORS.pymk);
            DPB.showElements(SELECTORS.suggestedPosts);
            DPB.showElements(SELECTORS.suggestedGroups);
        }
    }

    function hideMarketplace(enabled) {
        const ID = 'fb-hide-marketplace';
        DPB.clearInterval(ID);
        DPB.toggleCSS(CSS.hideMarketplace, ID, enabled);

        if (enabled) {
            function enforce() {
                DPB.hideElements(SELECTORS.marketplaceNav);
                DPB.hideElements(SELECTORS.marketplaceSidebar);
                if (window.location.pathname.startsWith('/marketplace')) {
                    DPB.redirectIf(true, '/');
                }
            }
            enforce();
            DPB.setInterval(ID, enforce, 500);
        } else {
            DPB.showElements(SELECTORS.marketplaceNav);
            DPB.showElements(SELECTORS.marketplaceSidebar);
        }
    }

    function hideWatch(enabled) {
        const ID = 'fb-hide-watch';
        DPB.clearInterval(ID);
        DPB.toggleCSS(CSS.hideWatch, ID, enabled);

        if (enabled) {
            function enforce() {
                DPB.hideElements(SELECTORS.watchNav);
                DPB.hideElements(SELECTORS.watchSidebar);
                if (window.location.pathname.startsWith('/watch')) {
                    DPB.redirectIf(true, '/');
                }
            }
            enforce();
            DPB.setInterval(ID, enforce, 500);
        } else {
            DPB.showElements(SELECTORS.watchNav);
            DPB.showElements(SELECTORS.watchSidebar);
        }
    }

    function hideSponsored(enabled) {
        const ID = 'fb-hide-sponsored';
        DPB.clearInterval(ID);
        DPB.toggleCSS(CSS.hideSponsored, ID, enabled);

        if (enabled) {
            function enforce() {
                DPB.hideElements(SELECTORS.sponsoredLabel);
                DPB.hideElements(SELECTORS.adSlots);
                DPB.hideElements(SELECTORS.sidebarAds);
                findAndHideSponsoredPosts();
            }
            enforce();
            DPB.setInterval(ID, enforce, 500);
        } else {
            DPB.showElements(SELECTORS.sponsoredLabel);
            DPB.showElements(SELECTORS.adSlots);
            DPB.showElements(SELECTORS.sidebarAds);
        }
    }

    function hideGaming(enabled) {
        const ID = 'fb-hide-gaming';
        DPB.clearInterval(ID);
        DPB.toggleCSS(CSS.hideGaming, ID, enabled);

        if (enabled) {
            function enforce() {
                DPB.hideElements(SELECTORS.gamingNav);
                DPB.hideElements(SELECTORS.gamingSidebar);
                DPB.hideElements(SELECTORS.gameRequests);
            }
            enforce();
            DPB.setInterval(ID, enforce, 500);
        } else {
            DPB.showElements(SELECTORS.gamingNav);
            DPB.showElements(SELECTORS.gamingSidebar);
            DPB.showElements(SELECTORS.gameRequests);
        }
    }

    function hideNotifications(enabled) {
        const ID = 'fb-hide-notifications';
        DPB.clearInterval(ID);
        DPB.toggleCSS(CSS.hideNotifications, ID, enabled);

        if (enabled) {
            function enforce() { DPB.hideElements(SELECTORS.notificationsBtn); }
            enforce();
            DPB.setInterval(ID, enforce, 500);
        } else {
            DPB.showElements(SELECTORS.notificationsBtn);
        }
    }

    function hideMessengerBtn(enabled) {
        const ID = 'fb-hide-messenger-btn';
        DPB.clearInterval(ID);
        DPB.toggleCSS(CSS.hideMessengerBtn, ID, enabled);

        if (enabled) {
            function enforce() { DPB.hideElements(SELECTORS.messengerBtn); }
            enforce();
            DPB.setInterval(ID, enforce, 500);
        } else {
            DPB.showElements(SELECTORS.messengerBtn);
        }
    }

    function hideGroupsBtn(enabled) {
        const ID = 'fb-hide-groups-btn';
        DPB.clearInterval(ID);
        DPB.toggleCSS(CSS.hideGroupsBtn, ID, enabled);

        if (enabled) {
            function enforce() { DPB.hideElements(SELECTORS.groupsBtn); }
            enforce();
            DPB.setInterval(ID, enforce, 500);
        } else {
            DPB.showElements(SELECTORS.groupsBtn);
        }
    }

    function hideMetaAI(enabled) {
        const ID = 'fb-hide-meta-ai';
        DPB.clearInterval(ID);
        DPB.toggleCSS(CSS.hideMetaAI, ID, enabled);

        if (enabled) {
            function enforce() { DPB.hideElements(SELECTORS.metaAI); }
            enforce();
            DPB.setInterval(ID, enforce, 500);
        } else {
            DPB.showElements(SELECTORS.metaAI);
        }
    }

    function hideNewsfeed(enabled) {
        const ID = 'fb-hide-newsfeed';
        DPB.clearInterval(ID);
        DPB.toggleCSS(CSS.hideNewsfeed, ID, enabled);

        if (enabled) {
            function enforce() {
                if (window.location.pathname === '/') {
                    DPB.hideElements(SELECTORS.newsfeed);
                }
            }
            enforce();
            DPB.setInterval(ID, enforce, 500);
        } else {
            DPB.showElements(SELECTORS.newsfeed);
        }
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Cleanup
    // ─────────────────────────────────────────────────────────────────────────

    function cleanup() {
        DPB.clearAllIntervals();

        Object.keys(CSS).forEach(key => {
            const idMap = {
                hideReels: 'fb-hide-reels',
                hideStories: 'fb-hide-stories',
                hideSuggested: 'fb-hide-suggested',
                hideMarketplace: 'fb-hide-marketplace',
                hideWatch: 'fb-hide-watch',
                hideSponsored: 'fb-hide-sponsored',
                hideGaming: 'fb-hide-gaming',
                hideNotifications: 'fb-hide-notifications',
                hideMessengerBtn: 'fb-hide-messenger-btn',
                hideGroupsBtn: 'fb-hide-groups-btn',
                hideMetaAI: 'fb-hide-meta-ai',
                hideNewsfeed: 'fb-hide-newsfeed',
            };
            if (idMap[key]) DPB.removeCSS(idMap[key]);
        });

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
            DPB.log('Facebook enforcement disabled, cleaning up');
            cleanup();
            return;
        }

        DPB.log('Applying Facebook config:', config);

        hideReels(config.hideReels ?? false);
        hideStories(config.hideStories ?? false);
        hideSuggested(config.hideSuggested ?? false);
        hideMarketplace(config.hideMarketplace ?? false);
        hideWatch(config.hideWatch ?? false);
        hideSponsored(config.hideSponsored ?? false);
        hideGaming(config.hideGaming ?? false);
        hideNotifications(config.hideNotifications ?? false);
        hideMessengerBtn(config.hideMessengerBtn ?? false);
        hideGroupsBtn(config.hideGroupsBtn ?? false);
        hideMetaAI(config.hideMetaAI ?? false);
        hideNewsfeed(config.hideNewsfeed ?? false);
    };

    // ─────────────────────────────────────────────────────────────────────────
    // SPA Navigation Detection
    // ─────────────────────────────────────────────────────────────────────────

    function watchNavigation() {
        let lastUrl = location.href;

        DPB.observe(() => {
            if (location.href !== lastUrl) {
                lastUrl = location.href;
                DPB.log('Navigation:', lastUrl);
                setTimeout(() => DPB.applyConfig(), 100);
            }
        });
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Initialize
    // ─────────────────────────────────────────────────────────────────────────

    async function init() {
        DPB.log('Initializing Facebook content script');
        DPB.hidePageUntilReady();
        await DPB.getConfig(SITE);
        DPB.applyConfig();
        watchNavigation();
    }

    if (document.readyState === 'loading') {
        document.addEventListener('DOMContentLoaded', init);
    } else {
        init();
    }

})();
