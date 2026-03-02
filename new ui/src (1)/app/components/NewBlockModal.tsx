import { useState } from "react";
import { Plus, ChevronDown, Download } from "lucide-react";

interface NewBlockModalProps {
  open: boolean;
  onClose: () => void;
  onSave: (name: string) => void;
}

const tabs = ["Websites", "Website Exceptions", "Apps"] as const;
type Tab = (typeof tabs)[number];

export function NewBlockModal({ open, onClose, onSave }: NewBlockModalProps) {
  const [activeTab, setActiveTab] = useState<Tab>("Websites");
  const [url, setUrl] = useState("");
  const [entries, setEntries] = useState<string[]>([]);

  if (!open) return null;

  const handleAdd = () => {
    if (url.trim()) {
      setEntries((prev) => [...prev, url.trim()]);
      setUrl("");
    }
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter") handleAdd();
  };

  const handleRemoveSelected = () => {
    setEntries([]);
  };

  const handleSave = () => {
    onSave("New Block");
    setEntries([]);
    setUrl("");
  };

  const handleClose = () => {
    setEntries([]);
    setUrl("");
    onClose();
  };

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center"
      style={{ backgroundColor: "rgba(0,0,0,0.6)" }}
      onClick={handleClose}
    >
      <div
        className="w-full max-w-[680px] rounded-xl p-6 flex flex-col gap-5"
        style={{ backgroundColor: "#252542", border: "1px solid #3a3a5c" }}
        onClick={(e) => e.stopPropagation()}
      >
        {/* Title */}
        <h2 className="text-center text-white uppercase tracking-widest">
          What do you want to block?
        </h2>

        {/* Divider */}
        <div style={{ height: 1, backgroundColor: "#3a3a5c" }} />

        {/* Tabs */}
        <div className="flex gap-6">
          {tabs.map((tab) => (
            <button
              key={tab}
              onClick={() => setActiveTab(tab)}
              className="pb-2 cursor-pointer text-sm transition-colors relative"
              style={{
                color: activeTab === tab ? "#ffffff" : "rgba(255,255,255,0.5)",
              }}
            >
              {tab}
              {activeTab === tab && (
                <span
                  className="absolute bottom-0 left-0 w-full h-0.5 rounded-full"
                  style={{ backgroundColor: "#6366f1" }}
                />
              )}
            </button>
          ))}
        </div>

        {/* Input Row */}
        <div className="flex items-center gap-3">
          <input
            type="text"
            placeholder="Enter URL, then click add..."
            value={url}
            onChange={(e) => setUrl(e.target.value)}
            onKeyDown={handleKeyDown}
            className="flex-1 h-10 px-4 rounded-md text-sm text-white placeholder-white/40 outline-none"
            style={{
              backgroundColor: "#1a1a2e",
              border: "1px solid #3a3a5c",
            }}
          />
          <button
            onClick={handleAdd}
            className="h-10 px-4 rounded-md flex items-center gap-1.5 cursor-pointer text-sm text-white"
            style={{ backgroundColor: "#6366f1" }}
          >
            <Plus className="w-4 h-4" />
            Add
          </button>
          <button
            className="h-10 px-4 rounded-md flex items-center gap-1.5 cursor-pointer text-sm text-white"
            style={{ backgroundColor: "#6366f1" }}
          >
            Import
            <ChevronDown className="w-4 h-4" />
          </button>
        </div>

        {/* List Area */}
        <div
          className="rounded-md min-h-[200px] max-h-[280px] overflow-y-auto p-3 flex flex-col gap-1"
          style={{
            backgroundColor: "#1a1a2e",
            border: "1px solid #3a3a5c",
          }}
        >
          {entries.length === 0 ? (
            <div className="flex-1 flex items-center justify-center text-white/30 text-sm">
              No entries yet
            </div>
          ) : (
            entries.map((entry, i) => (
              <div
                key={i}
                className="px-3 py-2 rounded text-sm text-white/80"
                style={{ backgroundColor: "#252542" }}
              >
                {entry}
              </div>
            ))
          )}
        </div>

        {/* Bottom Actions Row */}
        <div className="flex items-center gap-3">
          <button
            className="h-9 px-4 rounded-md flex items-center gap-1.5 cursor-pointer text-sm text-white"
            style={{
              backgroundColor: "#1a1a2e",
              border: "1px solid #3a3a5c",
            }}
          >
            <Download className="w-4 h-4" />
            Export...
          </button>
          <button
            className="h-9 px-4 rounded-md flex items-center gap-1.5 cursor-pointer text-sm text-white"
            style={{
              backgroundColor: "#1a1a2e",
              border: "1px solid #3a3a5c",
            }}
          >
            Select
            <ChevronDown className="w-4 h-4" />
          </button>
          <button
            onClick={handleRemoveSelected}
            className="cursor-pointer text-sm text-white/70 hover:text-white transition-colors"
          >
            Remove selected
          </button>
        </div>

        {/* Divider */}
        <div style={{ height: 1, backgroundColor: "#3a3a5c" }} />

        {/* Footer Buttons */}
        <div className="flex items-center justify-center gap-3">
          <button
            onClick={handleClose}
            className="h-10 px-6 rounded-md cursor-pointer text-sm text-white"
            style={{
              backgroundColor: "#1a1a2e",
              border: "1px solid #3a3a5c",
            }}
          >
            Close without saving
          </button>
          <button
            onClick={handleSave}
            className="h-10 px-6 rounded-md cursor-pointer text-sm text-white"
            style={{ backgroundColor: "#6366f1" }}
          >
            Save As...
          </button>
        </div>
      </div>
    </div>
  );
}
