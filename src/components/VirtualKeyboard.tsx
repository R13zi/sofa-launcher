import { useState } from "preact/hooks";
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

/** Teclado na tela. X apaga, Y dá espaço, LB/RB movem o cursor e Start conclui. */
export function VirtualKeyboard({ title, initial = "", secret, onDone, onChange }: Props) {
  const [s, setRaw] = useState(() => initialKeyboard(initial));
  const [page, setPage] = useState<KeyboardPage>("letters");

  const set = (next: typeof s) => {
    setRaw(next);
    if (next.text !== s.text) onChange?.(next.text);
  };
  const done = () => {
    nav.pop();
    onDone(s.text);
  };

  useActions({
    x: () => set(backspace(s)),
    y: () => set(typeChar({ ...s, shift: false, capsLock: false, dead: null }, " ")),
    lb: () => set(moveCursor(s, -1)),
    rb: () => set(moveCursor(s, 1)),
    menu: done,
  });

  const chars = Array.from(secret ? "•".repeat(Array.from(s.text).length) : s.text);
  const rows = page === "letters" ? LETTER_ROWS : SYMBOL_ROWS;

  return (
    <div class="kb-panel">
      <h2>{title}</h2>
      <div class={`kb-text ${chars.length > 40 ? "right" : ""}`}>
        <span>{chars.slice(0, s.cursor).join("")}</span>
        <span class="kb-cursor" />
        <span>{chars.slice(s.cursor).join("")}</span>
      </div>
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
      <Hints items={[["confirm", "Digitar"], ["x", "Apagar"], ["y", "Espaço"], ["menu", "Concluir"], ["back", "Cancelar"]]} />
    </div>
  );
}

export function openKeyboard(props: Props): void {
  nav.push(<VirtualKeyboard {...props} />, true);
}
