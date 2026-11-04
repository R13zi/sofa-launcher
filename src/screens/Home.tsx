import { useEffect, useState } from "preact/hooks";
import { artUrl, call, GameView, on } from "../api";
import { Hints } from "../components/Hints";
import { openKeyboard } from "../components/VirtualKeyboard";
import { afterRender, focusFirst, nav, useActions } from "../nav";
import { GameDetail } from "./GameDetail";
import { buildTabs, cycleTab, favoriteGames, filterGames, recentGames } from "./library";
import { openQuickMenu } from "./QuickMenu";

function GameCard({ game, autofocus }: { game: GameView; autofocus?: boolean }) {
  const [broken, setBroken] = useState(false);
  const src = broken ? null : artUrl(game.art);
  return (
    <button
      class={`card ${src ? "" : "no-art"}`}
      data-autofocus={autofocus ? "" : undefined}
      onClick={() => nav.push(<GameDetail gameId={game.id} />)}
    >
      {src && <img src={src} alt="" loading="lazy" onError={() => setBroken(true)} />}
      {game.favorite && <span class="badge">★</span>}
      <span class="name">{game.title}</span>
    </button>
  );
}

function Section({ title, games, rowFocus }: { title: string; games: GameView[]; rowFocus?: boolean }) {
  if (games.length === 0) return null;
  return (
    <section style={{ marginBottom: "1.2rem" }}>
      <h2>{title}</h2>
      <div class="grid">
        {games.map((g, i) => <GameCard key={g.id} game={g} autofocus={rowFocus && i === 0} />)}
      </div>
    </section>
  );
}

export function useLibrary(): GameView[] | null {
  const [games, setGames] = useState<GameView[] | null>(null);
  useEffect(() => {
    const load = () => call<GameView[]>("library_list").then(setGames);
    void load();
    void on("library-changed", load);
    window.addEventListener("sofa-library-changed", load);
  }, []);
  return games;
}

export function Home() {
  const games = useLibrary();
  const [tab, setTab] = useState("home");
  const [query, setQuery] = useState("");
  const list = games ?? [];
  const tabs = buildTabs(list);
  const activeTab = tabs.some((t) => t.id === tab) ? tab : "home";

  const switchTab = (delta: number) => {
    setQuery("");
    setTab(cycleTab(tabs, activeTab, delta));
  };

  useEffect(() => {
    afterRender(focusFirst);
  }, [activeTab, games === null]);

  useActions({
    lb: () => switchTab(-1),
    rb: () => switchTab(1),
    y: () => openKeyboard({
      title: "Buscar jogo",
      initial: query,
      onChange: setQuery,
      onDone: setQuery,
    }),
    menu: openQuickMenu,
    back: query ? () => setQuery("") : undefined,
  });

  const searching = query.trim().length > 0;
  const showHome = activeTab === "home" && !searching;
  const filtered = filterGames(list, activeTab, query);
  const recent = recentGames(list);
  const favorites = favoriteGames(list);

  return (
    <>
      <div class="top-bar">
        <h1>Sofa</h1>
        <div class="tabs" style={{ marginLeft: "1.5rem" }}>
          {tabs.map((t) => (
            <span key={t.id} class={`tab ${t.id === activeTab ? "current" : ""}`}>{t.label}</span>
          ))}
        </div>
      </div>
      {searching && <p class="muted">Busca: “{query}” — {filtered.length} resultado(s)</p>}
      <div class="scroll">
        {games === null && <p class="muted">Carregando biblioteca…</p>}
        {games !== null && list.length === 0 && (
          <p class="muted">Nenhum jogo encontrado. Abra o menu para adicionar jogos ou emuladores.</p>
        )}
        {showHome ? (
          <>
            <Section title="Continuar jogando" games={recent} rowFocus />
            <Section title="Favoritos" games={favorites.filter((f) => !recent.includes(f))} />
            <Section title="Todos os jogos" games={filtered.filter((g) => !recent.includes(g) && !favorites.includes(g))} />
          </>
        ) : (
          <div class="grid">{filtered.map((g) => <GameCard key={g.id} game={g} />)}</div>
        )}
      </div>
      <Hints items={[["confirm", "Abrir"], ["lb", "Aba anterior"], ["rb", "Próxima aba"], ["y", "Buscar"], ["menu", "Menu"]]} />
    </>
  );
}
