import { useEffect, useState } from "preact/hooks";
import { call, DriveView, sourceLabel } from "../api";
import { Hints } from "../components/Hints";
import { nav } from "../nav";
import { uninstallGame } from "./storeActions";
import { formatSize } from "./updateFormat";

/** Discos e o que cada jogo ocupa. */
export function Storage() {
  const [drives, setDrives] = useState<DriveView[] | null>(null);
  const load = () => call<DriveView[]>("storage_overview").then(setDrives);

  useEffect(() => {
    void load();
    window.addEventListener("sofa-library-changed", load);
    return () => window.removeEventListener("sofa-library-changed", load);
  }, []);

  return (
    <>
      <div class="top-bar"><h1>Armazenamento</h1></div>
      <div class="scroll list storage">
        {drives === null && <p class="muted">Medindo os discos…</p>}
        {drives?.map((drive) => {
          const used = drive.total - drive.free;
          const games = drive.games.reduce((sum, g) => sum + g.size, 0);
          return (
            <section class="storage-drive" key={drive.root}>
              <div class="storage-head">
                <h2>Disco {drive.root.replace("\\", "")}</h2>
                <span class="muted">{formatSize(drive.free)} livres de {formatSize(drive.total)}</span>
              </div>
              <div class="storage-bar" role="img" aria-label={`${Math.round((used / drive.total) * 100)}% usado`}>
                <span class="storage-games" style={{ width: `${(games / drive.total) * 100}%` }} />
                <span class="storage-other" style={{ width: `${(Math.max(0, used - games) / drive.total) * 100}%` }} />
              </div>
              <div class="storage-legend muted">
                <span><i class="dot games" /> Jogos · {formatSize(games)}</span>
                <span><i class="dot other" /> Outros arquivos · {formatSize(Math.max(0, used - games))}</span>
              </div>
              {drive.games.map((game, i) => (
                <button
                  key={game.id}
                  data-autofocus={drives.indexOf(drive) === 0 && i === 0 ? "" : undefined}
                  onClick={() => void uninstallGame(game.id, () => {})}
                >
                  <span>{game.title}</span>
                  <span class="detail">{sourceLabel(game.source)} · {game.size > 0 ? formatSize(game.size) : "tamanho desconhecido"}</span>
                </button>
              ))}
              {drive.games.length === 0 && <p class="muted">Nenhum jogo neste disco.</p>}
            </section>
          );
        })}
      </div>
      <Hints items={[["confirm", "Desinstalar"], ["back", "Voltar"]]} />
    </>
  );
}

export const openStorage = () => nav.push(<Storage />);
