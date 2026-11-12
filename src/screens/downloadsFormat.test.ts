import { describe, expect, it } from "vitest";
import type { Download } from "../api";
import { describeDownload, finished, formatEta, formatSpeed, problemOf, ProgressTracker, STALL_MS } from "./downloadsFormat";

const GB = 1024 ** 3;
const MB = 1024 ** 2;

function dl(over: Partial<Download> = {}): Download {
  return {
    id: "steam:730", title: "Counter-Strike 2", source: "steam", art: null, kind: "update", status: "active",
    done: 10 * GB, total: 20 * GB, drive: "C:\\", free: 100 * GB, in_library: true, ...over,
  };
}

describe("downloads", () => {
  it("mede a velocidade e percebe quando parou", () => {
    const t = new ProgressTracker();
    t.record([dl({ done: 0 })], 0);
    t.record([dl({ done: 50 * MB })], 5000);
    expect(t.speed("steam:730")).toBe(10 * MB);
    t.record([dl({ done: 50 * MB })], 5000 + STALL_MS);
    expect(t.idleFor("steam:730", 5000 + STALL_MS)).toBe(STALL_MS);
    t.record([], 6000 + STALL_MS);
    expect(t.speed("steam:730")).toBeNull();
  });

  it("avisa quando o resto não cabe no disco", () => {
    expect(problemOf(dl({ free: 4 * GB }), 0)).toEqual({ kind: "no_space", missing: 6 * GB });
    expect(problemOf(dl(), STALL_MS)).toEqual({ kind: "stalled", minutes: 2 });
    expect(problemOf(dl({ done: 20 * GB }), STALL_MS)).toBeNull();
    expect(problemOf(dl({ status: "queued", free: 0 }), STALL_MS)).toBeNull();
  });

  it("descreve cada situação", () => {
    expect(describeDownload(dl(), 42 * MB, null)).toBe("Steam · Atualização · 10 GB de 20 GB · 42 MB/s · cerca de 4 min");
    expect(describeDownload(dl({ status: "queued", total: 0 }), null, null)).toBe("Steam · Atualização · aguardando");
    expect(describeDownload(dl({ free: 4 * GB }), null, { kind: "no_space", missing: 6 * GB }))
      .toBe("Steam · C: encheu durante o download. Libere mais 6 GB para continuar.");
    expect(describeDownload(dl({ total: 0, done: 0, kind: "install" }), null, null)).toBe("Steam · Instalação · preparando");
  });

  it("formata velocidade e tempo", () => {
    expect(formatSpeed(1.5 * MB)).toBe("1,5 MB/s");
    expect(formatSpeed(300 * 1024)).toBe("300 KB/s");
    expect(formatEta(30)).toBe("menos de 1 min");
    expect(formatEta(95 * 60)).toBe("cerca de 1 h 35 min");
  });

  it("marca como concluído o que estava baixando e sumiu", () => {
    const queued = dl({ id: "steam:1", status: "queued" });
    expect(finished([dl(), queued], [], 99)).toEqual([{ id: "steam:730", title: "Counter-Strike 2", at: 99 }]);
    expect(finished([dl()], [dl()], 99)).toEqual([]);
  });
});
