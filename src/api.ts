import { mockInvoke, mockOn } from "./mock";

export const inTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

export interface GameView {
  id: string;
  title: string;
  source: string;
  art: string | null;
  hero: string | null;
  favorite: boolean;
  hidden: boolean;
  last_played: number;
  playtime_secs: number;
  launch_file: string | null;
}

export interface Emulator {
  id: string;
  name: string;
  kind: string;
  exe: string;
  args: string[];
  rom_dirs: string[];
  exts: string[];
}

export interface FsEntry {
  name: string;
  path: string;
  dir: boolean;
}

export interface SessionInfo {
  game_id: string;
  title: string;
  art: string | null;
  hero: string | null;
  phase: "launching" | "playing";
}

export interface ArtProgress {
  done: number;
  total: number;
  found: number;
  error: string | null;
  finished: boolean;
}

export async function call<T>(command: string, args: Record<string, unknown> = {}): Promise<T> {
  if (!inTauri) return mockInvoke(command, args) as T;
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<T>(command, args);
}

/** Escuta um evento dirigido a esta janela ou a todas. */
export async function on<T>(event: string, handler: (payload: T) => void): Promise<void> {
  if (!inTauri) {
    mockOn(event, handler as (payload: unknown) => void);
    return;
  }
  const { getCurrentWebviewWindow } = await import("@tauri-apps/api/webviewWindow");
  await getCurrentWebviewWindow().listen<T>(event, (e) => handler(e.payload));
}

let convert: ((path: string) => string) | null = null;
if (inTauri) {
  void import("@tauri-apps/api/core").then((core) => {
    convert = core.convertFileSrc;
  });
}

export function artUrl(path: string | null): string | null {
  if (!path) return null;
  return convert ? convert(path) : path;
}

export const SOURCE_LABELS: Record<string, string> = {
  steam: "Steam",
  epic: "Epic Games",
  manual: "Outros jogos",
  cemu: "Wii U (Cemu)",
  dolphin: "GameCube/Wii (Dolphin)",
  rpcs3: "PS3 (RPCS3)",
};

export const sourceLabel = (source: string) => SOURCE_LABELS[source] ?? source;

export function formatPlaytime(secs: number): string {
  if (secs < 60) return "Ainda não jogado";
  const hours = Math.floor(secs / 3600);
  const minutes = Math.floor((secs % 3600) / 60);
  return hours > 0 ? `${hours} h ${minutes} min jogados` : `${minutes} min jogados`;
}
