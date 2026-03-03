import { useState } from "react";
import { Plus, ListFilter, Menu, Lock, Unlock } from "lucide-react";
import { NewBlockModal } from "./components/NewBlockModal";
import { ScheduleModal } from "./components/ScheduleModal";

interface Block {
  id: number;
  name: string;
  enabled: boolean;
  schedule: string;
  breaks: string;
  users: string;
  locked: boolean;
}

export default function App() {
  const [blocks, setBlocks] = useState<Block[]>([
    {
      id: 1,
      name: "Distractions",
      enabled: false,
      schedule: "Blocked at all times",
      breaks: "No breaks",
      users: "All users",
      locked: false,
    },
  ]);
  const [modalOpen, setModalOpen] = useState(false);
  const [scheduleModal, setScheduleModal] = useState<{ open: boolean; blockId: number | null }>({
    open: false,
    blockId: null,
  });

  const toggleBlock = (id: number) => {
    setBlocks((prev) =>
      prev.map((b) => (b.id === id ? { ...b, enabled: !b.enabled } : b))
    );
  };

  const handleNewBlock = (name: string) => {
    setBlocks((prev) => [
      ...prev,
      {
        id: Date.now(),
        name,
        enabled: false,
        schedule: "Blocked at all times",
        breaks: "No breaks",
        users: "All users",
        locked: false,
      },
    ]);
    setModalOpen(false);
  };

  const handleScheduleSave = (schedule: string) => {
    if (scheduleModal.blockId !== null) {
      setBlocks((prev) =>
        prev.map((b) =>
          b.id === scheduleModal.blockId ? { ...b, schedule } : b
        )
      );
    }
    setScheduleModal({ open: false, blockId: null });
  };

  const activeBlock = blocks.find((b) => b.id === scheduleModal.blockId);

  return (
    <div
      className="min-h-screen w-full p-8"
      style={{ backgroundColor: "#1a1a2e", color: "#ffffff" }}
    >
      <div className="max-w-[1200px] mx-auto">
        {/* Header */}
        <div className="flex items-center justify-between mb-6">
          <h1 className="tracking-wider uppercase" style={{ letterSpacing: "0.05em" }}>
            Website & App Blocks
          </h1>
          <div className="flex items-center gap-3">
            <button
              className="w-10 h-10 flex items-center justify-center rounded-md cursor-pointer"
              style={{ backgroundColor: "#6366f1" }}
            >
              <Plus className="w-5 h-5 text-white" />
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

        {/* New Block Button */}
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
