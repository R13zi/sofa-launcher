/** Regras do modo mouse e teclado, separadas da interface para poderem ser testadas. */
import type { Action } from "./nav";

/** Movimento mínimo do mouse, para uma vibração na mesa não ativar o modo. */
export const MOUSE_THRESHOLD = 12;
/** Tempo parado até o cursor sumir. */
export const CURSOR_HIDE_MS = 3000;

export function movedEnough(dx: number, dy: number): boolean {
  return Math.hypot(dx, dy) >= MOUSE_THRESHOLD;
}

const KEYS: Record<string, Action> = {
  ArrowUp: "up",
  ArrowDown: "down",
  ArrowLeft: "left",
  ArrowRight: "right",
  Enter: "confirm",
  " ": "confirm",
  Escape: "back",
  Backspace: "back",
  x: "x",
  f: "y",
  y: "y",
  q: "lb",
  e: "rb",
  m: "menu",
  v: "view",
};

const REPEATABLE = new Set<Action>(["up", "down", "left", "right"]);

/** Modificadores desligam os atalhos de letra, para não capturar Alt + F4 e parecidos. */
export function actionForKey(key: string, mods: { ctrl?: boolean; alt?: boolean; meta?: boolean } = {}): Action | null {
  if (mods.alt || mods.meta) return null;
  if (mods.ctrl) return key.toLowerCase() === "f" ? "y" : null;
  return KEYS[key.length === 1 ? key.toLowerCase() : key] ?? null;
}

/** Segurar uma tecla só repete a navegação, para não abrir e fechar telas sem querer. */
export const allowsRepeat = (action: Action) => REPEATABLE.has(action);
