export function isTauri(): boolean {
  return !!(window as any).__TAURI_INTERNALS__;
}
export async function tauriInvoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (!isTauri()) throw new Error("Auralis requires the desktop runtime.");
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<T>(cmd, args);
}
