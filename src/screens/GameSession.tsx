import { call, SessionInfo } from "../api";
import { showError } from "../components/Dialog";
import { HeroArt } from "../components/HeroArt";
import { Hints } from "../components/Hints";
import { useActions } from "../nav";
import { titleClass } from "./Home";
import { openQuickMenu } from "./QuickMenu";
import { useUpdateStatus } from "./GameDetail";
import { describeUpdate, updateProgress } from "./updateFormat";

/**
 * Fica na tela inteira enquanto o jogo abre e, depois, atrás dele. É o que impede o desktop
 * de aparecer. Durante o jogo, por ser estática, praticamente não consome processamento.
 */
export function GameSession({ session }: { session: SessionInfo }) {
  const launching = session.phase === "launching";
  // Enquanto abre, a Steam pode estar atualizando o jogo antes: o progresso real substitui a barra.
  const update = useUpdateStatus(session.game_id, launching ? "steam" : undefined, null);
  const updating = launching && session.game_id.startsWith("steam:") && update?.state === "downloading";
  const progress = updating ? updateProgress(update) : null;

  // Com o jogo em primeiro plano o controle não chega aqui. Estes botões valem
  // quando o usuário volta para cá a partir do menu, como ao sair das Configurações.
  useActions(
    launching
      ? { back: () => void call("session_cancel").catch(showError) }
      : {
          confirm: () => void call("session_resume").catch(showError),
          back: () => void call("session_resume").catch(showError),
          menu: () => void openQuickMenu(),
        },
  );

  return (
    <div class="session-full">
      <HeroArt hero={session.hero} art={session.art} />
      <div class="session-info">
        <span class="home-meta">{updating ? describeUpdate(update) : launching ? "Iniciando" : "Jogando agora"}</span>
        <h1 class={titleClass(session.title)}>{session.title}</h1>
        {launching ? (
          <div class={`session-bar ${progress !== null ? "determinate" : ""}`} aria-hidden="true">
            <span style={progress !== null ? { width: `${progress * 100}%` } : undefined} />
          </div>
        ) : (
          <p class="muted">Segure os dois analógicos pressionados (L3 + R3) para abrir o menu</p>
        )}
      </div>
      <Hints items={launching ? [["back", "Cancelar"]] : [["confirm", "Voltar ao jogo"], ["menu", "Menu"]]} />
    </div>
  );
}
