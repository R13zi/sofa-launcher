import { useEffect, useState } from "preact/hooks";
import { artUrl, call, formatPlaytime, GameView, on, sourceLabel } from "../api";
import { confirmAction, openMenu, showError } from "../components/Dialog";
import { openFileBrowser } from "../components/FileBrowser";
import { Hints } from "../components/Hints";
import { nav } from "../nav";
import { fileName, openGameFilesEntry } from "./GameFiles";
import { filesForGame, GameFilesEntry } from "./importFormat";

export function GameDetail({ gameId }: { gameId: string }) {
  const [game, setGame] = useState<GameView | null>(null);
  const [gameFiles, setGameFiles] = useState<GameFilesEntry | null>(null);

  const load = async () => {
    const games = await call<GameView[]>("library_list");
    const found = games.find((g) => g.id === gameId) ?? null;
    setGame(found);
    if (found) setGameFiles(filesForGame(found.title, await call<GameFilesEntry[]>("game_files_list")));
  };

  useEffect(() => {
    void load();
    void on("library-changed", load);
  }, []);

  if (!game) return <p class="muted">Carregando…</p>;

  const update = async (command: string, args: Record<string, unknown>) => {
    try {
      await call(command, { id: game.id, ...args });
      await load();
    } catch (e) {
      showError(e);
    }
  };

  const src = artUrl(game.art);

  return (
    <>
      <div class="top-bar">
        <h1>{game.title}</h1>
      </div>
      <div class="detail-page">
        <div class="cover">
          {src ? <img src={src} alt="" /> : <span>{game.title}</span>}
        </div>
        <div class="side">
          <p class="muted">{sourceLabel(game.source)} · {formatPlaytime(game.playtime_secs)}</p>
          <button
            data-autofocus
            onClick={() => call("launch_game", { id: game.id }).catch(showError)}
          >
            ▶ Jogar
          </button>
          <button onClick={() => update("set_favorite", { value: !game.favorite })}>
            {game.favorite ? "★ Remover dos favoritos" : "☆ Adicionar aos favoritos"}
          </button>
          <button onClick={() => update("set_hidden", { value: !game.hidden })}>
            {game.hidden ? "Mostrar na biblioteca" : "Ocultar da biblioteca"}
          </button>
          {gameFiles && (
            <button onClick={() => openGameFilesEntry(gameFiles, game, () => void load())}>
              Arquivos deste jogo (ISO e assets)
            </button>
          )}
          {game.source === "manual" && (
            <button
              onClick={() => openMenu(
                "Arquivo ao iniciar",
                [
                  {
                    label: "Escolher arquivo",
                    onClick: () => openFileBrowser({
                      title: "Arquivo para passar ao jogo",
                      onPick: (path) => update("set_launch_file", { path }),
                    }),
                  },
                  ...(game.launch_file
                    ? [{ label: "Remover", danger: true, onClick: () => update("set_launch_file", { path: null }) }]
                    : []),
                ],
                `${game.launch_file ?? "Nenhum arquivo definido."}\n\nO Sofa passa este caminho ao jogo sempre que ele iniciar. Serve para jogos que aceitam a ISO por comando, como o Dusklight.`,
              )}
            >
              Arquivo ao iniciar: {game.launch_file ? fileName(game.launch_file) : "nenhum"}
            </button>
          )}
          <button
            onClick={() => openFileBrowser({
              title: "Escolher capa",
              exts: ["jpg", "jpeg", "png", "webp"],
              onPick: (path) => update("set_art", { path }),
            })}
          >
            Escolher capa
          </button>
          {game.source === "manual" && (
            <button
              class="danger"
              onClick={() => confirmAction(
                `Remover ${game.title}?`,
                "Remover",
                async () => {
                  try {
                    await call("manual_remove", { gameId: game.id });
                    await call("library_refresh");
                    nav.pop();
                  } catch (e) {
                    showError(e);
                  }
                },
                "Só tira o jogo da biblioteca. Os arquivos continuam no disco.",
              )}
            >
              Remover da biblioteca
            </button>
          )}
        </div>
      </div>
      <Hints items={[["confirm", "Selecionar"], ["back", "Voltar"]]} />
    </>
  );
}
