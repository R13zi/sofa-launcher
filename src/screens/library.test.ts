import { describe, expect, it } from "vitest";
import { GameView } from "../api";
import { buildTabs, cycleTab, favoriteGames, filterGames, matchesSearch, recentGames } from "./library";

const game = (id: string, source: string, extra: Partial<GameView> = {}): GameView => ({
  id, title: id, source, art: null, hero: null, favorite: false, hidden: false, last_played: 0, playtime_secs: 0, launch_file: null, ...extra,
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
    expect(ids).toEqual(["home", "all", "steam", "dolphin", "manual", "hidden"]);
    expect(buildTabs([game("a", "epic", { hidden: true })]).map((t) => t.id)).toEqual(["home", "all", "hidden"]);
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

  it("abas dão a volta nas pontas", () => {
    const tabs = buildTabs(games);
    expect(cycleTab(tabs, "home", -1)).toBe("hidden");
    expect(cycleTab(tabs, "hidden", 1)).toBe("home");
  });
});
