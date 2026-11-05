import { useEffect, useState } from "preact/hooks";
import { on } from "./api";
import { actionForKey, allowsRepeat, CURSOR_HIDE_MS, movedEnough } from "./inputMode";
import { Action, dispatchAction } from "./nav";

export type Family = "xbox" | "nintendo" | "generic" | "keyboard";
export type InputMode = "pad" | "pc";

let padFamily: Family = "xbox";
let mode: InputMode = "pad";
/** Se o último uso no modo PC foi o teclado (e não o mouse). */
let keyboardActive = false;
let textCapture = false;
let cursorVisible = false;
let cursorTimer: ReturnType<typeof setTimeout> | undefined;

const listeners = new Set<() => void>();
const notify = () => listeners.forEach((fn) => fn());

export const currentFamily = (): Family => (mode === "pc" ? "keyboard" : padFamily);
export const isCursorVisible = () => cursorVisible;

export function onFamilyChange(fn: (f: Family) => void): () => void {
  const listener = () => fn(currentFamily());
  listeners.add(listener);
  return () => void listeners.delete(listener);
}

/** Estado para as telas que mudam entre controle e teclado. */
export function useInputMode(): { mode: InputMode; keyboardActive: boolean } {
  const [state, setState] = useState({ mode, keyboardActive });
  useEffect(() => {
    const listener = () => setState({ mode, keyboardActive });
    listeners.add(listener);
    listener();
    return () => void listeners.delete(listener);
  }, []);
  return state;
}

/** Com um campo de texto aberto, as letras são texto, e não atalhos. */
export function setTextCapture(active: boolean): void {
  textCapture = active;
}

function setCursor(visible: boolean): void {
  cursorVisible = visible;
  document.documentElement.classList.toggle("cursor-visible", visible);
  clearTimeout(cursorTimer);
  if (visible) cursorTimer = setTimeout(() => setCursor(false), CURSOR_HIDE_MS);
}

function setMode(next: InputMode, nextKeyboardActive: boolean): void {
  if (next === mode && nextKeyboardActive === keyboardActive) return;
  mode = next;
  keyboardActive = nextKeyboardActive;
  notify();
}

function usePad(): void {
  setCursor(false);
  setMode("pad", false);
}

function startPcInput(): void {
  let lastX: number | null = null;
  let lastY = 0;
  window.addEventListener("mousemove", (e) => {
    if (lastX === null) {
      lastX = e.screenX;
      lastY = e.screenY;
      return;
    }
    if (!movedEnough(e.screenX - lastX, e.screenY - lastY) && !cursorVisible) return;
    lastX = e.screenX;
    lastY = e.screenY;
    setCursor(true);
    setMode("pc", false);
  });

  // Passar o mouse dá foco, e a navegação continua de onde o mouse parou.
  window.addEventListener("mouseover", (e) => {
    if (!cursorVisible || textCapture) return;
    const button = (e.target as Element).closest?.<HTMLButtonElement>("[data-layer] button");
    if (button && !button.disabled) button.focus({ preventScroll: true });
  });

  window.addEventListener("mousedown", (e) => {
    const target = e.target as Element;
    // Clicar em dicas e abas não pode tirar o foco do item atual.
    if (!target.closest?.("button, input")) e.preventDefault();
    if (e.button === 3) {
      e.preventDefault();
      dispatchAction("back");
    }
  });

  window.addEventListener("contextmenu", (e) => {
    e.preventDefault();
    dispatchAction("back");
  });
}

/** Liga controle e teclado ao despachante. `onCombo` recebe L3 + R3 e Ctrl + Shift + F10. */
export async function startInput(onCombo: () => void, pcMode = true): Promise<void> {
  window.addEventListener("keydown", (e) => {
    if (pcMode && e.key === "F2") {
      e.preventDefault();
      if (mode === "pc") usePad();
      else setMode("pc", true);
      return;
    }
    if (pcMode && !e.repeat && !["Control", "Shift", "Alt", "Meta"].includes(e.key)) setMode("pc", true);
    if (textCapture) return;
    const action = actionForKey(e.key, { ctrl: e.ctrlKey, alt: e.altKey, meta: e.metaKey });
    if (!action) return;
    e.preventDefault();
    if (e.repeat && !allowsRepeat(action)) return;
    dispatchAction(action);
  });
  if (pcMode) startPcInput();

  await on<{ action: Action }>("input", (payload) => {
    if (pcMode) usePad();
    dispatchAction(payload.action);
  });
  await on<null>("combo", () => onCombo());
  await on<{ family: Family }>("controller", (payload) => {
    if (payload.family !== padFamily) {
      padFamily = payload.family;
      notify();
    }
  });
}
