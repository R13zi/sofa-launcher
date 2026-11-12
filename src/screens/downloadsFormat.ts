import { Download, sourceLabel } from "../api";
import { formatSize } from "./updateFormat";

/** Sem progresso por este tempo, o download é mostrado como parado. */
export const STALL_MS = 2 * 60 * 1000;
/** Janela usada para medir a velocidade. */
const SPEED_WINDOW_MS = 20 * 1000;

interface Sample {
  t: number;
  done: number;
}

/** Guarda as últimas leituras de cada download, para medir velocidade e ver se parou. */
export class ProgressTracker {
  private samples = new Map<string, Sample[]>();
  private changedAt = new Map<string, number>();

  record(list: Download[], now: number): void {
    const seen = new Set<string>();
    for (const d of list) {
      seen.add(d.id);
      const history = this.samples.get(d.id) ?? [];
      const last = history[history.length - 1];
      if (!last || last.done !== d.done || !this.changedAt.has(d.id)) this.changedAt.set(d.id, now);
      history.push({ t: now, done: d.done });
      while (history.length > 2 && now - history[0].t > SPEED_WINDOW_MS) history.shift();
      this.samples.set(d.id, history);
    }
    for (const id of [...this.samples.keys()]) {
      if (!seen.has(id)) {
        this.samples.delete(id);
        this.changedAt.delete(id);
      }
    }
  }

  /** Bytes por segundo; nulo enquanto não há leituras suficientes. */
  speed(id: string): number | null {
    const history = this.samples.get(id);
    if (!history || history.length < 2) return null;
    const first = history[0];
    const last = history[history.length - 1];
    const seconds = (last.t - first.t) / 1000;
    return seconds > 0 ? Math.max(0, (last.done - first.done) / seconds) : null;
  }

  /** Há quanto tempo o progresso não muda. */
  idleFor(id: string, now: number): number {
    const at = this.changedAt.get(id);
    return at === undefined ? 0 : now - at;
  }
}

export type Problem = { kind: "no_space"; missing: number } | { kind: "stalled"; minutes: number } | null;

const remaining = (d: Download) => Math.max(0, d.total - d.done);

/** Falta espaço (o que resta baixar não cabe no disco) ou o progresso parou. */
export function problemOf(d: Download, idleMs: number): Problem {
  if (d.status !== "active") return null;
  if (d.total > 0 && d.free !== null && d.free < remaining(d)) {
    return { kind: "no_space", missing: remaining(d) - d.free };
  }
  const finishing = d.total > 0 && d.done >= d.total;
  if (!finishing && idleMs >= STALL_MS) return { kind: "stalled", minutes: Math.floor(idleMs / 60000) };
  return null;
}

export function formatSpeed(bytesPerSecond: number): string {
  const mb = bytesPerSecond / (1024 * 1024);
  return mb >= 1 ? `${mb.toLocaleString("pt-BR", { maximumFractionDigits: mb >= 10 ? 0 : 1 })} MB/s` : `${Math.round(bytesPerSecond / 1024)} KB/s`;
}

export function formatEta(seconds: number): string {
  if (seconds < 60) return "menos de 1 min";
  const minutes = Math.round(seconds / 60);
  if (minutes < 60) return `cerca de ${minutes} min`;
  const hours = Math.floor(minutes / 60);
  const rest = minutes % 60;
  return rest ? `cerca de ${hours} h ${rest} min` : `cerca de ${hours} h`;
}

/** `C:\` vira `C:`. */
export const driveLabel = (drive: string) => drive.replace(/\\$/, "");

/** De 0 a 1, ou nulo quando a loja ainda não informou o tamanho. */
export const fraction = (d: Download) => (d.total > 0 ? Math.min(1, d.done / d.total) : null);

/** Linha de detalhes: loja, tipo, quanto já veio, velocidade e tempo restante. */
export function describeDownload(d: Download, speed: number | null, problem: Problem): string {
  const parts = [sourceLabel(d.source), d.kind === "install" ? "Instalação" : "Atualização"];
  if (d.status === "queued") {
    parts.push(d.total > 0 ? formatSize(d.total) : "aguardando");
    return parts.join(" · ");
  }
  if (problem?.kind === "no_space") {
    return `${parts[0]} · ${d.drive ? driveLabel(d.drive) : "O disco"} encheu durante o download. Libere mais ${formatSize(problem.missing)} para continuar.`;
  }
  if (problem?.kind === "stalled") {
    return `${parts.join(" · ")} · sem progresso há ${problem.minutes} min. A loja pode ter pausado o download.`;
  }
  if (d.total > 0 && d.done >= d.total) {
    parts.push("finalizando a instalação");
    return parts.join(" · ");
  }
  if (d.total > 0) parts.push(`${formatSize(d.done)} de ${formatSize(d.total)}`);
  else parts.push("preparando");
  if (speed !== null && speed > 0) {
    parts.push(formatSpeed(speed));
    if (d.total > 0) parts.push(formatEta(remaining(d) / speed));
  }
  return parts.join(" · ");
}

export interface Completed {
  id: string;
  title: string;
  /** Hora em que saiu da lista, em ms. */
  at: number;
}

/** Downloads ativos que sumiram da lista desde a última leitura terminaram. */
export function finished(previous: Download[], current: Download[], now: number): Completed[] {
  const still = new Set(current.map((d) => d.id));
  return previous.filter((d) => d.status === "active" && !still.has(d.id)).map((d) => ({ id: d.id, title: d.title, at: now }));
}

export const sameDay = (a: number, b: number) => new Date(a).toDateString() === new Date(b).toDateString();
