import { create } from "zustand";
import type { RecordingState } from "../types";
const IDLE: RecordingState = { status: "idle", durationMs: 0, rms: 0, peak: 0, speaking: false };
interface Store { state: RecordingState; applyState: (p: Partial<RecordingState>) => void;
  start: (d?: string) => Promise<void>; pause: () => Promise<void>; resume: () => Promise<void>; stop: () => Promise<void>; cancel: () => Promise<void>; }
export const useRecorderStore = create<Store>((set) => ({
  state: IDLE,
  applyState: (p) => set((s) => ({ state: { ...s.state, ...p } })),
  start: async (d) => { try { const { tauriInvoke } = await import("../services/tauri"); await tauriInvoke("start_recording", { device: d ?? null }); } catch (e) { console.error(e); } },
  pause: async () => { try { const { tauriInvoke } = await import("../services/tauri"); await tauriInvoke("pause_recording"); } catch (e) { console.error(e); } },
  resume: async () => { try { const { tauriInvoke } = await import("../services/tauri"); await tauriInvoke("resume_recording"); } catch (e) { console.error(e); } },
  stop: async () => { try { const { tauriInvoke } = await import("../services/tauri"); await tauriInvoke("stop_recording"); } catch (e) { console.error(e); } },
  cancel: async () => { try { const { tauriInvoke } = await import("../services/tauri"); await tauriInvoke("cancel_recording"); } catch (e) { console.error(e); } },
}));
