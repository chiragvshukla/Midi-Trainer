/** Which top-level screen is shown: the file Library, or the Practice area. */
export type Section = "library" | "practice";

/** The top-level navigation entries, in order. */
const SECTIONS: { id: Section; label: string; icon: string }[] = [
  { id: "library", label: "Library", icon: "♪" },
  { id: "practice", label: "Practice", icon: "🎹" },
];

/**
 * The left navigation rail: the brand, then one button per top-level screen. Rendering only;
 * the active section lives in the app shell.
 */
export function Sidebar({
  section,
  onSelect,
  version,
}: {
  section: Section;
  onSelect: (section: Section) => void;
  version: string | null;
}) {
  return (
    <aside className="sidebar">
      <div className="sidebar-brand">
        <span className="brand-mark">♬</span>
        <span className="brand-name">MidiTrainer</span>
        {version ? <span className="version-badge">v{version}</span> : null}
      </div>
      <nav className="sidebar-nav">
        {SECTIONS.map((item) => (
          <button
            key={item.id}
            type="button"
            className={section === item.id ? "nav-item active" : "nav-item"}
            aria-current={section === item.id ? "page" : undefined}
            onClick={() => onSelect(item.id)}
          >
            <span className="nav-icon" aria-hidden="true">
              {item.icon}
            </span>
            {item.label}
          </button>
        ))}
      </nav>
    </aside>
  );
}
