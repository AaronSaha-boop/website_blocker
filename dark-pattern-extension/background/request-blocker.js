// background/request-blocker.js
// Blocks entire sites when they appear in blockedSites.
//
// blockedSites is populated from two sources:
//   1. Active focus sessions (manual timer)
//   2. Scheduled profiles pushed by the daemon
//
// Both paths write to state.blockedSites, so this blocker
// doesn't need to know which source put a site on the list.

export class RequestBlocker {
    constructor(stateManager) {
        this.state = stateManager;
    }

    async shouldBlockUrl(url) {
        const state = await this.state.getState();

        if (!state.blockedSites || state.blockedSites.length === 0) return false;

        try {
            const hostname = new URL(url).hostname;

            return state.blockedSites.some(site =>
                hostname === site || hostname.endsWith('.' + site)
            );
        } catch {
            return false;
        }
    }

    async isHostBlocked(hostname) {
        const state = await this.state.getState();

        if (!state.blockedSites || state.blockedSites.length === 0) return false;

        return state.blockedSites.some(site =>
            hostname === site || hostname.endsWith('.' + site)
        );
    }
}
