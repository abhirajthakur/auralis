import { useEffect, useState } from "react";
import { Mic, Square, RotateCcw, History, Copy, Check, FileAudio, ChevronRight, Keyboard } from "lucide-react";
import { useRecorderStore } from "../stores/recorderStore";
import { useSettingsStore } from "../stores/settingsStore";
import type { RecordingResult } from "../types";
function fmt(ms: number) { const s = Math.floor(ms / 1000); return `${Math.floor(s/60)}:${(s%60).toString().padStart(2,"0")}`; }
export function HomePage() {
  const { state, start, stop, cancel } = useRecorderStore();
  const { settings, loaded } = useSettingsStore();
  const [recordings, setRecordings] = useState<RecordingResult[]>([]);
  const [lastText, setLastText] = useState("");
  const [copied, setCopied] = useState(false);
  const isRecording = state.status === "recording";
  const isPaused = state.status === "paused";
  const isIdle = state.status === "idle";
  useEffect(() => { if (!loaded) return; useSettingsStore.getState().load(); }, [loaded]);
  useEffect(() => {
    if (!loaded) return;
    import("@tauri-apps/api/event").then(({ listen }) => {
      listen<{ status: string; durationMs: number }>("recording//state", (e) => {
        useRecorderStore.getState().applyState({ status: e.payload.status as any, durationMs: e.payload.durationMs });
      }).then(u => u);
    }).catch(() => {});
  }, [loaded]);
  const copyLast = () => { if (!lastText) return; navigator.clipboard.writeText(lastText); setCopied(true); setTimeout(() => setCopied(false), 2000); };
  return (
    <div className="p-8 flex flex-col gap-8 max-w-3xl mx-auto">
      <div className="text-center">
        <h1 className="text-3xl font-semibold tracking-tight mb-2">Your voice, refined.</h1>
        <p className="text-sm text-[var(--color-text-secondary)]">Press <kbd className="text-xs mx-1 px-1.5 py-0.5 rounded border border-[var(--color-border)] font-mono">Ctrl+Shift+Space</kbd> to dictate.</p>
      </div>
      <div className="flex flex-col items-center gap-4">
        <button className={`relative flex items-center justify-center rounded-full w-32 h-32 transition-all duration-200 ${isRecording ? "bg-[var(--color-error)] shadow-[0_0_40px_rgba(252,165,165,0.35)]" : "bg-[var(--color-accent)] shadow-[0_0_40px_rgba(167,139,250,0.3)]"}`} onClick={isIdle ? start : stop} aria-label={isRecording ? "Stop" : "Start"}>
          {isRecording ? <Square size={36} fill="currentColor" /> : <Mic size={40} fill="currentColor" />}
          {isRecording && <span className="absolute inset-0 rounded-full animate-ping opacity-20 bg-[var(--color-error)]" />}
        </button>
        <p className="text-sm font-medium">{isIdle && "Ready"}{isRecording && <span className="text-[var(--color-error)]">Recording · {fmt(state.durationMs)}</span>}{isPaused && "Paused"}{state.status==="saving"&&"Saving…"}</p>
        {(isRecording||isPaused) && <div className="w-48 h-1.5 bg-[var(--color-bg-muted)] rounded-full overflow-hidden"><div className="h-full rounded-full transition-all" style={{width:`${Math.min(state.rms*100,100)}%`,background:"linear-gradient(90deg,var(--color-accent),var(--color-accent-hover))"}}/></div>}
        {(isRecording||isPaused) && <button className="flex items-center gap-2 px-4 py-2 rounded-lg bg-[var(--color-bg-surface)] border border-[var(--color-border)] text-sm hover:bg-[var(--color-bg-muted)]" onClick={cancel}><RotateCcw size={14}/>Cancel</button>}
      </div>
      {lastText && <div className="bg-[var(--color-bg-surface)] border border-[var(--color-border)] rounded-xl p-4"><div className="flex items-center justify-between mb-2"><span className="text-xs text-[var(--color-text-muted)] uppercase tracking-wide">Last result</span><button className="flex items-center gap-1.5 text-xs text-[var(--color-accent)]" onClick={copyLast}>{copied?<Check size={13}/>:<Copy size={13}/>}{copied?"Copied!":"Copy"}</button></div><p className="text-sm leading-relaxed">{lastText}</p></div>}
      <div><h2 className="text-sm font-medium text-[var(--color-text-secondary)] mb-3 flex items-center gap-2"><History size={14}/>Recent recordings</h2>
        {recordings.length===0 ? <p className="text-sm text-[var(--color-text-muted)]">No recordings yet.</p> : <ul className="flex flex-col gap-2">{recordings.slice(0,5).map(r=>(<li key={r.id} className="flex items-center gap-3 px-4 py-3 bg-[var(--color-bg-surface)] border border-[var(--color-border)] rounded-lg"><FileAudio size={16} className="text-[var(--color-accent)]"/><div className="flex-1"><p className="text-xs text-[var(--color-text-muted)]">{new Date(r.createdAt).toLocaleTimeString()} · {fmt(r.durationMs)}</p></div></li>))}</ul>}
      </div>
      <div className="flex items-center gap-2 text-xs text-[var(--color-text-muted)]"><Keyboard size={12}/><span>Speech: {loaded?settings.speechProvider:"…"} · Refinement: {loaded?(settings.refinementEnabled?settings.styleId:"off"):"…"}</span><ChevronRight size={12} className="ml-auto opacity-50"/></div>
    </div>
  );
}
