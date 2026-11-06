import { useEffect, useState } from "preact/hooks";
import { artUrl, call, formatPlaytime, GameView, on, sourceLabel, UpdateState } from "../api";
import { describeUpdate, needsUpdate, updateProgress } from "./updateFormat";
import { confirmAction, openMenu, showError } from "../components/Dialog";
import { openFileBrowser } from "../components/FileBrowser";
import { HeroArt } from "../components/HeroArt";
import { Hints } from "../components/Hints";
import { nav } from "../nav";
import { fileName, openGameFilesEntry } from "./GameFiles";
import { filesForGame, GameFilesEntry } from "./importFormat";

const hasStore = (source: string) => source === "steam" || source === "epic";

/** Relê o estado de atualização a cada segundo enquanto a tela está aberta (só lê um arquivo local). */
export function useUpdateStatus(gameId: string, source: string | undefined, initial: UpdateState | null): UpdateState | null {
  const [update, setUpdate] = useState(initial);
  useEffect(() => setUpdate(initial), [initial?.state]);
  useEffect(() => {
    if (!source || !hasStore(source)) return;
    const timer = setInterval(() => {
      call<UpdateState | null>("game_update_status", { id: gameId }).then(setUpdate).catch(() => {});
    }, 1000);
    return () => clearInterval(timer);
  }, [gameId, source]);
  return update;
}

export function UpdateLine({ update }: { update: UpdateState | null }) {
  const text = describeUpdate(update);
  if (!text) return null;
  const progress = updateProgress(update);
  return (
    <div class={`update-line ${needsUpdate(update) ? "pending" : ""}`}>
      <span>{text}</span>
      {progress !== null && <div class="update-bar"><span style={{ width: `${progress * 100}%` }} /></div>}
    </div>
  );
}

export function GameDetail({ gameId }: { gameId: string }) {
  const [game, setGame] = useState<GameView | null>(null);
  const [gameFiles, setGameFiles] = useState<GameFilesEntry | null>(null);
  const [checkNote, setCheckNote] = useState<string | null>(null);
  const [checkedVersion, setCheckedVersion] = useState<string | null>(null);
  const update = useUpdateStatus(gameId, game?.source, game?.update ?? null);

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

  const change = async (command: string, args: Record<string, unknown>) => {
    try {
      await call(command, { id: game.id, ...args });
      await load();
    } catch (e) {
      showError(e);
    }
  };

  const cover = artUrl(game.art);
  const updateFirst = game.source === "steam" && needsUpdate(update);
  const newVersion = checkedVersion && update?.state === "version" && update.version !== checkedVersion ? update.version : null;

  const checkUpdates = async () => {
    try {
      const startedSteam = await call<boolean>("game_update_check", { id: game.id });
      if (game.source === "epic") {
        setCheckedVersion(update?.state === "version" ? update.version : null);
        setCheckNote("A Epic está procurando atualizações. Se achar uma, ela baixa, e a versão nova aparece aqui.");
      } else {
        setCheckNote(startedSteam
          ? "Abrindo a Steam em segundo plano. Ela confere as atualizações em alguns segundos."
          : "A Steam já está aberta e confere as atualizações sozinha. O estado acima é o mais recente.");
      }
    } catch (e) {
      showError(e);
    }
  };

  return (
    <div class="detail-screen">
      <HeroArt hero={game.hero} art={game.art} />
      <div class="detail-cover">
        {cover ? <img src={cover} alt="" /> : <span>{game.title}</span>}
      </div>

      <div class="detail-panel">
        <h1 class="display detail-title">{game.title}</h1>
        <p class="muted" style={{ margin: 0 }}>{sourceLabel(game.source)} · {formatPlaytime(game.playtime_secs)}</p>
        <UpdateLine update={update} />
        {newVersion && <p class="update-note">Atualizado para a versão {newVersion}.</p>}
        {!newVersion && checkNote && <p class="update-note">{checkNote}</p>}
        <div class="detail-actions">
          <button
            class="primary"
            data-autofocus
            onClick={() => call("launch_game", { id: game.id }).catch(showError)}
          >
            {updateFirst ? "Atualizar e jogar" : "Jogar"}
          </button>
          {hasStore(game.source) && <button onClick={() => void checkUpdates()}>Procurar atualizações</button>}
          <button onClick={() => change("set_favorite", { value: !game.favorite })}>
            {game.favorite ? "★ Remover dos favoritos" : "☆ Adicionar aos favoritos"}
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
                      onPick: (path) => change("set_launch_file", { path }),
                    }),
                  },
                  ...(game.launch_file
                    ? [{ label: "Remover", danger: true, onClick: () => change("set_launch_file", { path: null }) }]
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
              onPick: (path) => change("set_art", { path }),
            })}
          >
            Escolher capa
          </button>
          <button
            onClick={() => openMenu(
              "Banner",
              [
                {
                  label: "Escolher imagem",
                  onClick: () => openFileBrowser({
                    title: "Escolher banner",
                    exts: ["jpg", "jpeg", "png", "webp"],
                    onPick: (path) => change("set_hero", { path }),
                  }),
                },
                ...(game.custom_hero
                  ? [{ label: "Remover", danger: true, onClick: () => change("set_hero", { path: null }) }]
                  : []),
              ],
              `${game.custom_hero ?? "Nenhuma imagem escolhida."}\n\nEste banner vale mais do que os importados, os baixados e os da Steam.`,
            )}
          >
            Banner: {game.custom_hero ? fileName(game.custom_hero) : "automático"}
          </button>
          <button onClick={() => change("set_hidden", { value: !game.hidden })}>
            {game.hidden ? "Mostrar na biblioteca" : "Ocultar da biblioteca"}
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
        <Hints items={[["confirm", "Selecionar"], ["back", "Voltar"]]} />
      </div>
    </div>
  );
}
