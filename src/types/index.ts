export interface Settings {
  version: number; speechProvider: string; refinementProvider: string;
  speechModels: Record<string, string>; refinementModels: Record<string, string>;
  customEndpoint: string | null; customModel: string | null;
  mode: TranscriptionMode; language: string; vocabulary: string[];
  refinementEnabled: boolean; styleId: string; customStyles: RefinementStyle[];
  shortcuts: ShortcutConfig; audio: AudioConfig; history: HistoryConfig;
  launchAtStartup: boolean; theme: string; preserveClipboard: boolean;
}
export type TranscriptionMode = "raw" | "refined";
export interface RefinementStyle { id: string; name: string; systemPrompt: string; builtin: boolean; }
export interface ShortcutConfig { dictation: string; cancel: string; openApp: string; }
export interface AudioConfig { device: string; sampleRate: number; silenceThreshold: number; prerollMs: number; }
export interface HistoryConfig { enabled: boolean; maxEntries: number; }
export interface RecordingResult { id: string; path: string; durationMs: number; sampleRate: number; createdAt: string; }
export type RecordingStatus = "idle" | "recording" | "paused" | "saving";
export interface RecordingState { status: RecordingStatus; durationMs: number; rms: number; peak: number; speaking: boolean; }
