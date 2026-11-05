import { describe, expect, it } from "vitest";
import { actionForKey, allowsRepeat, movedEnough } from "./inputMode";

describe("modo mouse e teclado", () => {
  it("ignora movimentos mínimos do mouse", () => {
    expect(movedEnough(0, 0)).toBe(false);
    expect(movedEnough(3, 4)).toBe(false);
    expect(movedEnough(12, 0)).toBe(true);
    expect(movedEnough(-9, -9)).toBe(true);
  });

  it("mapeia as teclas, com ou sem Caps Lock", () => {
    expect(actionForKey("ArrowLeft")).toBe("left");
    expect(actionForKey("Enter")).toBe("confirm");
    expect(actionForKey("Escape")).toBe("back");
    expect(actionForKey("Backspace")).toBe("back");
    expect(actionForKey("f")).toBe("y");
    expect(actionForKey("F")).toBe("y");
    expect(actionForKey("Q")).toBe("lb");
    expect(actionForKey("m")).toBe("menu");
    expect(actionForKey("z")).toBeNull();
    expect(actionForKey("F10")).toBeNull();
  });

  it("modificadores não acionam atalhos de letra, exceto Ctrl + F", () => {
    expect(actionForKey("f", { ctrl: true })).toBe("y");
    expect(actionForKey("q", { ctrl: true })).toBeNull();
    expect(actionForKey("f", { alt: true })).toBeNull();
    expect(actionForKey("ArrowUp", { alt: true })).toBeNull();
    expect(actionForKey("m", { meta: true })).toBeNull();
  });

  it("só a navegação repete ao segurar a tecla", () => {
    expect(allowsRepeat("down")).toBe(true);
    expect(allowsRepeat("confirm")).toBe(false);
    expect(allowsRepeat("back")).toBe(false);
  });
});
