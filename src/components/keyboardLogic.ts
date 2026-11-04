/** Regras do teclado virtual, separadas da interface para poderem ser testadas. */

export type KeyboardPage = "letters" | "symbols";

export const LETTER_ROWS = [
  ["1", "2", "3", "4", "5", "6", "7", "8", "9", "0"],
  ["q", "w", "e", "r", "t", "y", "u", "i", "o", "p"],
  ["a", "s", "d", "f", "g", "h", "j", "k", "l", "ç"],
  ["z", "x", "c", "v", "b", "n", "m", "-", "_", "."],
];

export const SYMBOL_ROWS = [
  ["!", "@", "#", "$", "%", "&", "*", "(", ")", "="],
  ["+", "/", "\\", ":", ";", "?", "'", "\"", "[", "]"],
  ["<", ">", "{", "}", "|", "~", "^", "`", ",", "€"],
];

/** Acentos: aperte o acento e depois a letra, como em um teclado ABNT. */
export const DEAD_KEYS = ["´", "`", "^", "~"] as const;
export type DeadKey = (typeof DEAD_KEYS)[number];

const COMPOSE: Record<DeadKey, Record<string, string>> = {
  "´": { a: "á", e: "é", i: "í", o: "ó", u: "ú", c: "ć" },
  "`": { a: "à", e: "è", i: "ì", o: "ò", u: "ù" },
  "^": { a: "â", e: "ê", i: "î", o: "ô", u: "û" },
  "~": { a: "ã", o: "õ", n: "ñ" },
};

export function compose(dead: DeadKey, letter: string): string {
  const lower = letter.toLowerCase();
  const result = COMPOSE[dead][lower];
  if (!result) return dead + letter; // Sem combinação: mantém os dois caracteres.
  return letter === lower ? result : result.toUpperCase();
}

export interface KeyboardState {
  text: string;
  /** Posição do cursor em code points, para não cortar caracteres ao meio. */
  cursor: number;
  shift: boolean;
  capsLock: boolean;
  dead: DeadKey | null;
}

export const initialKeyboard = (text = ""): KeyboardState => ({
  text,
  cursor: Array.from(text).length,
  shift: false,
  capsLock: false,
  dead: null,
});

export function applyCase(s: KeyboardState, key: string): string {
  return s.shift !== s.capsLock ? key.toUpperCase() : key;
}

export function typeChar(s: KeyboardState, key: string): KeyboardState {
  const cased = applyCase(s, key);
  const char = s.dead ? compose(s.dead, cased) : cased;
  const chars = Array.from(s.text);
  const inserted = Array.from(char);
  chars.splice(s.cursor, 0, ...inserted);
  return { ...s, text: chars.join(""), cursor: s.cursor + inserted.length, shift: false, dead: null };
}

export function pressDead(s: KeyboardState, dead: DeadKey): KeyboardState {
  // Apertar o mesmo acento duas vezes escreve o próprio símbolo.
  if (s.dead === dead) return { ...typeRaw(s, dead), dead: null };
  return { ...s, dead };
}

function typeRaw(s: KeyboardState, char: string): KeyboardState {
  const chars = Array.from(s.text);
  chars.splice(s.cursor, 0, char);
  return { ...s, text: chars.join(""), cursor: s.cursor + 1 };
}

export function backspace(s: KeyboardState): KeyboardState {
  if (s.dead) return { ...s, dead: null };
  if (s.cursor === 0) return s;
  const chars = Array.from(s.text);
  chars.splice(s.cursor - 1, 1);
  return { ...s, text: chars.join(""), cursor: s.cursor - 1 };
}

export function moveCursor(s: KeyboardState, delta: number): KeyboardState {
  const max = Array.from(s.text).length;
  return { ...s, cursor: Math.min(max, Math.max(0, s.cursor + delta)) };
}
