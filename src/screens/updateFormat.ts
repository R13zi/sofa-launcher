import type { UpdateState } from "../api";

const GB = 1024 ** 3;
const MB = 1024 ** 2;

export function formatSize(bytes: number): string {
  if (bytes >= GB) return `${(bytes / GB).toLocaleString("pt-BR", { maximumFractionDigits: 1 })} GB`;
  return `${Math.max(1, Math.round(bytes / MB))} MB`;
}

/** De 0 a 1, ou `null` quando a Steam ainda não informou o tamanho. */
export function updateProgress(update: UpdateState | null): number | null {
  if (update?.state !== "downloading" || update.total <= 0) return null;
  return Math.min(1, update.done / update.total);
}

/** Se há algo para baixar: vale para o selo do cartão e para o "Atualizar e jogar". */
export const needsUpdate = (update: UpdateState | null) => update?.state === "pending" || update?.state === "downloading";

/** Texto da linha de estado nos detalhes do jogo. */
export function describeUpdate(update: UpdateState | null): string | null {
  switch (update?.state) {
    case "up_to_date": return "Atualizado";
    case "pending": return "Atualização disponível";
    case "downloading": {
      const progress = updateProgress(update);
      if (progress === null) return "Atualizando…";
      return `Atualizando · ${Math.floor(progress * 100)}% · ${formatSize(update.done)} de ${formatSize(update.total)}`;
    }
    case "version": return `Versão ${update.version}`;
    default: return null;
  }
}
