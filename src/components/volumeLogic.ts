/** Um toque muda 2 pontos. Segurando, o passo cresce para ir de 0 a 100 em cerca de 2,5 s. */
export function volumeStep(heldMs: number): number {
  if (heldMs < 1000) return 2;
  if (heldMs < 1600) return 4;
  return 6;
}

/** Intervalo máximo entre dois comandos para contar como o mesmo direcional segurado. */
export const HOLD_GAP_MS = 250;
/** Repetições do teclado mais rápidas que isso são ignoradas, para andar no ritmo do controle. */
export const MIN_REPEAT_MS = 80;

export interface HoldState {
  start: number;
  last: number;
  dir: number;
}

/**
 * Aplica um comando de esquerda ou direita. Devolve o novo volume e o estado da sequência,
 * ou `null` quando o comando chegou cedo demais e deve ser ignorado.
 */
export function applyStep(level: number, dir: -1 | 1, now: number, hold: HoldState | null): { level: number; hold: HoldState } | null {
  const continuing = hold !== null && hold.dir === dir && now - hold.last <= HOLD_GAP_MS;
  if (continuing && now - hold.last < MIN_REPEAT_MS) return null;
  const start = continuing ? hold.start : now;
  const next = clampLevel(level + dir * volumeStep(now - start));
  return { level: next, hold: { start, last: now, dir } };
}

export const clampLevel = (level: number) => Math.max(0, Math.min(100, Math.round(level)));

/** Posição do clique no trilho para um volume de 0 a 100. */
export const levelFromPointer = (x: number, left: number, width: number) =>
  width <= 0 ? 0 : clampLevel(((x - left) / width) * 100);
