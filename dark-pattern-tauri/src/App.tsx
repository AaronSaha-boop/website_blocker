// src/App.tsx
//
// Main app component - integrates with daemon via Tauri commands.

import { useState, useEffect } from "react";
import { Plus, ListFilter, Menu, Lock, Unlock, RefreshCw } from "lucide-react";
import { NewBlockModal, NewBlockData } from "./components/NewBlockModal";
import { ScheduleModal } from "./components/ScheduleModal";
import { daemon, Profile, isError, getError } from "./hooks/useDaemon";

interface Block {
  id: string;
  name: string;
  enabled: boolean;
  schedule: string;
  breaks: string;
  users: string;
  locked: boolean;
}

export default function App() {
  const [blocks, setBlocks] = useState<Block[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [modalOpen, setModalOpen] = useState(false);
  const [scheduleModal, setScheduleModal] = useState<{ open: boolean; blockId: string | null }>({
    open: false,
    blockId: null,
  });

  // ─────────────────────────────────────────────────────────────────────────
  // Load profiles from daemon on mount
  // ─────────────────────────────────────────────────────────────────────────

  const loadProfiles = async () => {
    try {
      setLoading(true);
      setError(null);
      const result = await daemon.listProfiles();
      
      if (isError(result)) {
        setError(getError(result));
        return;
      }

      if ("ProfileList" in result) {
        const profiles = result.ProfileList as Profile[];
        setBlocks(
          profiles.map((p) => ({
            id: p.id,
            name: p.name,
            enabled: p.enabled,
            schedule: "Blocked at all times", // TODO: load from schedules
            breaks: "No breaks",
            users: "All users",
            locked: false,
          }))
        );
      }
    } catch (e) {
      setError(e instanceof Error ? e.message : "Failed to connect to daemon");
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    loadProfiles();
  }, []);

  // ─────────────────────────────────────────────────────────────────────────
  // Actions
  // ─────────────────────────────────────────────────────────────────────────

  const toggleBlock = async (id: string) => {
    const block = blocks.find((b) => b.id === id);
    if (!block) return;

    const newEnabled = !block.enabled;

    // Optimistic update
    setBlocks((prev) =>
      prev.map((b) => (b.id === id ? { ...b, enabled: newEnabled } : b))
    );

    // Persist to daemon (triggers policy broadcast to Chrome extension)
    const result = await daemon.updateProfile(id, block.name, newEnabled);
    if (isError(result)) {
      // Revert on failure
      setBlocks((prev) =>
        prev.map((b) => (b.id === id ? { ...b, enabled: !newEnabled } : b))
      );
      setError(getError(result));
    }
  };

  const handleNewBlock = async (data: NewBlockData) => {
    try {
      setError(null);

      // 1. Create the profile
      const result = await daemon.createProfile(data.name);
      if (isError(result)) {
        setError(getError(result));
        return;
      }

      // Extract the new profile's ID
      let profileId: string | null = null;
      if ("Profile" in result) {
        profileId = (result.Profile as Profile).id;
      }
      if (!profileId) {
        setError("Profile created but no ID returned");
        await loadProfiles();
        setModalOpen(false);
        return;
      }

      // 2. Add all blocked websites
      for (const domain of data.websites) {
        try {
          await daemon.addBlockedWebsite(profileId, domain);
        } catch (e) {
          console.error(`Failed to add website ${domain}:`, e);
        }
      }

      // 3. Add all DOM rules
      for (const rule of data.domRules) {
        try {
          await daemon.addDomRule(profileId, rule.site, rule.toggle);
        } catch (e) {
          console.error(`Failed to add DOM rule ${rule.site}/${rule.toggle}:`, e);
        }
      }

      // 4. Add all blocked apps
      for (const appIdentifier of data.apps) {
        try {
          await daemon.addBlockedApp(profileId, appIdentifier);
        } catch (e) {
          console.error(`Failed to add app ${appIdentifier}:`, e);
        }
      }

      // Reload to reflect the new profile with all its rules
      await loadProfiles();
      setModalOpen(false);
    } catch (e) {
      setError(e instanceof Error ? e.message : "Failed to create profile");
    }
  };

  const handleScheduleSave = async (schedule: string) => {
    if (scheduleModal.blockId !== null) {
      // TODO: Save schedule to daemon
      setBlocks((prev) =>
        prev.map((b) =>
          b.id === scheduleModal.blockId ? { ...b, schedule } : b
        )
      );
    }
    setScheduleModal({ open: false, blockId: null });
  };

  const activeBlock = blocks.find((b) => b.id === scheduleModal.blockId);

  // ─────────────────────────────────────────────────────────────────────────
  // Render
  // ─────────────────────────────────────────────────────────────────────────

  if (loading) {
    return (
      <div
        className="min-h-screen w-full flex items-center justify-center"
        style={{ backgroundColor: "#1a1a2e", color: "#ffffff" }}
      >
        <div className="text-center">
          <RefreshCw className="w-8 h-8 animate-spin mx-auto mb-4 text-indigo-500" />
          <p>Connecting to daemon...</p>
        </div>
      </div>
    );
  }

  return (
    <div
      className="min-h-screen w-full p-8"
      style={{ backgroundColor: "#1a1a2e", color: "#ffffff" }}
    >
      <div className="max-w-[1200px] mx-auto">
        {/* Error Banner */}
        {error && (
          <div
            className="mb-4 p-4 rounded-lg flex items-center justify-between"
            style={{ backgroundColor: "#ef4444", color: "white" }}
          >
            <span>{error}</span>
            <button
              onClick={() => setError(null)}
              className="text-white/80 hover:text-white"
            >
              ✕
            </button>
          </div>
        )}

        {/* Header */}
        <div className="flex items-center justify-between mb-6">
          <h1 className="tracking-wider uppercase" style={{ letterSpacing: "0.05em" }}>
            Website & App Blocks
          </h1>
          <div className="flex items-center gap-3">
            <button
              onClick={() => setModalOpen(true)}
              className="w-10 h-10 flex items-center justify-center rounded-md cursor-pointer"
              style={{ backgroundColor: "#6366f1" }}
            >
              <Plus className="w-5 h-5 text-white" />
            </button>
            <button
              onClick={loadProfiles}
              className="w-10 h-10 flex items-center justify-center rounded-md cursor-pointer"
              style={{ backgroundColor: "#252542" }}
              title="Refresh"
            >
              <RefreshCw className="w-5 h-5 text-white" />
            </button>
            <button
              className="w-10 h-10 flex items-center justify-center rounded-md cursor-pointer"
              style={{ backgroundColor: "#252542" }}
            >
              <ListFilter className="w-5 h-5 text-white" />
            </button>
            <button
              className="w-10 h-10 flex items-center justify-center rounded-md cursor-pointer"
              style={{ backgroundColor: "#252542" }}
            >
              <Menu className="w-5 h-5 text-white" />
            </button>
          </div>
        </div>

        {/* Block Cards */}
        {blocks.length === 0 ? (
          <div
            className="text-center py-16 rounded-lg"
            style={{ backgroundColor: "#252542" }}
          >
            <p className="text-white/60 mb-4">No blocks yet</p>
            <button
              onClick={() => setModalOpen(true)}
              className="px-5 py-2.5 rounded-lg cursor-pointer flex items-center gap-2 text-white mx-auto"
              style={{ backgroundColor: "#6366f1" }}
            >
              <Plus className="w-4 h-4" />
              Create your first block
            </button>
          </div>
        ) : (
          <div className="flex flex-col gap-4">
            {blocks.map((block) => (
              <div
                key={block.id}
                className="rounded-lg overflow-hidden"
                style={{
                  backgroundColor: "#252542",
                  borderLeft: "4px solid #6366f1",
                }}
              >
                <div className="p-5">
                  {/* Top row: name + toggle */}
                  <div className="flex items-center justify-between mb-3">
                    <span className="text-white" style={{ fontSize: "1.15rem" }}>
                      {block.name}
                    </span>
                    <div className="flex items-center gap-3">
                      {/* Toggle */}
                      <button
                        onClick={() => toggleBlock(block.id)}
                        className="relative w-14 h-7 rounded-full cursor-pointer transition-colors"
                        style={{
                          backgroundColor: block.enabled ? "#6366f1" : "#3a3a5c",
                        }}
                      >
                        <span
                          className="absolute top-0.5 w-6 h-6 rounded-full bg-white transition-transform"
                          style={{
                            left: block.enabled ? "calc(100% - 1.625rem)" : "0.125rem",
                          }}
                        />
                      </button>
                      <span className="text-white/70 text-sm">
                        {block.enabled ? "On" : "Off"}
                      </span>
                    </div>
                  </div>

                  {/* Bottom row: details */}
                  <div className="flex items-center gap-0 text-sm text-white/70">
                    <button
                      className="flex-1 text-left cursor-pointer bg-transparent text-white/70 hover:text-white transition-colors text-sm"
                      onClick={() => setScheduleModal({ open: true, blockId: block.id })}
                    >
                      {block.schedule}
                    </button>
                    <span className="flex-1">{block.breaks}</span>
                    <span className="flex-1">{block.users}</span>
                    <span className="flex items-center gap-1.5">
                      {block.locked ? "Locked" : "Unlocked"}
                      {block.locked ? (
                        <Lock className="w-4 h-4" />
                      ) : (
                        <Unlock className="w-4 h-4" />
                      )}
                    </span>
                  </div>
                </div>
              </div>
            ))}
          </div>
        )}

        {/* New Block Button */}
        {blocks.length > 0 && (
          <div className="mt-6">
            <button
              onClick={() => setModalOpen(true)}
              className="px-5 py-2.5 rounded-lg cursor-pointer flex items-center gap-2 text-white"
              style={{ backgroundColor: "#6366f1" }}
            >
              <Plus className="w-4 h-4" />
              New block...
            </button>
          </div>
        )}
      </div>

      {/* Modals */}
      <NewBlockModal
        open={modalOpen}
        onClose={() => setModalOpen(false)}
        onSave={handleNewBlock}
      />
      <ScheduleModal
        open={scheduleModal.open}
        blockName={activeBlock?.name ?? ""}
        onClose={() => setScheduleModal({ open: false, blockId: null })}
        onSave={handleScheduleSave}
      />
    </div>
  );
}
