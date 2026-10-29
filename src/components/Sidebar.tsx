import { useAppearance } from "../lib/theme";

/** Which top-level screen is shown: the file Library, or the Practice area. */
export type Section = "library" | "practice";

/** The top-level navigation entries, in order. */
const SECTIONS: { id: Section; label: string; icon: string }[] = [
  { id: "library", label: "Library", icon: "♪" },
  { id: "practice", label: "Practice", icon: "🎹" },
];

/**
 * The left navigation rail: the brand (with the collapse tab), one button per top-level screen,
 * and the app settings pinned to the foot. The active section lives in the app shell; the
 * collapsed state and its control live in the appearance store, so it is remembered.
 */
export function Sidebar({
  section,
  onSelect,
  onOpenSettings,
}: {
  section: Section;
  onSelect: (section: Section) => void;
  onOpenSettings: () => void;
}) {
  const collapsed = useAppearance((state) => state.collapsed);
  const toggleSidebar = useAppearance((state) => state.toggleSidebar);

  return (
    <aside className={collapsed ? "sidebar collapsed" : "sidebar"}>
      <div className="sidebar-brand">
        <span className="brand-mark">♬</span>
        <span className="brand-name">MidiTrainer</span>
        <button
          type="button"
          className="sidebar-tab"
          aria-label={collapsed ? "Expand menu" : "Collapse menu"}
          aria-pressed={collapsed}
          title={collapsed ? "Expand menu" : "Collapse menu"}
          onClick={toggleSidebar}
        >
          {collapsed ? "»" : "«"}
        </button>
      </div>
      <nav className="sidebar-nav">
        {SECTIONS.map((item) => (
          <button
            key={item.id}
            type="button"
            className={section === item.id ? "nav-item active" : "nav-item"}
            aria-current={section === item.id ? "page" : undefined}
            aria-label={item.label}
            title={item.label}
            onClick={() => onSelect(item.id)}
          >
            <span className="nav-icon" aria-hidden="true">
              {item.icon}
            </span>
            <span className="nav-label">{item.label}</span>
          </button>
        ))}
      </nav>
      <button
        type="button"
        className="nav-item sidebar-settings"
        aria-label="App settings"
        title="App settings"
        onClick={onOpenSettings}
      >
        <span className="nav-icon" aria-hidden="true">
          ⚙
        </span>
        <span className="nav-label">Settings</span>
      </button>
    </aside>
  );
}
