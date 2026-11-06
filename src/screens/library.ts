import { GameView, Installable, InstallList, sourceLabel } from "../api";

export type InstallAction = "install" | "steam-key" | "steam-retry" | "epic-library";

export interface InstallEntry {
  id: string;
  title: string;
  source: "steam" | "epic";
  art: string | null;
  action: InstallAction;
  /** Linha abaixo do título. */
  note: string;
  game?: Installable;
}

/** Cartões da aba Instalar: Steam, depois Epic, com os atalhos de configuração no lugar certo. */
export function installEntries(list: InstallList): InstallEntry[] {
  const entries: InstallEntry[] = [];
  if (list.steam_needs_key) {
    entries.push({
      id: "action:steam-key", title: "Conectar a Steam", source: "steam", art: null, action: "steam-key",
      note: "Digite uma vez a chave da API Steam para ver os jogos da sua conta.",
    });
  } else if (list.steam_error) {
    entries.push({ id: "action:steam-retry", title: "Steam indisponível", source: "steam", art: null, action: "steam-retry", note: list.steam_error });
  }
  const toEntry = (g: Installable): InstallEntry => ({
    id: g.id, title: g.title, source: g.source, art: g.art, action: "install", game: g,
    note: `${sourceLabel(g.source)} · Não instalado`,
  });
  entries.push(...list.steam.map(toEntry), ...list.epic.map(toEntry));
  entries.push({
    id: "action:epic-library", title: "Biblioteca da Epic", source: "epic", art: null, action: "epic-library",
    note: "Abre a Epic para instalar jogos que não aparecem aqui.",
  });
  return entries;
}

/** Aba com os jogos da conta que ainda não estão instalados. */
export const INSTALL_TAB = "install";

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
  tabs.push({ id: INSTALL_TAB, label: "Instalar" });
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
    if (tab === INSTALL_TAB) return false;
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
