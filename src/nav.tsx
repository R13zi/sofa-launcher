import { ComponentChild, createContext } from "preact";
import { useContext, useEffect, useLayoutEffect, useRef, useState } from "preact/hooks";
import { Direction, findNext } from "./focus/spatial";

export type Action =
  | "up" | "down" | "left" | "right"
  | "confirm" | "back" | "x" | "y" | "lb" | "rb" | "menu" | "view";

export type Handlers = Partial<Record<Action, () => void>>;

export interface Layer {
  id: number;
  view: ComponentChild;
  /** Camada sobreposta: a de baixo continua visível por trás. */
  overlay: boolean;
  handlers: Handlers;
  savedFocus: HTMLElement | null;
  element?: HTMLElement | null;
}

/**
 * Roda depois do Preact atualizar a tela. Não usa requestAnimationFrame porque ele
 * fica parado enquanto a janela está oculta, como acontece durante um jogo.
 */
export function afterRender(fn: () => void): void {
  setTimeout(fn, 30);
}

let stack: Layer[] = [];
let nextId = 1;
const listeners = new Set<() => void>();
const notify = () => listeners.forEach((l) => l());

export const nav = {
  push(view: ComponentChild, overlay = false): void {
    const top = stack[stack.length - 1];
    // Só guarda o foco se ele estiver nesta camada. Logo após fechar um diálogo, ele ainda não
    // voltou para cá, e o que foi guardado antes continua sendo o certo.
    const active = document.activeElement as HTMLElement | null;
    if (top && active && top.element?.contains(active)) top.savedFocus = active;
    stack = [...stack, { id: nextId++, view, overlay, handlers: {}, savedFocus: null }];
    notify();
  },
  pop(): void {
    if (stack.length <= 1) return;
    stack = stack.slice(0, -1);
    notify();
    const top = stack[stack.length - 1];
    afterRender(() => {
      if (top.savedFocus?.isConnected) top.savedFocus.focus();
      else focusFirst();
    });
  },
  /** Volta para a camada base (tela inicial). */
  reset(): void {
    stack = stack.slice(0, 1);
    notify();
    afterRender(focusFirst);
  },
  depth: () => stack.length,
};

const LayerContext = createContext<Layer | null>(null);

/** Registra botões específicos da tela atual. */
export function useActions(handlers: Handlers): void {
  const layer = useContext(LayerContext);
  if (layer) layer.handlers = handlers;
}

function activeRoot(): HTMLElement | null {
  const layers = document.querySelectorAll<HTMLElement>("[data-layer]");
  return layers[layers.length - 1] ?? null;
}

function focusables(root: HTMLElement): HTMLElement[] {
  return Array.from(root.querySelectorAll<HTMLElement>("button:not([disabled])")).filter(
    (el) => el.offsetParent !== null,
  );
}

export function focusFirst(): void {
  const root = activeRoot();
  if (!root) return;
  const preferred = root.querySelector<HTMLElement>("[data-autofocus]:not([disabled])");
  const target = preferred ?? focusables(root)[0];
  target?.focus();
  target?.scrollIntoView({ block: "center", inline: "nearest" });
}

function moveFocus(direction: Direction): void {
  const root = activeRoot();
  if (!root) return;
  const current = document.activeElement as HTMLElement | null;
  if (!current || !root.contains(current)) {
    focusFirst();
    return;
  }
  const others = focusables(root).filter((el) => el !== current);
  const index = findNext(current.getBoundingClientRect(), others.map((el) => el.getBoundingClientRect()), direction);
  if (index >= 0) {
    others[index].focus({ preventScroll: true });
    others[index].scrollIntoView({ block: "center", inline: "nearest", behavior: "smooth" });
  }
}

/** Ponto único por onde passam controle e teclado. */
export function dispatchAction(action: Action): void {
  const top = stack[stack.length - 1];
  const handler = top?.handlers[action];
  if (handler) {
    handler();
    return;
  }
  switch (action) {
    case "up":
    case "down":
    case "left":
    case "right":
      moveFocus(action);
      break;
    case "confirm": {
      const el = document.activeElement as HTMLElement | null;
      const root = activeRoot();
      if (el && root?.contains(el)) el.click();
      else focusFirst();
      break;
    }
    case "back":
      nav.pop();
      break;
  }
}

function LayerView({ layer, active, hidden }: { layer: Layer; active: boolean; hidden: boolean }) {
  const ref = useRef<HTMLDivElement>(null);
  useLayoutEffect(() => {
    layer.element = ref.current;
    if (active && !ref.current?.contains(document.activeElement)) focusFirst();
  }, []);
  // Telas que carregam dados depois de abrir, ou que perdem o item focado, precisam de novo foco.
  useEffect(() => {
    const element = ref.current;
    if (!active || !element) return;
    const observer = new MutationObserver(() => {
      if (!element.contains(document.activeElement)) focusFirst();
    });
    observer.observe(element, { childList: true, subtree: true });
    return () => observer.disconnect();
  }, [active]);
  return (
    <LayerContext.Provider value={layer}>
      <div
        ref={ref}
        class={layer.overlay ? "layer overlay" : "layer"}
        style={hidden ? { display: "none" } : undefined}
        {...(active ? { "data-layer": "" } : { inert: true })}
      >
        {layer.view}
      </div>
    </LayerContext.Provider>
  );
}

export function NavRoot({ root }: { root: ComponentChild }) {
  const [, force] = useState(0);
  useEffect(() => {
    const l = () => force((n) => n + 1);
    listeners.add(l);
    return () => void listeners.delete(l);
  }, []);
  if (stack.length === 0) {
    stack = [{ id: nextId++, view: root, overlay: false, handlers: {}, savedFocus: null }];
  }
  // Camadas abaixo da última tela cheia não precisam ser desenhadas.
  let firstVisible = stack.length - 1;
  while (firstVisible > 0 && stack[firstVisible].overlay) firstVisible--;
  return (
    <>
      {stack.map((layer, i) => (
        <LayerView key={layer.id} layer={layer} active={i === stack.length - 1} hidden={i < firstVisible} />
      ))}
    </>
  );
}
