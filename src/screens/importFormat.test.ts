import { describe, expect, it } from "vitest";
import { filesForGame, formatBytes, formatEta, ImportItem, isImportable, selectedBytes, statusLabel } from "./importFormat";

const item = (id: string, status: ImportItem["status"], pending = 0): ImportItem => ({
  id, title: id, category: "pc", status, problem: null, size: pending, pending_bytes: pending, exe: null, exe_options: [],
});

describe("formatação da importação", () => {
  it("formata tamanhos em português", () => {
    expect(formatBytes(512)).toBe("512 B");
    expect(formatBytes(1536)).toBe("1,5 KB");
    expect(formatBytes(1024 ** 3 * 2.5)).toBe("2,5 GB");
    expect(formatBytes(1024 ** 2 * 250)).toBe("250 MB");
  });

  it("liga arquivos ao jogo pelo nome, sem confundir nomes parecidos", () => {
    const entry = (name: string) => ({ name, path: `C:\\iso\\${name}`, size: 1, file_count: 1, main_files: [] });
    const entries = [entry("Dusklight"), entry("Pokémon"), entry("Mario 64 2")];
    expect(filesForGame("DUSKLIGHT", entries)?.name).toBe("Dusklight");
    expect(filesForGame("Pokemon", entries)?.name).toBe("Pokémon");
    expect(filesForGame("Mario 64", entries)).toBeNull();
  });

  it("formata o tempo restante", () => {
    expect(formatEta(100, 0)).toBe("calculando…");
    expect(formatEta(100, 10)).toBe("10 s");
    expect(formatEta(600, 2)).toBe("5 min");
    expect(formatEta(3600 * 90, 60)).toBe("1 h 30 min");
  });

  it("só novos e com escolha de exe são importáveis", () => {
    expect(isImportable(item("a", "new"))).toBe(true);
    expect(isImportable(item("a", "needs_exe"))).toBe(true);
    for (const status of ["done", "conflict", "blocked"] as const) {
      expect(isImportable(item("a", status))).toBe(false);
    }
  });

  it("descreve o estado e soma só o que foi marcado", () => {
    expect(statusLabel(item("a", "done"))).toBe("Já importado");
    expect(statusLabel(item("a", "needs_exe", 1024))).toBe("1,0 KB · escolher .exe");
    expect(selectedBytes([item("a", "new", 10), item("b", "new", 20)], new Set(["b"]))).toBe(20);
  });
});
