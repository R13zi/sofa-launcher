import type { Battery } from "../api";

/** Limiar em que o ícone fica com a cor de alerta. */
export const LOW_BATTERY = 20;

export function batteryLabel(b: Battery): string {
  if (b.state === "wired") return "No cabo";
  if (b.state === "full") return "Carregado";
  if (b.state === "unknown") return "Sem leitura";
  const pct = b.percent === null ? "" : `${b.percent}%`;
  if (b.state === "charging") return pct ? `Carregando · ${pct}` : "Carregando";
  return pct || "Na bateria";
}

/** Fração preenchida no desenho; no cabo ou carregado, cheio. */
export function batteryFill(b: Battery): number {
  if (b.state === "wired" || b.state === "full") return 1;
  if (b.state === "unknown") return 0;
  return b.percent === null ? 0.5 : Math.max(0.06, b.percent / 100);
}

export const isLow = (b: Battery) => b.state === "battery" && b.percent !== null && b.percent <= LOW_BATTERY;
