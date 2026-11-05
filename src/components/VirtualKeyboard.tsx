import { useEffect, useRef, useState } from "preact/hooks";
import { setTextCapture, useInputMode } from "../input";
import { nav, useActions } from "../nav";
import { Hints } from "./Hints";
import {
  applyCase, backspace, DEAD_KEYS, initialKeyboard, KeyboardPage, LETTER_ROWS,
  moveCursor, pressDead, SYMBOL_ROWS, typeChar,
} from "./keyboardLogic";

interface Props {
  title: string;
  initial?: string;
  /** Oculta o texto digitado (usado para chaves de API). */
  secret?: boolean;
  onDone: (text: string) => void;
  /** Chamado a cada alteração, para a busca filtrar enquanto digita. */
  onChange?: (text: string) => void;
}

/**
 * Digitação. Com o controle, teclado na tela: X apaga, Y dá espaço, LB/RB movem o cursor e Start
 * conclui. No modo mouse e teclado, um campo de verdade recebe o teclado físico, com acentos.
 */
export function VirtualKeyboard({ title, initial = "", secret, onDone, onChange }: Props) {
  const [s, setRaw] = useState(() => initialKeyboard(initial));
  const [page, setPage] = useState<KeyboardPage>("letters");
  const { mode, keyboardActive } = useInputMode();
  const pc = mode === "pc";
  const field = useRef<HTMLInputElement>(null);

  const set = (next: typeof s) => {
    setRaw(next);
    if (next.text !== s.text) onChange?.(next.text);
  };
  const done = () => {
    nav.pop();
    onDone(s.text);
  };
  // Cancelar desfaz o que `onChange` já aplicou, como o filtro da busca.
  const cancel = () => {
    if (s.text !== initial) onChange?.(initial);
    nav.pop();
  };

  useEffect(() => {
    setTextCapture(pc);
    return () => setTextCapture(false);
  }, [pc]);

  // Clicar em uma tecla da tela não pode tirar o texto do campo.
  useEffect(() => {
    if (pc) field.current?.focus();
  }, [pc, s]);

  useActions({
    x: () => set(backspace(s)),
    y: () => set(typeChar({ ...s, shift: false, capsLock: false, dead: null }, " ")),
    lb: () => set(moveCursor(s, -1)),
    rb: () => set(moveCursor(s, 1)),
    menu: done,
    back: cancel,
  });

  const chars = Array.from(secret ? "•".repeat(Array.from(s.text).length) : s.text);
  const rows = page === "letters" ? LETTER_ROWS : SYMBOL_ROWS;
  // Usando o teclado físico, o da tela só atrapalharia. Com o mouse, ele continua clicável.
  const hideKeys = pc && keyboardActive;

  return (
    <div class={`kb-panel anchored ${hideKeys ? "compact" : ""}`}>
      <div class="kb-head">
        <h2 class="display kb-title">{title}</h2>
        {pc ? (
          <input
            ref={field}
            class="kb-text kb-input"
            type={secret ? "password" : "text"}
            value={s.text}
            spellcheck={false}
            autocomplete="off"
            onInput={(e) => {
              const input = e.currentTarget;
              const before = input.value.slice(0, input.selectionStart ?? input.value.length);
              set({ ...s, text: input.value, cursor: Array.from(before).length, shift: false, dead: null });
            }}
            onKeyDown={(e) => {
              if (e.isComposing) return;
              if (e.key === "Enter") {
                e.preventDefault();
                done();
              } else if (e.key === "Escape") {
                e.preventDefault();
                cancel();
              }
            }}
          />
        ) : (
          <div class={`kb-text ${chars.length > 40 ? "right" : ""}`}>
            <span>{chars.slice(0, s.cursor).join("")}</span>
            <span class="kb-cursor" />
            <span>{chars.slice(s.cursor).join("")}</span>
          </div>
        )}
      </div>
      {!hideKeys && (
        <div class="kb-rows">
          {rows.map((row, r) => (
            <div class="kb-row" key={r}>
              {row.map((key, k) => (
                <button key={key} data-autofocus={r === 1 && k === 0 ? "" : undefined} onClick={() => set(typeChar(s, key))}>
                  {applyCase(s, key)}
                </button>
              ))}
            </div>
          ))}
          <div class="kb-row">
            {DEAD_KEYS.map((d) => (
              <button key={d} class={s.dead === d ? "on" : ""} onClick={() => set(pressDead(s, d))}>{d}</button>
            ))}
            <button class={`wide ${s.shift ? "on" : ""}`} onClick={() => set({ ...s, shift: !s.shift })}>Shift</button>
            <button class={`wide ${s.capsLock ? "on" : ""}`} onClick={() => set({ ...s, capsLock: !s.capsLock })}>Caps</button>
            <button class="wide" onClick={() => setPage(page === "letters" ? "symbols" : "letters")}>
              {page === "letters" ? "#+=" : "abc"}
            </button>
          </div>
          <div class="kb-row">
            <button class="wide" onClick={() => set(backspace(s))}>Apagar</button>
            <button class="space" onClick={() => set(typeChar({ ...s, shift: false, capsLock: false, dead: null }, " "))}>Espaço</button>
            <button class="wide" onClick={done}>Concluir</button>
          </div>
        </div>
      )}
      <Hints
        items={pc
          ? [["menu", "Concluir", "Enter"], ["back", "Cancelar", "Esc"]]
          : [["confirm", "Digitar"], ["x", "Apagar"], ["y", "Espaço"], ["menu", "Concluir"], ["back", "Cancelar"]]}
      />
    </div>
  );
}

export function openKeyboard(props: Props): void {
  nav.push(<VirtualKeyboard {...props} />, true);
}
