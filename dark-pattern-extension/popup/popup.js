// popup/popup.js
// Popup UI logic

document.addEventListener('DOMContentLoaded', async () => {
    // Get current state
    const state = await getState();
    updateUI(state);

    // Set up duration buttons
    document.querySelectorAll('.duration-buttons .btn').forEach(btn => {
        btn.addEventListener('click', () => {
            const duration = parseInt(btn.dataset.duration, 10);
            startSession(duration);
        });
    });

    // Set up site toggles
    const sites = ['youtube', 'instagram', 'twitter', 'reddit', 'facebook', 'linkedin', 'netflix'];
    sites.forEach(site => {
        const toggle = document.getElementById(`toggle-${site}`);
        if (toggle) {
            // Set initial state
            toggle.checked = state.sites?.[site]?.enabled ?? true;
            
            // Listen for changes
            toggle.addEventListener('change', () => {
                updateSiteConfig(site, { enabled: toggle.checked });
            });
        }
    });

    // Update timer every second
    setInterval(async () => {
        const state = await getState();
        updateUI(state);
    }, 1000);
});

// ─────────────────────────────────────────────────────────────────────────
// State Management
// ─────────────────────────────────────────────────────────────────────────

async function getState() {
    return new Promise((resolve) => {
        chrome.runtime.sendMessage({ type: 'GET_STATE' }, (response) => {
            resolve(response || {});
        });
    });
}

async function startSession(duration) {
    return new Promise((resolve) => {
        chrome.runtime.sendMessage({ type: 'START_SESSION', duration }, (response) => {
            resolve(response);
            if (response?.success) {
                getState().then(updateUI);
            }
        });
    });
}

async function updateSiteConfig(site, config) {
    return new Promise((resolve) => {
        chrome.runtime.sendMessage({ 
            type: 'UPDATE_SITE_CONFIG', 
            site, 
            config 
        }, resolve);
    });
}

//
// ─────────────────────────────────────────────────────────────────────────
// UI Updates
// ─────────────────────────────────────────────────────────────────────────

function updateUI(state) {
    const statusIdle = document.getElementById('status-idle');
    const statusActive = document.getElementById('status-active');
    const timeRemaining = document.getElementById('time-remaining');
    const buttons = document.querySelectorAll('.duration-buttons .btn');
    const daemonStatus = document.getElementById('daemon-status');

    if (state.sessionActive && state.sessionEndTime) {
        const remaining = Math.max(0, state.sessionEndTime - Date.now());
        
        statusIdle.classList.add('hidden');
        statusActive.classList.remove('hidden');
        timeRemaining.textContent = formatTime(remaining);
        
        // Disable start buttons during active session
        buttons.forEach(btn => btn.disabled = true);
    } else {
        statusIdle.classList.remove('hidden');
        statusActive.classList.add('hidden');
        
        buttons.forEach(btn => btn.disabled = false);
    }

    // Update daemon connection status
    // TODO: Get actual daemon connection status
    daemonStatus.textContent = 'Daemon: Connected';
    daemonStatus.classList.add('connected');
}

function formatTime(ms) {
    const totalSeconds = Math.floor(ms / 1000);
    const hours = Math.floor(totalSeconds / 3600);
    const minutes = Math.floor((totalSeconds % 3600) / 60);
    const seconds = totalSeconds % 60;

    if (hours > 0) {
        return `${hours}:${pad(minutes)}:${pad(seconds)}`;
    }
    return `${minutes}:${pad(seconds)}`;
}

function pad(n) {
    return n.toString().padStart(2, '0');
}

// // when toggle changes
// toggle.addEventListener('change', async () => {
//     await updateSiteConfig(site, { enabled: toggle.checked });
    
//     // Reload active tab to apply changes
//     const [tab] = await chrome.tabs.query({ active: true, currentWindow: true });
//     if (tab?.url?.includes(site)) {
//         chrome.tabs.reload(tab.id);
//     }
// });
