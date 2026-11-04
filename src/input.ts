import { on } from "./api";
import { Action, dispatchAction } from "./nav";

export type Family = "xbox" | "nintendo" | "generic";

let family: Family = "xbox";
const familyListeners = new Set<(f: Family) => void>();

export const currentFamily = () => family;
export function onFamilyChange(fn: (f: Family) => void): () => void {
  familyListeners.add(fn);
  return () => void familyListeners.delete(fn);
}

const KEYS: Record<string, Action> = {
  ArrowUp: "up",
  ArrowDown: "down",
  ArrowLeft: "left",
  ArrowRight: "right",
  Enter: "confirm",
  Escape: "back",
  x: "x",
  y: "y",
  q: "lb",
  e: "rb",
  m: "menu",
  v: "view",
};

/** Liga controle e teclado reserva ao despachante. `onCombo` recebe L3 + R3 segurados. */
export async function startInput(onCombo: () => void): Promise<void> {
  window.addEventListener("keydown", (e) => {
    const action = KEYS[e.key];
    if (!action) return;
    e.preventDefault();
    dispatchAction(action);
  });

  await on<{ action: Action }>("input", (payload) => dispatchAction(payload.action));
  await on<null>("combo", () => onCombo());
  await on<{ family: Family }>("controller", (payload) => {
    if (payload.family !== family) {
      family = payload.family;
      familyListeners.forEach((fn) => fn(family));
    }
  });
}
