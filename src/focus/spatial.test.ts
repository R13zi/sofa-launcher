import { describe, expect, it } from "vitest";
import { findNext, Rect } from "./spatial";

const box = (col: number, row: number): Rect => ({
  left: col * 110,
  right: col * 110 + 100,
  top: row * 110,
  bottom: row * 110 + 100,
});

// Grade 3 x 3, em ordem de linhas.
const grid = [0, 1, 2].flatMap((row) => [0, 1, 2].map((col) => box(col, row)));

describe("findNext", () => {
  it("anda para o vizinho direto na grade", () => {
    expect(findNext(box(1, 1), grid, "right")).toBe(5);
    expect(findNext(box(1, 1), grid, "left")).toBe(3);
    expect(findNext(box(1, 1), grid, "up")).toBe(1);
    expect(findNext(box(1, 1), grid, "down")).toBe(7);
  });

  it("retorna -1 na borda", () => {
    expect(findNext(box(2, 1), grid, "right")).toBe(-1);
    expect(findNext(box(0, 0), grid, "up")).toBe(-1);
  });

  it("prefere o alinhado ao diagonal mais próximo", () => {
    const from: Rect = { left: 0, right: 100, top: 0, bottom: 100 };
    const diagonal: Rect = { left: 110, right: 210, top: 120, bottom: 220 };
    const aligned: Rect = { left: 400, right: 500, top: 0, bottom: 100 };
    expect(findNext(from, [diagonal, aligned], "right")).toBe(1);
  });

  it("alcança um elemento largo abaixo de uma linha estreita", () => {
    const wide: Rect = { left: 0, right: 330, top: 330, bottom: 400 };
    expect(findNext(box(2, 2), [wide], "down")).toBe(0);
  });
});
