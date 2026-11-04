import { useEffect, useState } from "preact/hooks";
import { call, on } from "../api";
import { Hints } from "../components/Hints";
import {
  applyCase, DEAD_KEYS, initialKeyboard, KeyboardPage, KeyboardState, LETTER_ROWS,
  pressDead, SYMBOL_ROWS, typeChar,
} from "../components/keyboardLogic";
import { focusFirst, useActions } from "../nav";

const key = (name: string) => void call("desktop_key", { name });
const close = () => void call("desktop_keyboard", { open: false });

/** Teclado que digita na janela em primeiro plano. Cada letra é enviada na hora. */
function SendKeyboard() {
  const [s, setS] = useState<KeyboardState>(initialKeyboard());
  const [page, setPage] = useState<KeyboardPage>("letters");

  const press = (char: string) => {
    const next = typeChar(s, char);
    void call("desktop_type", { text: next.text });
    setS({ ...next, text: "", cursor: 0 });
  };

  useActions({
    x: () => key("backspace"),
    y: () => press(" "),
    lb: () => key("left"),
    rb: () => key("right"),
    menu: () => key("enter"),
    back: close,
  });

  useEffect(() => {
    focusFirst();
  }, []);

  const rows = page === "letters" ? LETTER_ROWS : SYMBOL_ROWS;

  return (
    <div class="kb-panel" style={{ width: "100%", height: "100%" }}>
      <div class="kb-rows" style={{ flex: 1 }}>
        {rows.map((row, r) => (
          <div class="kb-row" key={r}>
            {row.map((k, i) => (
              <button key={k} data-autofocus={r === 1 && i === 0 ? "" : undefined} onClick={() => press(k)}>
                {applyCase(s, k)}
              </button>
            ))}
          </div>
        ))}
        <div class="kb-row">
          {DEAD_KEYS.map((d) => (
            <button
              key={d}
              class={s.dead === d ? "on" : ""}
              onClick={() => {
                const next = pressDead(s, d);
                if (next.text) void call("desktop_type", { text: next.text });
                setS({ ...next, text: "", cursor: 0 });
              }}
            >
              {d}
            </button>
          ))}
          <button class={`wide ${s.shift ? "on" : ""}`} onClick={() => setS({ ...s, shift: !s.shift })}>Shift</button>
          <button class={`wide ${s.capsLock ? "on" : ""}`} onClick={() => setS({ ...s, capsLock: !s.capsLock })}>Caps</button>
          <button class="wide" onClick={() => setPage(page === "letters" ? "symbols" : "letters")}>
            {page === "letters" ? "#+=" : "abc"}
          </button>
          <button class="wide" onClick={() => key("tab")}>Tab</button>
          <button class="wide" onClick={() => key("paste")}>Colar</button>
        </div>
        <div class="kb-row">
          <button class="wide" onClick={() => key("backspace")}>Apagar</button>
          <button class="space" onClick={() => press(" ")}>Espaço</button>
          <button class="wide" onClick={() => key("enter")}>Enter</button>
          <button class="wide" onClick={close}>Fechar</button>
        </div>
      </div>
      <Hints items={[["confirm", "Digitar"], ["x", "Apagar"], ["y", "Espaço"], ["lb", "◀"], ["rb", "▶"], ["menu", "Enter"], ["back", "Fechar"]]} />
    </div>
  );
}

export function DesktopOverlay() {
  const [keyboard, setKeyboard] = useState(false);
  useEffect(() => {
    void on<boolean>("desktop-keyboard", setKeyboard);
  }, []);

  if (keyboard) return <SendKeyboard />;
  return (
    <div style={{ height: "100%", display: "flex", alignItems: "center", justifyContent: "center" }}>
      <Hints
        items={[
          ["confirm", "Clique"],
          ["x", "Clique direito"],
          ["y", "Teclado"],
          ["menu", "Enter"],
          ["view", "Esc"],
          ["lb", "Alt+Tab"],
          ["rb", "Tab"],
          ["back", "Sair"],
        ]}
      />
    </div>
  );
}
