import { describe, expect, it } from "vitest";
import { applyStep, HoldState, levelFromPointer, volumeStep } from "./volumeLogic";

/** Simula o controle: um toque, 350 ms de espera e repetições a cada 90 ms. */
function holdFor(level: number, dir: -1 | 1, ms: number): number {
  let hold: HoldState | null = null;
  const times = [0];
  for (let t = 350; t <= ms; t += 90) times.push(t);
  for (const t of times) {
    const r = applyStep(level, dir, t, hold);
    if (r) ({ level, hold } = r);
  }
  return level;
}

describe("barra de volume", () => {
  it("um toque muda 2 pontos e respeita os limites", () => {
    expect(applyStep(50, 1, 0, null)?.level).toBe(52);
    expect(applyStep(1, -1, 0, null)?.level).toBe(0);
    expect(applyStep(99, 1, 0, null)?.level).toBe(100);
  });

  it("acelera quando o direcional fica segurado", () => {
    expect(volumeStep(0)).toBe(2);
    expect(volumeStep(1200)).toBe(4);
    expect(volumeStep(2000)).toBe(6);
    // De 0 a 100 em cerca de 2,5 s, e não antes de 2 s.
    expect(holdFor(0, 1, 2000)).toBeLessThan(100);
    expect(holdFor(0, 1, 2700)).toBe(100);
  });

  it("toques separados não aceleram", () => {
    let level = 50;
    let hold: HoldState | null = null;
    for (let t = 0; t < 3000; t += 400) ({ level, hold } = applyStep(level, 1, t, hold)!);
    expect(level).toBe(66);
  });

  it("ignora repetições rápidas demais do teclado", () => {
    const first = applyStep(50, 1, 0, null)!;
    expect(applyStep(first.level, 1, 30, first.hold)).toBeNull();
  });

  it("trocar de direção recomeça devagar", () => {
    const right = applyStep(50, 1, 2000, { start: 0, last: 1910, dir: 1 })!;
    expect(right.level).toBe(56);
    expect(applyStep(right.level, -1, 2090, right.hold)!.level).toBe(54);
  });

  it("clique no trilho vira volume", () => {
    expect(levelFromPointer(150, 100, 200)).toBe(25);
    expect(levelFromPointer(50, 100, 200)).toBe(0);
    expect(levelFromPointer(400, 100, 200)).toBe(100);
  });
});
