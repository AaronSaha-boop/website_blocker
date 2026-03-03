// content-scripts/netflix/content.js
// Netflix-specific DOM enforcement

(function() {
    'use strict';

    const DPB = window.DarkPatternBlocker;
    const SITE = 'netflix';

    // ─────────────────────────────────────────────────────────────────────────
    // Feature Implementations
    // ─────────────────────────────────────────────────────────────────────────

    function disableAutoplayPreviews(enabled) {
        DPB.clearInterval('disableAutoplay');
        
        if (!enabled) return;

        function disable() {
            // Pause all preview videos
            document.querySelectorAll('video').forEach(video => {
                if (video.closest('.billboard') || video.closest('.jawBone')) {
                    video.pause();
                    video.muted = true;
                }
            });
        }

        disable();
        DPB.setInterval('disableAutoplay', disable, 500);
    }

    function hideMoreLikeThis(enabled) {
        DPB.clearInterval('hideMoreLikeThis');
        
        if (!enabled) return;

        function hide() {
            DPB.hideElements('.moreLikeThis');
        }

        hide();
        DPB.setInterval('hideMoreLikeThis', hide, 500);
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
        
        DPB.log('Applying Netflix config:', config);
        
        disableAutoplayPreviews(config.disableAutoplayPreviews);
        hideMoreLikeThis(config.hideMoreLikeThis);
    };

    // ─────────────────────────────────────────────────────────────────────────
    // Initialize
    // ─────────────────────────────────────────────────────────────────────────

    async function init() {
        DPB.log('Initializing Netflix content script');
        DPB.hidePageUntilReady();
        await DPB.getConfig(SITE);
        DPB.applyConfig();
    }

    init();
})();
