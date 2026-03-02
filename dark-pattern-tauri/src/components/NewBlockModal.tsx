import { useState } from "react";
import { Plus, Settings, Gem, X } from "lucide-react";

// ─────────────────────────────────────────────────────────────────────────────
// Data passed back to App.tsx on save
// ─────────────────────────────────────────────────────────────────────────────

export interface NewBlockData {
  name: string;
  websites: string[];
  apps: string[];
  domRules: { site: string; toggle: string }[];
}

interface NewBlockModalProps {
  open: boolean;
  onClose: () => void;
  onSave: (data: NewBlockData) => void;
}

// ─────────────────────────────────────────────────────────────────────────────
// Tabs
// ─────────────────────────────────────────────────────────────────────────────

const tabs = ["Websites", "DOM Rules", "Apps"] as const;
type Tab = (typeof tabs)[number];

// ─────────────────────────────────────────────────────────────────────────────
// DOM rule definitions — `key` maps to the daemon toggle name
// ─────────────────────────────────────────────────────────────────────────────

interface DomRuleDef {
  key: string;      // camelCase toggle name sent to daemon
  label: string;    // human-readable label
  enabled: boolean;
  premium?: boolean;
}

interface SiteEntry {
  name: string;     // display name
  siteKey: string;  // lowercase key sent to daemon (e.g. "facebook")
  color: string;
  icon: string;
  rules: DomRuleDef[];
}

const initialSites: SiteEntry[] = [
  {
    name: "Facebook", siteKey: "facebook", color: "#3b5998", icon: "f",
    rules: [
      { key: "enabled",          label: "Block Facebook",              enabled: false },
      { key: "hideNewsfeed",     label: "Hide Feed",                   enabled: false },
      { key: "hideStories",      label: "Hide Stories",                enabled: false },
      { key: "hideReels",        label: "Hide Reels & Video",          enabled: false },
      { key: "hideMarketplace",  label: "Hide Marketplace",            enabled: false },
      { key: "hideSponsored",    label: "Hide Sponsored Posts",        enabled: false },
      { key: "hideSuggested",    label: "Hide Suggested",              enabled: false },
      { key: "hideMessengerBtn", label: "Hide Messenger Button",       enabled: false },
      { key: "hideNotifications",label: "Hide Notifications",          enabled: false },
      { key: "hideMetaAI",       label: "Hide Meta AI",                enabled: false },
    ],
  },
  {
    name: "YouTube", siteKey: "youtube", color: "#ff0000", icon: "Y",
    rules: [
      { key: "enabled",              label: "Block YouTube",            enabled: false },
      { key: "hideHomeFeed",         label: "Hide Home Feed",           enabled: false },
      { key: "hideShorts",           label: "Hide Shorts",              enabled: false },
      { key: "hideRecommendations",  label: "Hide Recommendations",    enabled: false },
      { key: "hideComments",         label: "Hide Comments",            enabled: false },
      { key: "disableAutoplay",      label: "Disable Autoplay",        enabled: false },
      { key: "hideEndCards",         label: "Hide End Cards",           enabled: false },
      { key: "grayscale",            label: "Grayscale Mode",           enabled: false, premium: true },
    ],
  },
  {
    name: "Twitter", siteKey: "twitter", color: "#1da1f2", icon: "T",
    rules: [
      { key: "enabled",          label: "Block Twitter",               enabled: false },
      { key: "hideForYou",       label: "Hide For You Feed",           enabled: false },
      { key: "hideTrends",       label: "Hide Trends",                 enabled: false },
      { key: "hideWhoToFollow",  label: "Hide Who to Follow",          enabled: false },
      { key: "hidePromoted",     label: "Hide Promoted Posts",         enabled: false },
    ],
  },
  {
    name: "Reddit", siteKey: "reddit", color: "#ff4500", icon: "R",
    rules: [
      { key: "enabled",          label: "Block Reddit",                enabled: false },
      { key: "hideHomeFeed",     label: "Hide Home Feed",              enabled: false },
      { key: "hidePopular",      label: "Hide Popular",                enabled: false },
      { key: "hideLeftSidebar",  label: "Hide Left Sidebar",           enabled: false },
      { key: "hideRightSidebar", label: "Hide Right Sidebar",          enabled: false },
      { key: "hideComments",     label: "Hide Comments",               enabled: false },
      { key: "hideUpvoteCount",  label: "Hide Upvote Counts",          enabled: false },
      { key: "hideTrending",     label: "Hide Trending",               enabled: false },
    ],
  },
  {
    name: "Netflix", siteKey: "netflix", color: "#e50914", icon: "N",
    rules: [
      { key: "enabled",                   label: "Block Netflix",              enabled: false },
      { key: "hideContinueWatching",      label: "Hide Continue Watching",     enabled: false },
      { key: "disableAutoplayPreviews",   label: "Disable Autoplay Previews",  enabled: false },
      { key: "hideMoreLikeThis",          label: "Hide More Like This",        enabled: false },
    ],
  },
  {
    name: "LinkedIn", siteKey: "linkedin", color: "#0077b5", icon: "in",
    rules: [
      { key: "enabled",                label: "Block LinkedIn",              enabled: false },
      { key: "hideFeed",               label: "Hide Feed",                   enabled: false },
      { key: "hideNotificationBadge",  label: "Hide Notification Badge",     enabled: false },
      { key: "hidePeopleYouMayKnow",   label: "Hide People You May Know",   enabled: false },
      { key: "hidePromoted",           label: "Hide Promoted Posts",         enabled: false },
    ],
  },
  {
    name: "Instagram", siteKey: "instagram", color: "#e1306c", icon: "IG",
    rules: [
      { key: "enabled",           label: "Block Instagram",            enabled: false },
      { key: "hideExplore",       label: "Hide Explore",               enabled: false },
      { key: "hideStories",       label: "Hide Stories",               enabled: false },
      { key: "hideReels",         label: "Hide Reels",                 enabled: false },
      { key: "hideSuggested",     label: "Hide Suggested Posts",       enabled: false },
      { key: "hideVanityMetrics", label: "Hide Vanity Metrics",        enabled: false, premium: true },
      { key: "grayscale",         label: "Grayscale Mode",             enabled: false, premium: true },
    ],
  },
  {
    name: "Pinterest", siteKey: "pinterest", color: "#bd081c", icon: "P",
    rules: [
      { key: "enabled",       label: "Block Pinterest",        enabled: false },
      { key: "hideHomeFeed",  label: "Hide Home Feed",          enabled: false },
      { key: "hideTodayTab",  label: "Hide Today Tab",          enabled: false },
    ],
  },
];

// ─────────────────────────────────────────────────────────────────────────────
// Component
// ─────────────────────────────────────────────────────────────────────────────

export function NewBlockModal({ open, onClose, onSave }: NewBlockModalProps) {
  const [blockName, setBlockName] = useState("");
  const [activeTab, setActiveTab] = useState<Tab>("Websites");
  // Separate state for Websites vs Apps entries
  const [websiteUrl, setWebsiteUrl] = useState("");
  const [websiteEntries, setWebsiteEntries] = useState<string[]>([]);
  const [appId, setAppId] = useState("");
  const [appEntries, setAppEntries] = useState<string[]>([]);
  // DOM rules
  const [sites, setSites] = useState<SiteEntry[]>(initialSites);
  const [activeSite, setActiveSite] = useState(0);
  const [nameError, setNameError] = useState("");

  if (!open) return null;

  // ── Websites ──────────────────────────────────────────────────────────────

  const handleAddWebsite = () => {
    const trimmed = websiteUrl.trim();
    if (trimmed && !websiteEntries.includes(trimmed)) {
      setWebsiteEntries((prev) => [...prev, trimmed]);
      setWebsiteUrl("");
    }
  };

  const handleRemoveWebsite = (idx: number) => {
    setWebsiteEntries((prev) => prev.filter((_, i) => i !== idx));
  };

  // ── Apps ──────────────────────────────────────────────────────────────────

  const handleAddApp = () => {
    const trimmed = appId.trim();
    if (trimmed && !appEntries.includes(trimmed)) {
      setAppEntries((prev) => [...prev, trimmed]);
      setAppId("");
    }
  };

  const handleRemoveApp = (idx: number) => {
    setAppEntries((prev) => prev.filter((_, i) => i !== idx));
  };

  // ── DOM rules ─────────────────────────────────────────────────────────────

  const toggleRule = (siteIdx: number, ruleIdx: number) => {
    setSites((prev) =>
      prev.map((site, si) =>
        si === siteIdx
          ? {
              ...site,
              rules: site.rules.map((rule, ri) =>
                ri === ruleIdx ? { ...rule, enabled: !rule.enabled } : rule
              ),
            }
          : site
      )
    );
  };

  // ── Save ──────────────────────────────────────────────────────────────────

  const handleSave = () => {
    const name = blockName.trim();
    if (!name) {
      setNameError("Enter a name for this block");
      return;
    }
    setNameError("");

    // Collect enabled DOM rules
    const domRules: { site: string; toggle: string }[] = [];
    for (const site of sites) {
      for (const rule of site.rules) {
        if (rule.enabled && !rule.premium) {
          domRules.push({ site: site.siteKey, toggle: rule.key });
        }
      }
    }

    onSave({
      name,
      websites: websiteEntries,
      apps: appEntries,
      domRules,
    });

    // Reset state
    resetState();
  };

  const handleClose = () => {
    resetState();
    onClose();
  };

  const resetState = () => {
    setBlockName("");
    setWebsiteUrl("");
    setWebsiteEntries([]);
    setAppId("");
    setAppEntries([]);
    setSites(initialSites);
    setActiveSite(0);
    setActiveTab("Websites");
    setNameError("");
  };

  // ── Render: Websites tab ──────────────────────────────────────────────────

  const renderWebsites = () => (
    <>
      <div className="flex items-center gap-3">
        <input
          type="text"
          placeholder="Enter domain, e.g. reddit.com"
          value={websiteUrl}
          onChange={(e) => setWebsiteUrl(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && handleAddWebsite()}
          className="flex-1 h-10 px-4 rounded-md text-sm text-white placeholder-white/40 outline-none"
          style={{ backgroundColor: "#1a1a2e", border: "1px solid #3a3a5c" }}
        />
        <button
          onClick={handleAddWebsite}
          className="h-10 px-4 rounded-md flex items-center gap-1.5 cursor-pointer text-sm text-white"
          style={{ backgroundColor: "#6366f1" }}
        >
          <Plus className="w-4 h-4" />
          Add
        </button>
      </div>

      <div
        className="rounded-md min-h-[200px] max-h-[280px] overflow-y-auto p-3 flex flex-col gap-1"
        style={{ backgroundColor: "#1a1a2e", border: "1px solid #3a3a5c" }}
      >
        {websiteEntries.length === 0 ? (
          <div className="flex-1 flex items-center justify-center text-white/30 text-sm">
            No websites added yet
          </div>
        ) : (
          websiteEntries.map((entry, i) => (
            <div
              key={i}
              className="px-3 py-2 rounded text-sm text-white/80 flex items-center justify-between"
              style={{ backgroundColor: "#252542" }}
            >
              <span>{entry}</span>
              <button
                onClick={() => handleRemoveWebsite(i)}
                className="text-red-400 hover:text-red-300 cursor-pointer"
              >
                <X className="w-4 h-4" />
              </button>
            </div>
          ))
        )}
      </div>
    </>
  );

  // ── Render: Apps tab ──────────────────────────────────────────────────────

  const renderApps = () => (
    <>
      <div className="flex items-center gap-3">
        <input
          type="text"
          placeholder="App identifier, e.g. com.apple.Safari"
          value={appId}
          onChange={(e) => setAppId(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && handleAddApp()}
          className="flex-1 h-10 px-4 rounded-md text-sm text-white placeholder-white/40 outline-none"
          style={{ backgroundColor: "#1a1a2e", border: "1px solid #3a3a5c" }}
        />
        <button
          onClick={handleAddApp}
          className="h-10 px-4 rounded-md flex items-center gap-1.5 cursor-pointer text-sm text-white"
          style={{ backgroundColor: "#6366f1" }}
        >
          <Plus className="w-4 h-4" />
          Add
        </button>
      </div>

      <div
        className="rounded-md min-h-[200px] max-h-[280px] overflow-y-auto p-3 flex flex-col gap-1"
        style={{ backgroundColor: "#1a1a2e", border: "1px solid #3a3a5c" }}
      >
        {appEntries.length === 0 ? (
          <div className="flex-1 flex items-center justify-center text-white/30 text-sm">
            No apps added yet
          </div>
        ) : (
          appEntries.map((entry, i) => (
            <div
              key={i}
              className="px-3 py-2 rounded text-sm text-white/80 flex items-center justify-between"
              style={{ backgroundColor: "#252542" }}
            >
              <span>{entry}</span>
              <button
                onClick={() => handleRemoveApp(i)}
                className="text-red-400 hover:text-red-300 cursor-pointer"
              >
                <X className="w-4 h-4" />
              </button>
            </div>
          ))
        )}
      </div>
    </>
  );

  // ── Render: DOM Rules tab ─────────────────────────────────────────────────

  const currentSite = sites[activeSite];

  const renderDomRules = () => (
    <div
      className="flex rounded-md overflow-hidden"
      style={{
        backgroundColor: "#1a1a2e",
        border: "1px solid #3a3a5c",
        minHeight: 380,
      }}
    >
      {/* Left sidebar – site list */}
      <div
        className="w-[180px] flex flex-col shrink-0 overflow-y-auto"
        style={{ borderRight: "1px solid #3a3a5c" }}
      >
        {sites.map((site, idx) => {
          const enabledCount = site.rules.filter((r) => r.enabled && !r.premium).length;
          return (
            <button
              key={site.name}
              onClick={() => setActiveSite(idx)}
              className="flex items-center gap-3 px-4 py-3 cursor-pointer text-sm text-left transition-colors"
              style={{
                backgroundColor: activeSite === idx ? "#252542" : "transparent",
                color: activeSite === idx ? "#818cf8" : "rgba(255,255,255,0.7)",
                borderBottom: "1px solid #2a2a48",
              }}
            >
              <span
                className="w-8 h-8 rounded-full flex items-center justify-center text-white text-xs shrink-0 relative"
                style={{ backgroundColor: site.color }}
              >
                {site.icon}
                {enabledCount > 0 && (
                  <span
                    className="absolute -top-1 -right-1 w-4 h-4 rounded-full flex items-center justify-center text-[10px] text-white"
                    style={{ backgroundColor: "#6366f1" }}
                  >
                    {enabledCount}
                  </span>
                )}
              </span>
              {site.name}
            </button>
          );
        })}
        <button
          className="flex items-center gap-3 px-4 py-3 cursor-pointer text-sm text-left text-white/70 hover:text-white transition-colors mt-auto"
          style={{ borderTop: "1px solid #2a2a48" }}
        >
          <span
            className="w-8 h-8 rounded-full flex items-center justify-center shrink-0"
            style={{ backgroundColor: "#4a4a6a" }}
          >
            <Settings className="w-4 h-4 text-white" />
          </span>
          Settings
        </button>
      </div>

      {/* Right panel – rules */}
      <div className="flex-1 flex flex-col overflow-y-auto">
        {currentSite.rules.map((rule, rIdx) => (
          <div
            key={rule.key}
            className="flex items-center justify-between px-5 py-3 transition-colors"
            style={{
              borderBottom: "1px solid #2a2a48",
              opacity: rule.premium ? 0.5 : 1,
            }}
          >
            <div className="flex items-center gap-2 text-sm text-white">
              {rule.premium && <Gem className="w-4 h-4 text-amber-400" />}
              {rule.label}
            </div>
            <button
              onClick={() => !rule.premium && toggleRule(activeSite, rIdx)}
              className="relative w-12 h-6 rounded-full cursor-pointer transition-colors shrink-0"
              style={{
                backgroundColor: rule.enabled ? "#6366f1" : "#3a3a5c",
                cursor: rule.premium ? "not-allowed" : "pointer",
              }}
            >
              <span
                className="absolute top-0.5 w-5 h-5 rounded-full bg-white transition-all"
                style={{
                  left: rule.enabled ? "calc(100% - 1.375rem)" : "0.125rem",
                }}
              />
            </button>
          </div>
        ))}
      </div>
    </div>
  );

  // Count total configured items for the save button
  const enabledDomCount = sites.reduce(
    (sum, s) => sum + s.rules.filter((r) => r.enabled && !r.premium).length,
    0
  );
  const totalItems = websiteEntries.length + appEntries.length + enabledDomCount;

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center"
      style={{ backgroundColor: "rgba(0,0,0,0.6)" }}
      onClick={handleClose}
    >
      <div
        className="w-full max-w-[720px] rounded-xl p-6 flex flex-col gap-5"
        style={{ backgroundColor: "#252542", border: "1px solid #3a3a5c" }}
        onClick={(e) => e.stopPropagation()}
      >
        {/* Title */}
        <h2 className="text-center text-white uppercase tracking-widest">
          What do you want to block?
        </h2>

        {/* Divider */}
        <div style={{ height: 1, backgroundColor: "#3a3a5c" }} />

        {/* Block name input */}
        <div className="flex flex-col gap-1">
          <input
            type="text"
            placeholder="Block name, e.g. Social Media, Work Focus..."
            value={blockName}
            onChange={(e) => { setBlockName(e.target.value); setNameError(""); }}
            className="h-10 px-4 rounded-md text-sm text-white placeholder-white/40 outline-none"
            style={{ backgroundColor: "#1a1a2e", border: `1px solid ${nameError ? "#f87171" : "#3a3a5c"}` }}
          />
          {nameError && <span className="text-red-400 text-xs">{nameError}</span>}
        </div>

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

        {/* Tab Content */}
        {activeTab === "Websites" && renderWebsites()}
        {activeTab === "DOM Rules" && renderDomRules()}
        {activeTab === "Apps" && renderApps()}

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
            Save{totalItems > 0 ? ` (${totalItems} rule${totalItems !== 1 ? "s" : ""})` : ""}
          </button>
        </div>
      </div>
    </div>
  );
}
