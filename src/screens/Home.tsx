import { useEffect, useState } from "preact/hooks";
import { artUrl, call, formatPlaytime, GameView, on, sourceLabel } from "../api";
import { showError } from "../components/Dialog";
import { HeroArt } from "../components/HeroArt";
import { Hints } from "../components/Hints";
import { isCursorVisible } from "../input";
import { openKeyboard } from "../components/VirtualKeyboard";
import { afterRender, focusFirst, moveFocus, nav, useActions } from "../nav";
import { GameDetail } from "./GameDetail";
import { buildTabs, cycleTab, favoriteGames, filterGames, recentGames } from "./library";
import { openQuickMenu } from "./QuickMenu";

function GameCard({ game, autofocus, onSelect }: { game: GameView; autofocus?: boolean; onSelect: () => void }) {
  const [broken, setBroken] = useState(false);
  const src = broken ? null : artUrl(game.art);
  return (
    <button
      class={`card ${src ? "" : "no-art"}`}
      data-autofocus={autofocus ? "" : undefined}
      data-game-id={game.id}
      onFocus={onSelect}
      onMouseEnter={() => {
        if (isCursorVisible()) onSelect();
      }}
      onClick={() => nav.push(<GameDetail gameId={game.id} />)}
    >
      {src && <img src={src} alt="" loading="lazy" onError={() => setBroken(true)} />}
      {game.favorite && <span class="badge">★</span>}
      <span class="name">{game.title}</span>
    </button>
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

/** Títulos longos não cabem na arte em tamanho cheio. */
export const titleClass = (title: string) => (title.length > 18 ? "display home-title long" : "display home-title");

const inHeroActions = () => !!(document.activeElement as HTMLElement | null)?.closest(".home-actions");

function moveWithinHero(direction: "left" | "right"): void {
  if (!inHeroActions()) {
    moveFocus(direction);
    return;
  }
  const buttons = Array.from(document.querySelectorAll<HTMLElement>("[data-layer] .home-actions button"));
  const index = buttons.indexOf(document.activeElement as HTMLElement) + (direction === "right" ? 1 : -1);
  buttons[index]?.focus();
}

export function Home() {
  const games = useLibrary();
  const [tab, setTab] = useState("home");
  const [query, setQuery] = useState("");
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const list = games ?? [];
  const tabs = buildTabs(list);
  const activeTab = tabs.some((t) => t.id === tab) ? tab : "home";

  const switchTab = (delta: number) => {
    setQuery("");
    setTab(cycleTab(tabs, activeTab, delta));
  };

  // Lê o foco diretamente: o evento de foco não dispara com a janela em segundo plano,
  // e o controle funciona mesmo assim (logo depois do boot, por exemplo).
  const syncSelection = () => {
    const id = (document.activeElement as HTMLElement | null)?.dataset?.gameId;
    if (id) setSelectedId(id);
  };

  useEffect(() => {
    afterRender(() => {
      focusFirst();
      syncSelection();
    });
  }, [activeTab, games === null]);

  const searching = query.trim().length > 0;
  const filtered = filterGames(list, activeTab, query);
  // Na tela inicial, o que foi jogado por último vem primeiro.
  const recent = recentGames(list);
  const favorites = favoriteGames(list).filter((g) => !recent.includes(g));
  const row = activeTab === "home" && !searching
    ? [...recent, ...favorites, ...filtered.filter((g) => !recent.includes(g) && !favorites.includes(g))]
    : filtered;
  const selected = row.find((g) => g.id === selectedId) ?? row[0] ?? null;

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
    // Da fileira para cima, sempre vai para o Jogar, qualquer que seja a posição do cartão.
    up: () => {
      const jogar = document.querySelector<HTMLElement>("[data-layer] .home-actions .primary");
      if (inHeroActions() || !jogar) moveFocus("up");
      else jogar.focus();
      syncSelection();
    },
    // Dos botões para baixo, volta ao jogo selecionado, e não ao cartão que estiver logo abaixo.
    down: () => {
      if (inHeroActions() && selected) {
        const card = Array.from(document.querySelectorAll<HTMLElement>("[data-layer] .card"))
          .find((el) => el.dataset.gameId === selected.id);
        if (card) {
          card.focus();
          return;
        }
      }
      moveFocus("down");
      syncSelection();
    },
    // Nos botões, os lados só alternam entre eles: não pulam para a fileira.
    left: () => {
      moveWithinHero("left");
      syncSelection();
    },
    right: () => {
      moveWithinHero("right");
      syncSelection();
    },
  });

  return (
    <div class="home">
      <HeroArt hero={selected?.hero ?? null} art={selected?.art ?? null} />
      <div class="hero-scrim" />

      <div class="home-nav">
        <span class="display home-brand">Sofa</span>
        {tabs.map((t) => (
          <span
            key={t.id}
            class={`tab ${t.id === activeTab && !searching ? "current" : ""}`}
            onClick={() => {
              setQuery("");
              setTab(t.id);
            }}
          >
            {t.label}
          </span>
        ))}
        {searching && <span class="home-search">Busca: “{query}” · {filtered.length} resultado(s)</span>}
      </div>

      {selected && (
        <div class="home-info">
          {/* A chave reinicia a animação ao trocar de jogo; os botões ficam de fora para manter o foco. */}
          <div class="home-text" key={selected.id}>
            <span class="home-eyebrow">{selected.last_played > 0 ? "Continuar jogando" : sourceLabel(selected.source)}</span>
            <h1 class={titleClass(selected.title)}>{selected.title}</h1>
            <span class="home-meta">{sourceLabel(selected.source)} · {formatPlaytime(selected.playtime_secs)}</span>
          </div>
          <div class="home-actions">
            <button class="primary" onClick={() => call("launch_game", { id: selected.id }).catch(showError)}>Jogar</button>
            <button onClick={() => nav.push(<GameDetail gameId={selected.id} />)}>Detalhes</button>
          </div>
        </div>
      )}

      <div
        class="home-row"
        key={activeTab}
        onWheel={(e) => {
          e.currentTarget.scrollLeft += e.deltaY;
        }}
      >
        {row.map((g, i) => (
          <GameCard key={g.id} game={g} autofocus={i === 0} onSelect={() => setSelectedId(g.id)} />
        ))}
      </div>
      {games === null && <p class="home-empty">Carregando biblioteca…</p>}
      {games !== null && row.length === 0 && (
        <p class="home-empty">
          {searching ? "Nenhum jogo encontrado nesta busca." : "Nenhum jogo encontrado. Abra o menu para adicionar jogos ou emuladores."}
        </p>
      )}

      <Hints items={[["confirm", "Abrir"], ["lb", "Aba anterior"], ["rb", "Próxima aba"], ["y", "Buscar"], ["menu", "Menu"]]} />
    </div>
  );
}
