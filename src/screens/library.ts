import { GameView, sourceLabel } from "../api";

export interface Tab {
  id: string;
  label: string;
}

/** Abas na ordem: início, todos, depois uma por origem que tenha jogos visíveis. */
export function buildTabs(games: GameView[]): Tab[] {
  const sources = Array.from(new Set(games.filter((g) => !g.hidden).map((g) => g.source)));
  const order = ["steam", "epic", "cemu", "dolphin", "rpcs3", "manual"];
  const rank = (s: string) => (order.indexOf(s) === -1 ? order.length : order.indexOf(s));
  sources.sort((a, b) => rank(a) - rank(b));
  const tabs: Tab[] = [{ id: "home", label: "Início" }, { id: "all", label: "Todos" }];
  tabs.push(...sources.map((s) => ({ id: s, label: sourceLabel(s) })));
  if (games.some((g) => g.hidden)) tabs.push({ id: "hidden", label: "Ocultos" });
  return tabs;
}

const fold = (text: string) => text.normalize("NFD").replace(/\p{Diacritic}/gu, "").toLowerCase();

/** Busca ignora maiúsculas e acentos. */
export function matchesSearch(title: string, query: string): boolean {
  return fold(title).includes(fold(query.trim()));
}

export function filterGames(games: GameView[], tab: string, query: string): GameView[] {
  return games.filter((g) => {
    if (query.trim()) return !g.hidden && matchesSearch(g.title, query);
    if (tab === "hidden") return g.hidden;
    if (g.hidden) return false;
    return tab === "all" || tab === "home" || g.source === tab;
  });
}

export const recentGames = (games: GameView[], limit = 8) =>
  games.filter((g) => !g.hidden && g.last_played > 0).sort((a, b) => b.last_played - a.last_played).slice(0, limit);

export const favoriteGames = (games: GameView[]) => games.filter((g) => !g.hidden && g.favorite);

/** Próxima aba, dando a volta nas pontas. */
export function cycleTab(tabs: Tab[], current: string, delta: number): string {
  const index = Math.max(0, tabs.findIndex((t) => t.id === current));
  return tabs[(index + delta + tabs.length) % tabs.length].id;
}
