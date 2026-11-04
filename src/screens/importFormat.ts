export interface ImportItem {
  id: string;
  title: string;
  category: string;
  status: "new" | "needs_exe" | "done" | "conflict" | "blocked";
  problem: string | null;
  size: number;
  pending_bytes: number;
  exe: string | null;
  exe_options: string[];
}

export interface DrivePlan {
  drive: string;
  items: ImportItem[];
  free_bytes: number | null;
  library_dir: string;
}

export interface ImportProgress {
  done_bytes: number;
  total_bytes: number;
  bytes_per_sec: number;
  current: string;
  item_index: number;
  item_count: number;
  finished: boolean;
  cancelled: boolean;
  imported: string[];
  errors: string[];
}

export interface GameFilesEntry {
  name: string;
  path: string;
  size: number;
  file_count: number;
  main_files: string[];
}

const foldName = (text: string) => text.normalize("NFD").replace(/\p{Diacritic}/gu, "").trim().toLowerCase();

/** Arquivos cuja pasta tem o mesmo nome do jogo, ignorando maiúsculas e acentos. */
export const filesForGame = (title: string, entries: GameFilesEntry[]) =>
  entries.find((entry) => foldName(entry.name) === foldName(title)) ?? null;

export const isImportable =(item: ImportItem) => item.status === "new" || item.status === "needs_exe";

export function formatBytes(bytes: number): string {
  const units = ["B", "KB", "MB", "GB", "TB"];
  let value = bytes;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit++;
  }
  const text = unit === 0 || value >= 100 ? Math.round(value).toString() : value.toFixed(1);
  return `${text.replace(".", ",")} ${units[unit]}`;
}

export function formatEta(remainingBytes: number, bytesPerSec: number): string {
  if (bytesPerSec <= 0) return "calculando…";
  const secs = Math.ceil(remainingBytes / bytesPerSec);
  if (secs < 60) return `${secs} s`;
  const minutes = Math.ceil(secs / 60);
  if (minutes < 60) return `${minutes} min`;
  return `${Math.floor(minutes / 60)} h ${minutes % 60} min`;
}

export function statusLabel(item: ImportItem): string {
  switch (item.status) {
    case "new": return formatBytes(item.pending_bytes);
    case "needs_exe": return `${formatBytes(item.pending_bytes)} · escolher .exe`;
    case "done": return "Já importado";
    case "conflict": return "Conflito";
    case "blocked": return "Não pode ser importado";
  }
}

export const selectedBytes = (items: ImportItem[], selected: Set<string>) =>
  items.filter((i) => selected.has(i.id)).reduce((sum, i) => sum + i.pending_bytes, 0);
