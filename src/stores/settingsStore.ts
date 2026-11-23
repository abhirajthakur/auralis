import { create } from "zustand";
import type { Settings } from "../types";
const DEFAULT: Settings = {
  version: 1, speechProvider: "groq", refinementProvider: "groq",
  speechModels: { groq: "whisper-large-v3", openai: "gpt-4o-mini-transcribe", gemini: "gemini-2.0-flash", "local-whisper": "base.en" },
  refinementModels: { groq: "llama-3.3-70b-versatile", openai: "gpt-4o-mini", openrouter: "openai/gpt-4o-mini", gemini: "gemini-2.0-flash", anthropic: "claude-3-5-haiku-latest", "openai-compatible": "" },
  customEndpoint: null, customModel: null, mode: "raw", language: "auto", vocabulary: [],
  refinementEnabled: false, styleId: "smart", customStyles: [],
  shortcuts: { dictation: "Ctrl+Shift+Space", cancel: "Escape", openApp: "Ctrl+Shift+A" },
  audio: { device: "", sampleRate: 16000, silenceThreshold: 0.01, prerollMs: 300 },
  history: { enabled: true, maxEntries: 500 }, launchAtStartup: false, theme: "dark", preserveClipboard: true,
};
interface Store { settings: Settings; loaded: boolean; load: () => Promise<void>; save: (s: Settings) => Promise<void>; update: (p: Partial<Settings>) => Promise<void>; }
export const useSettingsStore = create<Store>((set, get) => ({
  settings: DEFAULT, loaded: false,
  load: async () => {
    try { const { tauriInvoke } = await import("../services/tauri"); const s = await tauriInvoke<Settings>("get_settings"); set({ settings: s, loaded: true }); }
    catch { set({ loaded: true }); }
  },
  save: async (s) => { try { const { tauriInvoke } = await import("../services/tauri"); await tauriInvoke("save_settings", { settings: s }); } catch (e) { console.error(e); } set({ settings: s }); },
  update: async (p) => { const next = { ...get().settings, ...p }; await get().save(next); },
}));
