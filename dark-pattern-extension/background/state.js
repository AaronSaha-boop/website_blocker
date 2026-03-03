// background/state.js
// Manages extension state in chrome.storage
//
// IMPORTANT: Toggle defaults are FALSE. The daemon is the authority on what's
// enforced. When the daemon pushes a policy with dom_rules [{site:"youtube",
// toggle:"hideShorts"}], we set hideShorts=true. Toggles NOT in the daemon's
// rules stay false. This means removing a dom_rule from the daemon will
// immediately un-enforce that toggle on the next push.

const DEFAULT_SITE_CONFIGS = {
    instagram: {
        enabled: true,
        hideReels: false,
        hideExplore: false,
        hideStories: false,
        hideSuggested: false,
        hideVanityMetrics: false,
        grayscale: false,
        hideFeed: false
    },
    youtube: {
        enabled: true,
        hideShorts: false,
        hideHomeFeed: false,
        hideRecommendations: false,
        disableAutoplay: false,
        hideComments: false,
        hideEndCards: false,
        grayscale: false
    },
    twitter: {
        enabled: true,
        hideForYou: false,
        hideTrends: false,
        hideWhoToFollow: false,
        hidePromoted: false
    },
    reddit: {
        enabled: true,
        hideHomeFeed: false,
        hideGallery: false,
        hideSubredditFeed: false,
        hideCommunityHighlights: false,
        hideLeftSidebar: false,
        hidePopular: false,
        hideExplore: false,
        hideAll: false,
        hideGames: false,
        hideCustomFeeds: false,
        hideRecentSubreddits: false,
        hideCommunities: false,
        hideRightSidebar: false,
        hideRecentPosts: false,
        hideSubredditInfo: false,
        hidePopularCommunities: false,
        hideComments: false,
        hideUpvotes: false,
        hideUpvoteCount: false,
        hideSearch: false,
        hideTrending: false,
        hideNotifications: false
    },
    facebook: {
        enabled: true,
        hideReels: false,
        hideStories: false,
        hideSuggested: false,
        hideMarketplace: false,
        hideWatch: false,
        hideSponsored: false,
        hideGaming: false,
        hideNotifications: false,
        hideMessengerBtn: false,
        hideGroupsBtn: false,
        hideMetaAI: false,
        hideNewsfeed: false
    },
    linkedin: {
        enabled: true,
        hideFeed: false,
        hideNotificationBadge: false,
        hidePeopleYouMayKnow: false,
        hidePromoted: false
    },
    netflix: {
        enabled: true,
        disableAutoplayPreviews: false,
        hideMoreLikeThis: false,
        hideContinueWatching: false
    }
};

const DEFAULT_STATE = {
    sessionActive: false,
    sessionEndTime: null,
    blockedSites: [],
    blockedApps: [],
    domRules: [],
    sites: structuredClone(DEFAULT_SITE_CONFIGS)
};

export class StateManager {
    constructor() {
        this.listeners = [];
    }

    async getState() {
        const result = await chrome.storage.local.get('state');
        return result.state || structuredClone(DEFAULT_STATE);
    }

    async setState(newState) {
        await chrome.storage.local.set({ state: newState });
        this.notifyListeners(newState);
    }

    async updateState(updates) {
        const current = await this.getState();
        const newState = { ...current, ...updates };
        await this.setState(newState);
        return newState;
    }

    async getSiteConfig(site) {
        const state = await this.getState();
        return state.sites[site] || null;
    }

    async updateSiteConfig(site, config) {
        const state = await this.getState();
        state.sites[site] = { ...state.sites[site], ...config };
        await this.setState(state);
        return state.sites[site];
    }

    async updateSession(isActive, sites, time_left) {
        const sessionShouldBeActive = isActive && sites && sites.length > 0;
        const endTime = time_left ? Date.now() + time_left * 1000 : null;
        await this.updateState({
            sessionActive: sessionShouldBeActive,
            blockedSites: sites || [],
            sessionEndTime: endTime
        });
    }

    /**
     * Apply a full policy push from the daemon.
     *
     * The daemon is the single source of truth for enforcement:
     * - Every toggle starts at false (the default)
     * - Each dom_rule in the policy sets its toggle to true
     * - Toggles NOT mentioned stay false → features get un-enforced
     *
     * This means: add a dom_rule → enforced immediately.
     *             delete a dom_rule → un-enforced on the next push.
     */
    async applyPolicy(policy) {
        const current = await this.getState();

        // Start fresh from defaults (all toggles false)
        const updatedSites = structuredClone(DEFAULT_SITE_CONFIGS);

        // Preserve user "enabled" flags (site-level on/off toggle from popup)
        for (const [site, config] of Object.entries(current.sites || {})) {
            if (updatedSites[site] && config.enabled !== undefined) {
                updatedSites[site].enabled = config.enabled;
            }
        }

        // Apply daemon DOM rules: each {site, toggle} → set that toggle to true
        for (const rule of (policy.dom_rules || [])) {
            if (updatedSites[rule.site]) {
                updatedSites[rule.site][rule.toggle] = true;
            }
        }

        const newState = await this.updateState({
            blockedSites: policy.blocked_websites || [],
            blockedApps: policy.blocked_apps || [],
            domRules: policy.dom_rules || [],
            sites: updatedSites
        });

        return newState;
    }

    async initializeDefaults() {
        const existing = await chrome.storage.local.get('state');
        if (!existing.state) {
            await this.setState(structuredClone(DEFAULT_STATE));
        }
    }

    async isSessionActive() {
        const state = await this.getState();
        if (!state.sessionActive) return false;

        if (state.sessionEndTime && Date.now() > state.sessionEndTime) {
            await this.updateState({ sessionActive: false, sessionEndTime: null });
            return false;
        }
        return true;
    }

    async getTimeRemaining() {
        const state = await this.getState();
        if (!state.sessionActive || !state.sessionEndTime) return 0;
        return Math.max(0, state.sessionEndTime - Date.now());
    }

    addListener(callback) {
        this.listeners.push(callback);
    }

    notifyListeners(state) {
        this.listeners.forEach(cb => cb(state));
    }
}