import styles from "./NotTauriBanner.module.css";
export function NotTauriBanner() {
  return (
    <div className={styles.banner} role="alert">
      <svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" strokeWidth="2">
        <circle cx="12" cy="12" r="10" /><line x1="12" y1="8" x2="12" y2="12" /><line x1="12" y1="16" x2="12.01" y2="16" />
      </svg>
      <span><strong>Auralis requires the desktop runtime.</strong> This UI is the web view. Run the Tauri binary instead. Press <kbd>Ctrl+Shift+Space</kbd> in the desktop app to dictate.</span>
    </div>
  );
}
