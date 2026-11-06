import { describe, expect, it } from "vitest";
import { GameView } from "../api";
import { buildTabs, cycleTab, favoriteGames, filterGames, installEntries, matchesSearch, recentGames } from "./library";

const game = (id: string, source: string, extra: Partial<GameView> = {}): GameView => ({
  id, title: id, source, art: null, hero: null, custom_hero: null, favorite: false, hidden: false, last_played: 0, playtime_secs: 0, launch_file: null, update: null, ...extra,
});

describe("biblioteca", () => {
  const games = [
    game("Half-Life", "steam", { favorite: true, last_played: 5 }),
    game("Pokémon", "dolphin", { last_played: 9 }),
    game("Oculto", "steam", { hidden: true, favorite: true, last_played: 99 }),
    game("Solto", "manual"),
  ];

  it("só cria abas de origens com jogos visíveis, em ordem fixa", () => {
    const ids = buildTabs(games).map((t) => t.id);
    expect(ids).toEqual(["home", "all", "steam", "dolphin", "manual", "hidden", "install"]);
    expect(buildTabs([game("a", "epic", { hidden: true })]).map((t) => t.id)).toEqual(["home", "all", "hidden", "install"]);
  });

  it("a aba Instalar existe mesmo sem jogos e não mostra os instalados", () => {
    expect(buildTabs([]).map((t) => t.id)).toEqual(["home", "all", "install"]);
    expect(filterGames(games, "install", "")).toEqual([]);
  });

  it("busca ignora acentos e maiúsculas", () => {
    expect(matchesSearch("Pokémon", "POKEMON")).toBe(true);
    expect(matchesSearch("Half-Life", "zelda")).toBe(false);
  });

  it("filtra por aba e esconde ocultos, exceto na aba de ocultos", () => {
    expect(filterGames(games, "steam", "").map((g) => g.id)).toEqual(["Half-Life"]);
    expect(filterGames(games, "hidden", "").map((g) => g.id)).toEqual(["Oculto"]);
    expect(filterGames(games, "all", "").length).toBe(3);
  });

  it("busca alcança todas as origens, menos ocultos", () => {
    expect(filterGames(games, "steam", "poke").map((g) => g.id)).toEqual(["Pokémon"]);
    expect(filterGames(games, "all", "oculto")).toEqual([]);
  });

  it("recentes ordenados e sem ocultos", () => {
    expect(recentGames(games).map((g) => g.id)).toEqual(["Pokémon", "Half-Life"]);
    expect(favoriteGames(games).map((g) => g.id)).toEqual(["Half-Life"]);
  });

  it("monta os cartões da aba Instalar", () => {
    const steam = { id: "steam:620", title: "Portal 2", source: "steam" as const, art: null };
    const epic = { id: "epic:Calluna", title: "Control", source: "epic" as const, art: null };
    const ids = (list: Parameters<typeof installEntries>[0]) => installEntries(list).map((e) => e.id);
    expect(ids({ steam: [steam], steam_error: null, steam_needs_key: false, epic: [epic] }))
      .toEqual(["steam:620", "epic:Calluna", "action:epic-library"]);
    expect(ids({ steam: [], steam_error: null, steam_needs_key: true, epic: [] })).toEqual(["action:steam-key", "action:epic-library"]);
    const failed = installEntries({ steam: [], steam_error: "perfil privado", steam_needs_key: false, epic: [] });
    expect(failed[0]).toMatchObject({ action: "steam-retry", note: "perfil privado" });
  });

  it("abas dão a volta nas pontas", () => {
    const tabs = buildTabs(games);
    expect(cycleTab(tabs, "home", -1)).toBe("install");
    expect(cycleTab(tabs, "install", 1)).toBe("home");
  });
});
