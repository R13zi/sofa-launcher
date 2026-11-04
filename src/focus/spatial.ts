export type Direction = "up" | "down" | "left" | "right";

export interface Rect {
  left: number;
  top: number;
  right: number;
  bottom: number;
}

/**
 * Escolhe o próximo elemento na direção pedida: precisa estar à frente do atual, e
 * elementos alinhados no eixo perpendicular têm preferência sobre os mais próximos em diagonal.
 * Retorna o índice em `candidates`, ou -1 se não houver nada naquela direção.
 */
export function findNext(from: Rect, candidates: Rect[], direction: Direction): number {
  const horizontal = direction === "left" || direction === "right";
  let best = -1;
  let bestScore = Infinity;

  candidates.forEach((c, i) => {
    let gap: number;
    switch (direction) {
      case "up":
        gap = from.top - c.bottom;
        break;
      case "down":
        gap = c.top - from.bottom;
        break;
      case "left":
        gap = from.left - c.right;
        break;
      case "right":
        gap = c.left - from.right;
        break;
    }
    // Tolera pequena sobreposição, comum com bordas e transformações de foco.
    if (gap < -4) return;

    const overlap = horizontal
      ? Math.min(from.bottom, c.bottom) - Math.max(from.top, c.top)
      : Math.min(from.right, c.right) - Math.max(from.left, c.left);
    const fromCenter = horizontal ? (from.top + from.bottom) / 2 : (from.left + from.right) / 2;
    const cCenter = horizontal ? (c.top + c.bottom) / 2 : (c.left + c.right) / 2;
    const offAxis = Math.abs(fromCenter - cCenter);

    const score = Math.max(gap, 0) + offAxis * (overlap > 0 ? 0.1 : 2) + (overlap > 0 ? 0 : 100000);
    if (score < bestScore) {
      bestScore = score;
      best = i;
    }
  });

  return best;
}
