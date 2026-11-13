// Self-update from the GitHub releases of this repo (desktop app only).
// The Rust side downloads the portable zip, verifies its checksum, swaps files and restarts.
// In a plain browser (development) these calls are skipped.

export type UpdateInfo = { version: string; notes: string; download_url: string; size: number; sha256: string | null };

export type UpdateState =
  | { kind: "idle" }
  | { kind: "checking" }
  | { kind: "available"; version: string; install: () => Promise<void> }
  | { kind: "installing"; percent: number }
  | { kind: "none" }
  | { kind: "error"; message: string };

const inTauri = () => typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

export async function appVersion(): Promise<string | null> {
  if (!inTauri()) return null;
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<string>("app_version");
}

export async function checkForUpdate(onProgress: (s: UpdateState) => void): Promise<UpdateState> {
  if (!inTauri()) return { kind: "none" };
  try {
    const { invoke } = await import("@tauri-apps/api/core");
    const info = await invoke<UpdateInfo | null>("check_update");
    if (!info) return { kind: "none" };
    return {
      kind: "available",
      version: info.version,
      install: async () => {
        onProgress({ kind: "installing", percent: 0 });
        try {
          await invoke("install_update", { info });
        } catch (e) {
          onProgress({ kind: "error", message: String(e) });
        }
      },
    };
  } catch (e) {
    return { kind: "error", message: String(e) };
  }
}
