import { describe, expect, it } from "vitest";
import { backspace, initialKeyboard, moveCursor, pressDead, typeChar } from "./keyboardLogic";

describe("teclado virtual", () => {
  it("compõe acentos do português", () => {
    let s = initialKeyboard();
    s = pressDead(s, "~");
    s = typeChar(s, "a");
    s = pressDead(s, "´");
    s = typeChar(s, "e");
    s = typeChar(s, "ç");
    expect(s.text).toBe("ãéç");
  });

  it("mantém os dois caracteres quando não há combinação", () => {
    const s = typeChar(pressDead(initialKeyboard(), "~"), "z");
    expect(s.text).toBe("~z");
  });

  it("acento repetido escreve o símbolo", () => {
    expect(pressDead(pressDead(initialKeyboard(), "~"), "~").text).toBe("~");
  });

  it("shift vale para uma letra, inclusive acentuada", () => {
    let s = { ...initialKeyboard(), shift: true };
    s = pressDead(s, "´");
    s = typeChar(s, "a");
    s = typeChar(s, "b");
    expect(s.text).toBe("Áb");
  });

  it("caps lock permanece e shift inverte", () => {
    let s = { ...initialKeyboard(), capsLock: true };
    s = typeChar(s, "a");
    s = typeChar(s, "b");
    expect(s.text).toBe("AB");
  });

  it("insere e apaga na posição do cursor", () => {
    let s = initialKeyboard("ac");
    s = moveCursor(s, -1);
    s = typeChar(s, "b");
    expect(s.text).toBe("abc");
    s = backspace(s);
    expect(s.text).toBe("ac");
    expect(s.cursor).toBe(1);
  });

  it("backspace cancela um acento pendente sem apagar texto", () => {
    const s = backspace(pressDead(initialKeyboard("a"), "~"));
    expect(s.text).toBe("a");
    expect(s.dead).toBeNull();
  });

  it("cursor não sai dos limites", () => {
    expect(moveCursor(initialKeyboard("a"), 5).cursor).toBe(1);
    expect(moveCursor(initialKeyboard("a"), -5).cursor).toBe(0);
  });
});
