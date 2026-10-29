import { useEffect, useRef, useState } from "react";

/** Which top-level screen is shown; the Synth section holds the Keys and Sheet pages. */
export type Section = "library" | "synth";

/**
 * The burger menu: switches between the Library and the Synth practice area. It closes on a
 * selection, a click outside, or Escape.
 */
export function MainMenu({
  section,
  onSelect,
}: {
  section: Section;
  onSelect: (section: Section) => void;
}) {
  const [open, setOpen] = useState(false);
  const rootRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) {
      return;
    }
    const closeOnOutside = (event: MouseEvent) => {
      if (!rootRef.current?.contains(event.target as Node)) {
        setOpen(false);
      }
    };
    const closeOnEscape = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        setOpen(false);
      }
    };
    document.addEventListener("mousedown", closeOnOutside);
    document.addEventListener("keydown", closeOnEscape);
    return () => {
      document.removeEventListener("mousedown", closeOnOutside);
      document.removeEventListener("keydown", closeOnEscape);
    };
  }, [open]);

  const choose = (value: Section) => {
    onSelect(value);
    setOpen(false);
  };

  return (
    <div className="menu" ref={rootRef}>
      <button
        type="button"
        className="menu-button"
        aria-haspopup="menu"
        aria-expanded={open}
        aria-label="Menu"
        onClick={() => setOpen((current) => !current)}
      >
        ☰
      </button>
      {open && (
        <div className="menu-items" role="menu">
          <button
            type="button"
            role="menuitem"
            className={section === "library" ? "active" : ""}
            onClick={() => choose("library")}
          >
            Library
          </button>
          <button
            type="button"
            role="menuitem"
            className={section === "synth" ? "active" : ""}
            onClick={() => choose("synth")}
          >
            Synth
          </button>
        </div>
      )}
    </div>
  );
}
