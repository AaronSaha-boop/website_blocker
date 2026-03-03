# Dark Pattern Blocker - Tauri App

Desktop UI for the Dark Pattern Blocker daemon.

## Project Structure

```
dark-pattern-tauri/
├── src/                          # React frontend
│   ├── App.tsx                   # Main component (connected to daemon)
│   ├── main.tsx                  # Entry point
│   ├── index.css                 # Tailwind CSS
│   ├── hooks/
│   │   └── useDaemon.ts          # Tauri → Rust bridge
│   ├── components/
│   │   ├── NewBlockModal.tsx
│   │   ├── ScheduleModal.tsx
│   │   └── ui/                   # UI components
│   └── lib/
│       └── utils.ts
├── src-tauri/                    # Rust backend
│   ├── src/
│   │   ├── main.rs               # Tauri entry point
│   │   └── commands.rs           # All daemon commands
│   ├── Cargo.toml                # Depends on your daemon crate
│   ├── build.rs
│   └── tauri.conf.json
├── package.json
├── vite.config.ts
├── tailwind.config.js
├── tsconfig.json
└── index.html
```

## Setup

### 1. Prerequisites

- Your daemon crate must be accessible
- Node.js 18+
- Rust toolchain

### 2. Update Cargo.toml path

Edit `src-tauri/Cargo.toml` and fix the path to your daemon crate:

```toml
# If your daemon is at ../website_blocker:
dark_pattern_blocker = { path = "../../website_blocker" }

# Or if it's the parent directory:
dark_pattern_blocker = { path = ".." }
```

### 3. Add helper to your client.rs

Copy the contents of `client_additions.rs` into your existing `src/client.rs`:

```rust
// Add to your existing src/client.rs

use crate::protocols::{self, ClientMessage, DaemonMessage, ProtocolError};

pub async fn send_and_receive(
    conn: &mut Connection,
    msg: ClientMessage,
) -> Result<DaemonMessage, ClientError> {
    let bytes = protocols::encode(&msg)?;
    conn.send(&bytes).await?;
    
    let response_bytes = conn
        .recv()
        .await?
        .ok_or(ClientError::ConnectionClosed)?;
    
    let response = protocols::decode(&response_bytes)?;
    Ok(response)
}

#[derive(Debug, thiserror::Error)]
pub enum ClientError {
    #[error("Socket error: {0}")]
    Socket(#[from] SocketError),
    
    #[error("Protocol error: {0}")]
    Protocol(#[from] ProtocolError),
    
    #[error("Connection closed by daemon")]
    ConnectionClosed,
}
```

### 4. Install dependencies

```bash
cd dark-pattern-tauri
npm install
```

### 5. Run in development

```bash
# Make sure daemon is running first!
dark_pattern_blocker daemon

# Then start Tauri dev
npm run tauri dev
```

### 6. Build for production

```bash
npm run tauri build
```

The app will be in `src-tauri/target/release/bundle/`.

## How It Works

```
┌─────────────────────────────────────────────────────────────┐
│                        Tauri App                             │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│   React UI (src/)                                           │
│      │                                                       │
│      │  invoke("get_status")                                │
│      ▼                                                       │
│   useDaemon.ts ─────────► Tauri IPC                         │
│                              │                               │
│                              ▼                               │
│   Rust Backend (src-tauri/)                                 │
│      │                                                       │
│      │  #[tauri::command]                                   │
│      │  async fn get_status()                               │
│      ▼                                                       │
│   commands.rs ──────────► client::send_and_receive()        │
│                              │                               │
│                              ▼                               │
│   Your daemon crate ────► Unix Socket ───► Daemon           │
│                                                              │
└─────────────────────────────────────────────────────────────┘
```

## Available Commands

All CLI commands are available via Tauri:

| Frontend | Tauri Command | Daemon Message |
|----------|--------------|----------------|
| `daemon.getStatus()` | `get_status` | `GetStatus` |
| `daemon.startSession(duration)` | `start_session` | `Start` |
| `daemon.stopSession()` | `stop_session` | `Stop` |
| `daemon.listProfiles()` | `list_profiles` | `ListProfiles` |
| `daemon.createProfile(name)` | `create_profile` | `CreateProfile` |
| `daemon.deleteProfile(id)` | `delete_profile` | `DeleteProfile` |
| `daemon.listGlobalWebsites()` | `list_global_websites` | `ListGlobalWebsites` |
| `daemon.addGlobalWebsite(domain)` | `add_global_website` | `AddGlobalWebsite` |
| ... and all others | | |

## Troubleshooting

### "Failed to connect to daemon"

Make sure the daemon is running:
```bash
dark_pattern_blocker daemon
```

### Compilation errors in commands.rs

Make sure the path in `src-tauri/Cargo.toml` points to your daemon crate correctly.

### Type mismatches

The `DaemonMessage` type in `useDaemon.ts` must match your Rust enum. If you add new variants, update both.
