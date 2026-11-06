import { describe, expect, it } from "vitest";
import { batteryFill, batteryLabel, isLow } from "./batteryFormat";

describe("bateria do controle", () => {
  it("descreve cada estado", () => {
    expect(batteryLabel({ state: "battery", percent: 64 })).toBe("64%");
    expect(batteryLabel({ state: "charging", percent: 40 })).toBe("Carregando · 40%");
    expect(batteryLabel({ state: "charging", percent: null })).toBe("Carregando");
    expect(batteryLabel({ state: "wired", percent: null })).toBe("No cabo");
    expect(batteryLabel({ state: "full", percent: 100 })).toBe("Carregado");
    expect(batteryLabel({ state: "battery", percent: null })).toBe("Na bateria");
    expect(batteryLabel({ state: "unknown", percent: null })).toBe("Sem leitura");
    expect(isLow({ state: "unknown", percent: null })).toBe(false);
  });

  it("preenche o desenho conforme a carga", () => {
    expect(batteryFill({ state: "battery", percent: 50 })).toBe(0.5);
    expect(batteryFill({ state: "battery", percent: 0 })).toBeGreaterThan(0);
    expect(batteryFill({ state: "wired", percent: null })).toBe(1);
  });

  it("só marca como baixa na bateria e até 20%", () => {
    expect(isLow({ state: "battery", percent: 20 })).toBe(true);
    expect(isLow({ state: "battery", percent: 21 })).toBe(false);
    expect(isLow({ state: "charging", percent: 5 })).toBe(false);
  });
});
