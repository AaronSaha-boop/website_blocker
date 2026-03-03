// content-scripts/reddit/content.js
// Reddit-specific DOM enforcement (based on unhook-for-reddit patterns)

(function() {
    'use strict';

    const DPB = window.DarkPatternBlocker;
    const SITE = 'reddit';

    // ─────────────────────────────────────────────────────────────────────────
    // Selectors (new Reddit / shreddit)
    // ─────────────────────────────────────────────────────────────────────────

    const SELECTORS = {
        // Main feeds
        homeFeed: 'shreddit-feed',
        gallery: 'shreddit-gallery-carousel',
        communityHighlights: 'community-highlight-carousel',

        // Left sidebar
        leftSidebar: '#left-sidebar',
        popular: '#popular-posts',
        explore: '#explore-communities',
        all: '#all-posts',
        games: 'games-section-badge-controller',
        customFeeds: '#multireddits_section',
        recentSubreddits: 'reddit-recent-pages',
        communities: '#communities_section',

        // Right sidebar
        rightSidebar: '#right-sidebar-contents',
        recentPosts: 'recent-posts',
        subredditInfo: '#subreddit-right-rail__partial',
        popularCommunities: '[aria-label="Popular Communities"]',

        // Comments & votes
        comments: 'shreddit-comment',
        upvotes: '[slot="vote-button"]',
        upvoteCount: 'faceplate-number',

        // Search
        search: 'reddit-search-large',
        trending: '#reddit-trending-searches-partial-container',
        trendingLabel: 'div.ml-md.mt-sm.mb-2xs.text-neutral-content-weak.flex.items-center',
        trendingContainer: 'div.w-full.border-solid.border-b-sm.border-t-0.border-r-0.border-l-0.border-neutral-border',

        // Notifications
        notifications: '#notifications-inbox-button',
    };

    // ─────────────────────────────────────────────────────────────────────────
    // CSS Files
    // ─────────────────────────────────────────────────────────────────────────

    const CSS = {
        hideHomeFeed:           '/content-scripts/reddit/css/hide-home-feed.css',
        hideGallery:            '/content-scripts/reddit/css/hide-gallery.css',
        hideSubredditFeed:      '/content-scripts/reddit/css/hide-subreddit-feed.css',
        hideCommunityHighlights:'/content-scripts/reddit/css/hide-community-highlights.css',
        hideLeftSidebar:        '/content-scripts/reddit/css/hide-left-sidebar.css',
        hidePopular:            '/content-scripts/reddit/css/hide-popular.css',
        hideExplore:            '/content-scripts/reddit/css/hide-explore.css',
        hideAll:                '/content-scripts/reddit/css/hide-all.css',
        hideGames:              '/content-scripts/reddit/css/hide-games.css',
        hideCustomFeeds:        '/content-scripts/reddit/css/hide-custom-feeds.css',
        hideRecentSubreddits:   '/content-scripts/reddit/css/hide-recent-subreddits.css',
        hideCommunities:        '/content-scripts/reddit/css/hide-communities.css',
        hideRightSidebar:       '/content-scripts/reddit/css/hide-right-sidebar.css',
        hideRecentPosts:        '/content-scripts/reddit/css/hide-recent-posts.css',
        hideSubredditInfo:      '/content-scripts/reddit/css/hide-subreddit-info.css',
        hidePopularCommunities: '/content-scripts/reddit/css/hide-popular-communities.css',
        hideComments:           '/content-scripts/reddit/css/hide-comments.css',
        hideUpvotes:            '/content-scripts/reddit/css/hide-upvotes.css',
        hideUpvoteCount:        '/content-scripts/reddit/css/hide-upvote-count.css',
        hideSearch:             '/content-scripts/reddit/css/hide-search.css',
        hideTrending:           '/content-scripts/reddit/css/hide-trending.css',
        hideNotifications:      '/content-scripts/reddit/css/hide-notifications.css',
    };

    // ─────────────────────────────────────────────────────────────────────────
    // Page context
    // ─────────────────────────────────────────────────────────────────────────

    function isSubredditPage() {
        return /^\/r\/[^/]+\/?$/.test(window.location.pathname);
    }

    function isUserPage() {
        return window.location.pathname.startsWith('/user');
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Feature Implementations
    //
    // Each follows the pattern:
    //   1. Clear any existing interval
    //   2. Toggle the CSS file (inject or remove the <style> tag)
    //   3. If enabled: enforce via JS + set interval for dynamic content
    //   4. If disabled: restore hidden elements
    // ─────────────────────────────────────────────────────────────────────────

    // ── Main Feed Controls ───────────────────────────────────────────────────

    function hideHomeFeed(enabled) {
        const ID = 'rd-hide-home-feed';
        DPB.clearInterval(ID);
        DPB.toggleCSS(CSS.hideHomeFeed, ID, enabled);

        if (enabled) {
            function enforce() {
                if (isSubredditPage() || isUserPage()) return;
                DPB.hideElements(SELECTORS.homeFeed);
            }
            enforce();
            DPB.setInterval(ID, enforce, 500);
        } else {
            DPB.showElements(SELECTORS.homeFeed);
        }
    }

    function hideGallery(enabled) {
        const ID = 'rd-hide-gallery';
        DPB.clearInterval(ID);
        DPB.toggleCSS(CSS.hideGallery, ID, enabled);

        if (enabled) {
            function enforce() { DPB.hideElements(SELECTORS.gallery); }
            enforce();
            DPB.setInterval(ID, enforce, 500);
        } else {
            DPB.showElements(SELECTORS.gallery);
        }
    }

    function hideSubredditFeed(enabled) {
        const ID = 'rd-hide-sub-feed';
        DPB.clearInterval(ID);
        DPB.toggleCSS(CSS.hideSubredditFeed, ID, enabled);

        if (enabled) {
            function enforce() {
                if (!isSubredditPage()) return;
                DPB.hideElements(SELECTORS.homeFeed);
            }
            enforce();
            DPB.setInterval(ID, enforce, 500);
        } else {
            if (isSubredditPage()) DPB.showElements(SELECTORS.homeFeed);
        }
    }

    function hideCommunityHighlights(enabled) {
        const ID = 'rd-hide-highlights';
        DPB.clearInterval(ID);
        DPB.toggleCSS(CSS.hideCommunityHighlights, ID, enabled);

        if (enabled) {
            function enforce() { DPB.hideElements(SELECTORS.communityHighlights); }
            enforce();
            DPB.setInterval(ID, enforce, 500);
        } else {
            DPB.showElements(SELECTORS.communityHighlights);
        }
    }

    // ── Left Sidebar ─────────────────────────────────────────────────────────
    // When hideLeftSidebar is on, all sub-options are redundant (whole bar gone)

    function hideLeftSidebar(enabled) {
        const ID = 'rd-hide-left-sidebar';
        DPB.clearInterval(ID);
        DPB.toggleCSS(CSS.hideLeftSidebar, ID, enabled);

        if (enabled) {
            function enforce() { DPB.hideElements(SELECTORS.leftSidebar); }
            enforce();
            DPB.setInterval(ID, enforce, 500);
        } else {
            DPB.showElements(SELECTORS.leftSidebar);
        }
    }

    function hidePopular(enabled) {
        const ID = 'rd-hide-popular';
        DPB.clearInterval(ID);
        DPB.toggleCSS(CSS.hidePopular, ID, enabled);

        if (enabled) {
            function enforce() {
                DPB.hideElements(SELECTORS.popular);
                if (window.location.pathname.startsWith('/r/popular')) {
                    DPB.redirectIf(true, '/');
                }
            }
            enforce();
            DPB.setInterval(ID, enforce, 500);
        } else {
            DPB.showElements(SELECTORS.popular);
        }
    }

    function hideExplore(enabled) {
        const ID = 'rd-hide-explore';
        DPB.clearInterval(ID);
        DPB.toggleCSS(CSS.hideExplore, ID, enabled);

        if (enabled) {
            function enforce() {
                DPB.hideElements(SELECTORS.explore);
                if (window.location.pathname.startsWith('/explore')) {
                    DPB.redirectIf(true, '/');
                }
            }
            enforce();
            DPB.setInterval(ID, enforce, 500);
        } else {
            DPB.showElements(SELECTORS.explore);
        }
    }

    function hideAll(enabled) {
        const ID = 'rd-hide-all';
        DPB.clearInterval(ID);
        DPB.toggleCSS(CSS.hideAll, ID, enabled);

        if (enabled) {
            function enforce() {
                DPB.hideElements(SELECTORS.all);
                if (window.location.pathname.startsWith('/r/all')) {
                    DPB.redirectIf(true, '/');
                }
            }
            enforce();
            DPB.setInterval(ID, enforce, 500);
        } else {
            DPB.showElements(SELECTORS.all);
        }
    }

    function hideGames(enabled) {
        const ID = 'rd-hide-games';
        DPB.clearInterval(ID);
        DPB.toggleCSS(CSS.hideGames, ID, enabled);

        if (enabled) {
            function enforce() { DPB.hideElements(SELECTORS.games); }
            enforce();
            DPB.setInterval(ID, enforce, 500);
        } else {
            DPB.showElements(SELECTORS.games);
        }
    }

    function hideCustomFeeds(enabled) {
        const ID = 'rd-hide-custom-feeds';
        DPB.clearInterval(ID);
        DPB.toggleCSS(CSS.hideCustomFeeds, ID, enabled);

        if (enabled) {
            function enforce() { DPB.hideElements(SELECTORS.customFeeds); }
            enforce();
            DPB.setInterval(ID, enforce, 500);
        } else {
            DPB.showElements(SELECTORS.customFeeds);
        }
    }

    function hideRecentSubreddits(enabled) {
        const ID = 'rd-hide-recent-subs';
        DPB.clearInterval(ID);
        DPB.toggleCSS(CSS.hideRecentSubreddits, ID, enabled);

        if (enabled) {
            function enforce() { DPB.hideElements(SELECTORS.recentSubreddits); }
            enforce();
            DPB.setInterval(ID, enforce, 500);
        } else {
            DPB.showElements(SELECTORS.recentSubreddits);
        }
    }

    function hideCommunities(enabled) {
        const ID = 'rd-hide-communities';
        DPB.clearInterval(ID);
        DPB.toggleCSS(CSS.hideCommunities, ID, enabled);

        if (enabled) {
            function enforce() { DPB.hideElements(SELECTORS.communities); }
            enforce();
            DPB.setInterval(ID, enforce, 500);
        } else {
            DPB.showElements(SELECTORS.communities);
        }
    }

    // ── Right Sidebar ────────────────────────────────────────────────────────
    // When hideRightSidebar is on, sub-options are redundant

    function hideRightSidebar(enabled) {
        const ID = 'rd-hide-right-sidebar';
        DPB.clearInterval(ID);
        DPB.toggleCSS(CSS.hideRightSidebar, ID, enabled);

        if (enabled) {
            function enforce() { DPB.hideElements(SELECTORS.rightSidebar); }
            enforce();
            DPB.setInterval(ID, enforce, 500);
        } else {
            DPB.showElements(SELECTORS.rightSidebar);
        }
    }

    function hideRecentPosts(enabled) {
        const ID = 'rd-hide-recent-posts';
        DPB.clearInterval(ID);
        DPB.toggleCSS(CSS.hideRecentPosts, ID, enabled);

        if (enabled) {
            function enforce() { DPB.hideElements(SELECTORS.recentPosts); }
            enforce();
            DPB.setInterval(ID, enforce, 500);
        } else {
            DPB.showElements(SELECTORS.recentPosts);
        }
    }

    function hideSubredditInfo(enabled) {
        const ID = 'rd-hide-sub-info';
        DPB.clearInterval(ID);
        DPB.toggleCSS(CSS.hideSubredditInfo, ID, enabled);

        if (enabled) {
            function enforce() { DPB.hideElements(SELECTORS.subredditInfo); }
            enforce();
            DPB.setInterval(ID, enforce, 500);
        } else {
            DPB.showElements(SELECTORS.subredditInfo);
        }
    }

    function hidePopularCommunities(enabled) {
        const ID = 'rd-hide-pop-communities';
        DPB.clearInterval(ID);
        DPB.toggleCSS(CSS.hidePopularCommunities, ID, enabled);

        if (enabled) {
            function enforce() { DPB.hideElements(SELECTORS.popularCommunities); }
            enforce();
            DPB.setInterval(ID, enforce, 500);
        } else {
            DPB.showElements(SELECTORS.popularCommunities);
        }
    }

    // ── Content Controls ─────────────────────────────────────────────────────

    function hideComments(enabled) {
        const ID = 'rd-hide-comments';
        DPB.clearInterval(ID);
        DPB.toggleCSS(CSS.hideComments, ID, enabled);

        if (enabled) {
            function enforce() { DPB.hideElements(SELECTORS.comments); }
            enforce();
            DPB.setInterval(ID, enforce, 500);
        } else {
            DPB.showElements(SELECTORS.comments);
        }
    }

    function hideUpvotes(enabled) {
        const ID = 'rd-hide-upvotes';
        DPB.clearInterval(ID);
        DPB.toggleCSS(CSS.hideUpvotes, ID, enabled);

        if (enabled) {
            function enforce() { DPB.hideElements(SELECTORS.upvotes); }
            enforce();
            DPB.setInterval(ID, enforce, 500);
        } else {
            DPB.showElements(SELECTORS.upvotes);
        }
    }

    function hideUpvoteCount(enabled) {
        const ID = 'rd-hide-upvote-count';
        DPB.clearInterval(ID);
        DPB.toggleCSS(CSS.hideUpvoteCount, ID, enabled);
        // CSS uses visibility:hidden so buttons stay, just count vanishes
    }

    // ── Search Controls ──────────────────────────────────────────────────────

    function hideSearch(enabled) {
        const ID = 'rd-hide-search';
        DPB.clearInterval(ID);
        DPB.toggleCSS(CSS.hideSearch, ID, enabled);

        if (enabled) {
            function enforce() { DPB.hideElements(SELECTORS.search); }
            enforce();
            DPB.setInterval(ID, enforce, 500);
        } else {
            DPB.showElements(SELECTORS.search);
        }
    }

    function hideTrending(enabled) {
        const ID = 'rd-hide-trending';
        DPB.clearInterval(ID);
        DPB.toggleCSS(CSS.hideTrending, ID, enabled);

        if (enabled) {
            function enforce() {
                DPB.hideElements(SELECTORS.trending);
                DPB.hideElements(SELECTORS.trendingLabel);
                DPB.hideElements(SELECTORS.trendingContainer);
            }
            enforce();
            DPB.setInterval(ID, enforce, 500);
        } else {
            DPB.showElements(SELECTORS.trending);
            DPB.showElements(SELECTORS.trendingLabel);
            DPB.showElements(SELECTORS.trendingContainer);
        }
    }

    // ── Notifications ────────────────────────────────────────────────────────

    function hideNotifications(enabled) {
        const ID = 'rd-hide-notifications';
        DPB.clearInterval(ID);
        DPB.toggleCSS(CSS.hideNotifications, ID, enabled);

        if (enabled) {
            function enforce() {
                DPB.hideElements(SELECTORS.notifications);
                if (window.location.pathname.startsWith('/notifications')) {
                    DPB.redirectIf(true, '/');
                }
            }
            enforce();
            DPB.setInterval(ID, enforce, 500);
        } else {
            DPB.showElements(SELECTORS.notifications);
        }
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Cleanup
    // ─────────────────────────────────────────────────────────────────────────

    function cleanup() {
        DPB.clearAllIntervals();

        // Remove all injected CSS
        Object.keys(CSS).forEach(key => {
            // Convert camelCase key to the ID we used
            const idMap = {
                hideHomeFeed: 'rd-hide-home-feed',
                hideGallery: 'rd-hide-gallery',
                hideSubredditFeed: 'rd-hide-sub-feed',
                hideCommunityHighlights: 'rd-hide-highlights',
                hideLeftSidebar: 'rd-hide-left-sidebar',
                hidePopular: 'rd-hide-popular',
                hideExplore: 'rd-hide-explore',
                hideAll: 'rd-hide-all',
                hideGames: 'rd-hide-games',
                hideCustomFeeds: 'rd-hide-custom-feeds',
                hideRecentSubreddits: 'rd-hide-recent-subs',
                hideCommunities: 'rd-hide-communities',
                hideRightSidebar: 'rd-hide-right-sidebar',
                hideRecentPosts: 'rd-hide-recent-posts',
                hideSubredditInfo: 'rd-hide-sub-info',
                hidePopularCommunities: 'rd-hide-pop-communities',
                hideComments: 'rd-hide-comments',
                hideUpvotes: 'rd-hide-upvotes',
                hideUpvoteCount: 'rd-hide-upvote-count',
                hideSearch: 'rd-hide-search',
                hideTrending: 'rd-hide-trending',
                hideNotifications: 'rd-hide-notifications',
            };
            if (idMap[key]) DPB.removeCSS(idMap[key]);
        });

        // Restore all hidden elements
        Object.values(SELECTORS).forEach(selector => {
            DPB.showElements(selector);
        });
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Apply Config
    //
    // Respects parent/child toggle relationships:
    //   - hideLeftSidebar=true  → skip left sidebar sub-options (whole bar gone)
    //   - hideRightSidebar=true → skip right sidebar sub-options
    //   - hideComments=true     → skip upvote sub-options
    //   - hideSearch=true       → skip trending sub-option
    // ─────────────────────────────────────────────────────────────────────────

    DPB.applyConfig = function() {
        const config = DPB.state.config;

        if (!config || !config.enabled) {
            DPB.log('Reddit enforcement disabled, cleaning up');
            cleanup();
            return;
        }

        DPB.log('Applying Reddit config:', config);

        // Main feed controls
        hideHomeFeed(config.hideHomeFeed ?? false);
        hideGallery(config.hideGallery ?? false);
        hideSubredditFeed(config.hideSubredditFeed ?? false);
        hideCommunityHighlights(config.hideCommunityHighlights ?? false);

        // Left sidebar: parent overrides children
        const leftSidebarHidden = config.hideLeftSidebar ?? false;
        hideLeftSidebar(leftSidebarHidden);

        if (leftSidebarHidden) {
            // Whole bar is gone — disable children so they don't waste cycles
            hidePopular(false);
            hideExplore(false);
            hideAll(false);
            hideGames(false);
            hideCustomFeeds(false);
            hideRecentSubreddits(false);
            hideCommunities(false);
        } else {
            hidePopular(config.hidePopular ?? false);
            hideExplore(config.hideExplore ?? false);
            hideAll(config.hideAll ?? false);
            hideGames(config.hideGames ?? false);
            hideCustomFeeds(config.hideCustomFeeds ?? false);
            hideRecentSubreddits(config.hideRecentSubreddits ?? false);
            hideCommunities(config.hideCommunities ?? false);
        }

        // Right sidebar: parent overrides children
        const rightSidebarHidden = config.hideRightSidebar ?? false;
        hideRightSidebar(rightSidebarHidden);

        if (rightSidebarHidden) {
            hideRecentPosts(false);
            hideSubredditInfo(false);
            hidePopularCommunities(false);
        } else {
            hideRecentPosts(config.hideRecentPosts ?? false);
            hideSubredditInfo(config.hideSubredditInfo ?? false);
            hidePopularCommunities(config.hidePopularCommunities ?? false);
        }

        // Comments: parent overrides children
        const commentsHidden = config.hideComments ?? false;
        hideComments(commentsHidden);

        if (commentsHidden) {
            hideUpvotes(false);
            hideUpvoteCount(false);
        } else {
            hideUpvotes(config.hideUpvotes ?? false);
            hideUpvoteCount(config.hideUpvoteCount ?? false);
        }

        // Search: parent overrides children
        const searchHidden = config.hideSearch ?? false;
        hideSearch(searchHidden);

        if (searchHidden) {
            hideTrending(false);
        } else {
            hideTrending(config.hideTrending ?? false);
        }

        // Notifications
        hideNotifications(config.hideNotifications ?? false);
    };

    // ─────────────────────────────────────────────────────────────────────────
    // SPA Navigation Detection
    // ─────────────────────────────────────────────────────────────────────────

    function watchNavigation() {
        let lastUrl = location.href;

        DPB.observe(() => {
            if (location.href !== lastUrl) {
                const oldUrl = lastUrl;
                lastUrl = location.href;
                DPB.log(`Navigation: ${oldUrl} → ${lastUrl}`);
                setTimeout(() => DPB.applyConfig(), 100);
            }
        });
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Initialize
    // ─────────────────────────────────────────────────────────────────────────

    async function init() {
        DPB.log('Initializing Reddit content script');
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