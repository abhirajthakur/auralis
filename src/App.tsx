import { useState } from "react";
import { Sidebar } from "./components/Sidebar";
import { NotTauriBanner } from "./components/NotTauriBanner";
import { HomePage } from "./pages/Home";
import { ProvidersPage } from "./pages/Providers";
import { DictationPage } from "./pages/Dictation";
import { RefinementPage } from "./pages/Refinement";
import { HistoryPage } from "./pages/History";
import { SettingsPage } from "./pages/Settings";
import { isTauri } from "./services/tauri";
export type Page = "home" | "providers" | "dictation" | "refinement" | "history" | "settings";
export default function App() {
  const [page, setPage] = useState<Page>("home");
  return (
    <div className="flex h-full">
      {!isTauri() && <NotTauriBanner />}
      <Sidebar page={page} onNavigate={setPage} />
      <main className="flex-1 overflow-auto">
        {page === "home" && <HomePage />}
        {page === "providers" && <ProvidersPage />}
        {page === "dictation" && <DictationPage />}
        {page === "refinement" && <RefinementPage />}
        {page === "history" && <HistoryPage />}
        {page === "settings" && <SettingsPage />}
      </main>
    </div>
  );
}
