import { useSession } from "../state/sessionStore";
import type { Timing } from "../lib/api";

/** Direction arrow for each timing verdict, so the marker reads at a glance. */
const TIMING_MARK: Record<Timing, string> = {
  Ahead: "◀ early",
  OnTime: "● on time",
  Behind: "▶ late",
};

/**
 * The practice readout: the engine's running accuracy and timing tallies, plus a marker for the
 * most recent note. Shown only once a note has been judged. Rendering only — every number comes
 * from the engine, which owns the scoring.
 */
export function PracticeStatsBar() {
  const stats = useSession((state) => state.stats);
  const lastTiming = useSession((state) => state.lastTiming);
  const lastOffsetMs = useSession((state) => state.lastOffsetMs);

  if (!stats || stats.attempts === 0) {
    return null;
  }

  const offset = Math.round(stats.average_abs_offset_ms);
  const last =
    lastTiming && lastOffsetMs !== null
      ? `${lastOffsetMs > 0 ? "+" : ""}${Math.round(lastOffsetMs)} ms`
      : "";

  return (
    <footer className="practice-stats" aria-live="polite">
      <span className="stats-accuracy">
        {Math.round(stats.accuracy * 100)}%
      </span>
      <span>
        {stats.correct} correct / {stats.wrong} wrong
      </span>
      <span>
        early {stats.ahead} · on time {stats.on_time} · late {stats.behind}
      </span>
      <span>avg {offset} ms from the beat</span>
      {lastTiming ? (
        <span className={`stats-last ${lastTiming.toLowerCase()}`}>
          {TIMING_MARK[lastTiming]} {last}
        </span>
      ) : null}
    </footer>
  );
}
