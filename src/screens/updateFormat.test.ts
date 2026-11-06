import { describe, expect, it } from "vitest";
import { describeUpdate, formatSize, needsUpdate, updateProgress } from "./updateFormat";

describe("atualizações de jogos", () => {
  it("descreve cada estado", () => {
    expect(describeUpdate({ state: "up_to_date" })).toBe("Atualizado");
    expect(describeUpdate({ state: "pending" })).toBe("Atualização disponível");
    expect(describeUpdate({ state: "version", version: "1.2.3" })).toBe("Versão 1.2.3");
    expect(describeUpdate(null)).toBeNull();
  });

  it("mostra o progresso do download", () => {
    const update = { state: "downloading" as const, done: 1.2 * 1024 ** 3, total: 2.8 * 1024 ** 3 };
    expect(describeUpdate(update)).toBe("Atualizando · 42% · 1,2 GB de 2,8 GB");
    expect(describeUpdate({ state: "downloading", done: 0, total: 0 })).toBe("Atualizando…");
    expect(updateProgress({ state: "downloading", done: 5, total: 4 })).toBe(1);
    expect(updateProgress({ state: "pending" })).toBeNull();
  });

  it("tamanhos pequenos em MB", () => {
    expect(formatSize(300 * 1024 ** 2)).toBe("300 MB");
    expect(formatSize(10)).toBe("1 MB");
  });

  it("só pendente ou baixando pedem atualização", () => {
    expect(needsUpdate({ state: "pending" })).toBe(true);
    expect(needsUpdate({ state: "downloading", done: 0, total: 1 })).toBe(true);
    expect(needsUpdate({ state: "up_to_date" })).toBe(false);
    expect(needsUpdate({ state: "version", version: "1" })).toBe(false);
  });
});
