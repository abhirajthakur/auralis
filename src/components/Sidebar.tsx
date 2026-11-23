import { Mic, Settings, History, Sliders, Globe, FileText } from "lucide-react";
import type { Page } from "../App";
import styles from "./Sidebar.module.css";
const NAV: { id: Page; icon: React.ReactNode; label: string }[] = [
  { id: "home", icon: <Mic size={20} />, label: "Home" },
  { id: "providers", icon: <Globe size={20} />, label: "Providers" },
  { id: "dictation", icon: <FileText size={20} />, label: "Dictation" },
  { id: "refinement", icon: <Sliders size={20} />, label: "Refinement" },
  { id: "history", icon: <History size={20} />, label: "History" },
  { id: "settings", icon: <Settings size={20} />, label: "Settings" },
];
export function Sidebar({ page, onNavigate }: { page: Page; onNavigate: (p: Page) => void }) {
  return (
    <nav className={styles.sidebar}>
      <div className={styles.logo}>
        <svg viewBox="0 0 512 512" width="36" height="36" aria-label="Auralis">
          <g stroke="#A78BFA" strokeWidth="42" strokeLinecap="round" fill="none">
            <path d="M 256 122 L 156 392" /><path d="M 256 122 L 356 392" />
          </g>
          <g fill="#C4B5FD">
            <rect x="202" y="282" width="13" height="40" rx="6.5" />
            <rect x="229" y="266" width="13" height="72" rx="6.5" />
            <rect x="256" y="248" width="13" height="108" rx="6.5" />
            <rect x="283" y="266" width="13" height="72" rx="6.5" />
            <rect x="310" y="282" width="13" height="40" rx="6.5" />
          </g>
        </svg>
        <span className={styles.wordmark}>auralis</span>
      </div>
      <div className={styles.nav}>
        {NAV.map(({ id, icon, label }) => (
          <button key={id} className={`${styles.navItem} ${page === id ? styles.active : ""}`}
            onClick={() => onNavigate(id)} aria-current={page === id ? "page" : undefined}>
            {icon}<span>{label}</span>
          </button>
        ))}
      </div>
      <footer className={styles.footer}><span className={styles.version}>v0.1.0</span></footer>
    </nav>
  );
}
