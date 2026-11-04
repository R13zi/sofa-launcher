import { artUrl, call, SessionInfo } from "../api";
import { showError } from "../components/Dialog";
import { Hints } from "../components/Hints";
import { useActions } from "../nav";
import { openQuickMenu } from "./QuickMenu";

/**
 * Fica na tela inteira enquanto o jogo abre e, depois, atrás dele. É o que impede o desktop
 * de aparecer. Durante o jogo, por ser estática, praticamente não consome processamento.
 */
export function GameSession({ session }: { session: SessionInfo }) {
  const launching = session.phase === "launching";
  const src = artUrl(session.art);

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
    <>
      <div class="session-screen">
        {src && <img class="session-art" src={src} alt="" />}
        <h1>{launching ? `Iniciando ${session.title}` : `Jogando ${session.title}`}</h1>
        {launching ? (
          <div class="session-spinner" />
        ) : (
          <p class="muted">Segure os dois analógicos pressionados (L3 + R3) para abrir o menu</p>
        )}
      </div>
      <Hints items={launching ? [["back", "Cancelar"]] : [["confirm", "Voltar ao jogo"], ["menu", "Menu"]]} />
    </>
  );
}
