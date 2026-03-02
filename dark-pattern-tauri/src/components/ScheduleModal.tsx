import { useState, useRef, useCallback } from "react";

interface ScheduleModalProps {
  open: boolean;
  blockName: string;
  onClose: () => void;
  onSave: (schedule: string) => void;
}

const days = ["SUN", "MON", "TUE", "WED", "THU", "FRI", "SAT"];
const hours = [
  "9 am", "10 am", "11 am", "12 pm", "1 pm", "2 pm",
  "3 pm", "4 pm", "5 pm", "6 pm", "7 pm", "8 pm",
  "9 pm", "10 pm", "11 pm",
];

export function ScheduleModal({ open, blockName, onClose, onSave }: ScheduleModalProps) {
  const [mode, setMode] = useState<"all" | "schedule">("all");
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [isDragging, setIsDragging] = useState(false);
  const [dragMode, setDragMode] = useState<"add" | "remove">("add");
  const gridRef = useRef<HTMLDivElement>(null);

  const cellKey = (day: number, hour: number) => `${day}-${hour}`;

  const handleMouseDown = (day: number, hour: number) => {
    const key = cellKey(day, hour);
    const adding = !selected.has(key);
    setDragMode(adding ? "add" : "remove");
    setIsDragging(true);
    setSelected((prev) => {
      const next = new Set(prev);
      if (adding) next.add(key);
      else next.delete(key);
      return next;
    });
  };

  const handleMouseEnter = (day: number, hour: number) => {
    if (!isDragging) return;
    const key = cellKey(day, hour);
    setSelected((prev) => {
      const next = new Set(prev);
      if (dragMode === "add") next.add(key);
      else next.delete(key);
      return next;
    });
  };

  const handleMouseUp = useCallback(() => {
    setIsDragging(false);
  }, []);

  const handleRemoveAll = () => setSelected(new Set());

  const handleSave = () => {
    if (mode === "all") {
      onSave("Blocked at all times");
    } else {
      onSave(selected.size > 0 ? "Custom schedule" : "Blocked at all times");
    }
  };

  if (!open) return null;

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center"
      style={{ backgroundColor: "rgba(0,0,0,0.6)" }}
      onClick={onClose}
      onMouseUp={handleMouseUp}
    >
      <div
        className="w-full max-w-[900px] rounded-xl p-6 flex flex-col gap-5"
        style={{ backgroundColor: "#252542", border: "1px solid #3a3a5c" }}
        onClick={(e) => e.stopPropagation()}
      >
        {/* Title */}
        <h2 className="text-center text-white uppercase tracking-widest">
          When is '{blockName}' blocked?
        </h2>

        {/* Divider */}
        <div style={{ height: 1, backgroundColor: "#3a3a5c" }} />

        {/* Options row */}
        <div className="flex items-start justify-between">
          <div className="flex flex-col gap-2">
            {/* At all times */}
            <label className="flex items-center gap-2 cursor-pointer text-sm text-white">
              <span
                className="w-4 h-4 rounded-full border-2 flex items-center justify-center"
                style={{
                  borderColor: mode === "all" ? "#6366f1" : "#3a3a5c",
                }}
              >
                {mode === "all" && (
                  <span
                    className="w-2 h-2 rounded-full"
                    style={{ backgroundColor: "#6366f1" }}
                  />
                )}
              </span>
              <button
                className="cursor-pointer text-white bg-transparent text-sm"
                onClick={() => setMode("all")}
              >
                At all times
              </button>
            </label>
            {/* According to schedule */}
            <label className="flex items-center gap-2 cursor-pointer text-sm text-white">
              <span
                className="w-4 h-4 rounded-full border-2 flex items-center justify-center"
                style={{
                  borderColor: mode === "schedule" ? "#6366f1" : "#3a3a5c",
                }}
              >
                {mode === "schedule" && (
                  <span
                    className="w-2 h-2 rounded-full"
                    style={{ backgroundColor: "#6366f1" }}
                  />
                )}
              </span>
              <button
                className="cursor-pointer text-white bg-transparent text-sm"
                onClick={() => setMode("schedule")}
              >
                According to this schedule
              </button>
            </label>
          </div>

          <div className="flex flex-col gap-1 items-end">
            <button
              className="cursor-pointer text-sm text-white/70 hover:text-white transition-colors bg-transparent"
            >
              Show other enabled scheduled blocks
            </button>
            <button
              onClick={handleRemoveAll}
              className="cursor-pointer text-sm text-white/70 hover:text-white transition-colors bg-transparent"
            >
              Remove all unlocked blocks
            </button>
          </div>
        </div>

        {/* Schedule Grid */}
        <div
          className="rounded-md overflow-hidden select-none"
          style={{
            backgroundColor: "#1a1a2e",
            border: "1px solid #3a3a5c",
            opacity: mode === "schedule" ? 1 : 0.4,
            pointerEvents: mode === "schedule" ? "auto" : "none",
          }}
          ref={gridRef}
        >
          {/* Day headers */}
          <div className="grid" style={{ gridTemplateColumns: "60px repeat(7, 1fr)" }}>
            <div className="h-9" />
            {days.map((day) => (
              <div
                key={day}
                className="h-9 flex items-center justify-center text-xs text-white/80"
              >
                <span
                  className="px-2 py-0.5 rounded"
                  style={{
                    backgroundColor:
                      day === days[new Date().getDay()] ? "#3a3a5c" : "transparent",
                  }}
                >
                  {day}
                </span>
              </div>
            ))}
          </div>

          {/* Time rows */}
          {hours.map((hour, hIdx) => (
            <div
              key={hour}
              className="grid"
              style={{
                gridTemplateColumns: "60px repeat(7, 1fr)",
                borderTop: "1px solid #2a2a48",
              }}
            >
              <div className="h-7 flex items-center justify-end pr-2 text-xs text-white/50">
                {hour}
              </div>
              {days.map((_, dIdx) => {
                const key = cellKey(dIdx, hIdx);
                const isSelected = selected.has(key);
                return (
                  <div
                    key={key}
                    className="h-7 cursor-pointer transition-colors"
                    style={{
                      backgroundColor: isSelected ? "rgba(99,102,241,0.35)" : "transparent",
                      borderLeft: "1px solid #2a2a48",
                    }}
                    onMouseDown={() => handleMouseDown(dIdx, hIdx)}
                    onMouseEnter={() => handleMouseEnter(dIdx, hIdx)}
                  >
                    {isSelected && (
                      <div
                        className="w-full h-full"
                        style={{
                          borderBottom: "2px solid #f87171",
                        }}
                      />
                    )}
                  </div>
                );
              })}
            </div>
          ))}
        </div>

        {/* Divider */}
        <div style={{ height: 1, backgroundColor: "#3a3a5c" }} />

        {/* Footer */}
        <div className="flex items-center justify-end gap-3">
          <button
            onClick={onClose}
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
            Save
          </button>
        </div>
      </div>
    </div>
  );
}
