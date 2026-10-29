import { useEffect } from "react";
import { useAppStore } from "./state/appStore";
import { usePcKeyboard } from "./lib/usePcKeyboard";
import "./App.css";

function App() {
  const appInfo = useAppStore((state) => state.appInfo);
  const loadAppInfo = useAppStore((state) => state.loadAppInfo);

  usePcKeyboard();

  useEffect(() => {
    void loadAppInfo();
  }, [loadAppInfo]);

  return (
    <div className="app">
      <header className="toolbar">
        <span className="app-title">MidiTrainer</span>
        {appInfo && <span className="version-badge">v{appInfo.version}</span>}
      </header>
      <main className="workspace">
        <section className="info-card">
          <h2>Application</h2>
          <dl>
            <div className="info-row">
              <dt>Version</dt>
              <dd>{appInfo?.version ?? "loading…"}</dd>
            </div>
            <div className="info-row">
              <dt>Operating system</dt>
              <dd>{appInfo?.os ?? "loading…"}</dd>
            </div>
            <div className="info-row">
              <dt>Architecture</dt>
              <dd>{appInfo?.arch ?? "loading…"}</dd>
            </div>
          </dl>
        </section>
      </main>
    </div>
  );
}

export default App;
