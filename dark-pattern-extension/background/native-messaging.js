// background/native-messaging.js
// Communicates with the Rust daemon via native messaging.
//
// The native host keeps a persistent connection to the daemon and forwards
// policy change pushes. The key push type is `activePolicy` which carries
// the full merged policy (blocked_websites, blocked_apps, dom_rules).

const NATIVE_HOST = 'com.aaron.dark_pattern';

export class NativeMessenger {
    constructor(stateManager) {
        this.state = stateManager;
        this.port = null;
        this.connected = false;
        this.reconnectAttempts = 0;
        this.maxReconnectAttempts = 5;

        this.connect();
    }

    connect() {
        try {
            this.port = chrome.runtime.connectNative(NATIVE_HOST);
            this.connected = true;
            this.reconnectAttempts = 0;

            this.port.onMessage.addListener((msg) => this.handleMessage(msg));

            this.port.onDisconnect.addListener(() => {
                this.connected = false;
                console.log('Native host disconnected:', chrome.runtime.lastError?.message);
                this.scheduleReconnect();
            });

            // Request initial status after connection settles
            setTimeout(() => {
                this.sendMessage({ type: 'GET_STATUS' });
            }, 100);

            console.log('Connected to native host');
        } catch (error) {
            console.error('Failed to connect to native host:', error);
            this.scheduleReconnect();
        }
    }

    scheduleReconnect() {
        if (this.reconnectAttempts >= this.maxReconnectAttempts) {
            console.error('Max reconnect attempts reached');
            return;
        }

        this.reconnectAttempts++;
        const delay = Math.min(1000 * Math.pow(2, this.reconnectAttempts), 30000);
        setTimeout(() => this.connect(), delay);
    }

    sendMessage(message) {
        if (!this.connected || !this.port) {
            console.warn('Not connected to native host');
            return false;
        }

        try {
            console.log('Sending to native host:', JSON.stringify(message));
            this.port.postMessage(message);
            return true;
        } catch (error) {
            console.error('Failed to send message:', error);
            return false;
        }
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Inbound message handler
    // ─────────────────────────────────────────────────────────────────────────

    async handleMessage(msg) {
        console.log('Received from daemon:', msg);

        switch (msg.type) {
            // ── Push: policy changed (daemon broadcast) ──────────────────────
            case 'activePolicy':
                await this.handlePolicyUpdate(msg.policy);
                break;

            // ── Response: session started ────────────────────────────────────
            case 'started':
                await this.state.updateState({
                    sessionActive: true,
                    sessionEndTime: Date.now() + (msg.duration * 1000)
                });
                this.sendMessage({ type: 'GET_STATUS' });
                break;

            // ── Response: session stopped ────────────────────────────────────
            case 'stopped':
                await this.state.updateState({
                    sessionActive: false,
                    sessionEndTime: null
                });
                break;

            // ── Response: current status ─────────────────────────────────────
            case 'status':
                if (msg.active) {
                    await this.state.updateState({
                        sessionActive: true,
                        sessionEndTime: Date.now() + ((msg.time_left || 0) * 1000),
                        blockedSites: msg.sites || []
                    });
                } else {
                    await this.state.updateState({
                        sessionActive: false,
                        sessionEndTime: null,
                        blockedSites: msg.sites || []
                    });
                }
                break;

            // ── Response: error ──────────────────────────────────────────────
            case 'error':
                console.error('Daemon error:', msg.message);
                break;

            // ── CRUD responses (forward to popup if open) ────────────────────
            case 'profile':
            case 'profileList':
            case 'profileUpdated':
            case 'profileDeleted':
            case 'schedule':
            case 'scheduleList':
            case 'scheduleUpdated':
            case 'scheduleDeleted':
            case 'blockedWebsite':
            case 'blockedWebsiteList':
            case 'blockedWebsiteDeleted':
            case 'blockedApp':
            case 'blockedAppList':
            case 'blockedAppDeleted':
            case 'domRule':
            case 'domRuleList':
            case 'domRuleDeleted':
            case 'manualSession':
            case 'manualSessionList':
            case 'manualSessionUpdated':
            case 'globalWebsiteAdded':
            case 'globalWebsiteRemoved':
            case 'globalWebsiteList':
                chrome.runtime.sendMessage({ ...msg, source: 'daemon' }).catch(() => {});
                break;

            case 'pong':
                break;

            default:
                console.warn('Unhandled daemon message type:', msg.type, msg);
        }
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Policy update handler
    // ─────────────────────────────────────────────────────────────────────────

    async handlePolicyUpdate(policy) {
        if (!policy) return;

        console.log('=== POLICY PUSH RECEIVED ===');
        console.log('  blocked_websites:', policy.blocked_websites || []);
        console.log('  blocked_apps:', policy.blocked_apps || []);
        console.log('  dom_rules:', (policy.dom_rules || []).map(r => `${r.site}:${r.toggle}`));

        // Apply to state → triggers state.addListener in service-worker.js
        // which handles both tab blocking and DOM rule injection
        const newState = await this.state.applyPolicy(policy);

        console.log('=== POLICY APPLIED ===');
        console.log('  blockedSites in state:', newState.blockedSites);
        console.log('  youtube config:', newState.sites?.youtube);
        console.log('  instagram config:', newState.sites?.instagram);
        console.log('  reddit config:', newState.sites?.reddit);
        console.log('  facebook config:', newState.sites?.facebook);
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Session commands
    // ─────────────────────────────────────────────────────────────────────────

    async startSession(duration) {
        return new Promise((resolve) => {
            if (!this.connected) {
                resolve({ success: false, error: 'Not connected to daemon' });
                return;
            }
            this.sendMessage({ type: 'START', duration });
            resolve({ success: true });
        });
    }

    async stopSession() {
        return new Promise((resolve) => {
            if (!this.connected) {
                resolve({ success: false, error: 'Not connected to daemon' });
                return;
            }
            this.sendMessage({ type: 'STOP' });
            resolve({ success: true });
        });
    }
}