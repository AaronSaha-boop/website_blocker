## 7. Your Implementation Checklist
```
□ Step 1: Test native host standalone
    echo '{"type":"PING"}' | native-host

□ Step 2: Connect extension to native host
    const port = chrome.runtime.connectNative('com.aaron.dark_pattern');

□ Step 3: Fetch policy on extension load
    port.postMessage({ type: 'GET_ACTIVE_POLICY', ... });

□ Step 4: Store policy in chrome.storage
    chrome.storage.local.set({ policy: response });

□ Step 5: Content script listens for changes
    chrome.storage.onChanged.addListener(...)

□ Step 6: Apply DOM rules based on policy
    applyDomRules(policy.dom_rules);

□ Step 7: Block sites based on policy
    if (isBlocked) redirect to blocked.html