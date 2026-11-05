import "@fontsource/figtree/400.css";
import "@fontsource/figtree/600.css";
import "@fontsource/fraunces/600.css";
import { render } from "preact";
import { useEffect, useState } from "preact/hooks";
import { call, on, SessionInfo } from "./api";
import { showError } from "./components/Dialog";
import { startInput } from "./input";
import { nav, NavRoot } from "./nav";
import { DesktopOverlay } from "./screens/DesktopOverlay";
import { GameSession } from "./screens/GameSession";
import { Home } from "./screens/Home";
import { promptImport } from "./screens/ImportScreen";
import { openQuickMenu } from "./screens/QuickMenu";
import "./styles.css";

const isOverlay = location.hash === "#overlay";
if (isOverlay) document.documentElement.classList.add("overlay-window");

/** Tela base: a inicial ou, com um jogo aberto, a de sessão. */
function Root() {
  const [session, setSession] = useState<SessionInfo | null>(null);
  useEffect(() => {
    void call<SessionInfo | null>("session_info").then(setSession);
    void on<SessionInfo | null>("session", (info) => {
      // Fecha telas abertas, como os detalhes do jogo, para a base aparecer.
      nav.reset();
      setSession(info);
    });
    void on<string>("session-failed", showError);
    void on<{ drive: string; count: number }>("import-found", (found) => void promptImport(found).catch(showError));
  }, []);
  return session ? <GameSession session={session} /> : <Home />;
}

function App() {
  useEffect(() => {
    if (isOverlay) {
      void startInput(() => {});
      return;
    }
    void startInput(async () => {
      // Enquanto o jogo abre, a tela só oferece cancelar.
      const session = await call<SessionInfo | null>("session_info");
      if (session?.phase === "launching") return;
      // A combinação sempre mostra o menu a partir da base, seja qual for a tela aberta.
      nav.reset();
      void openQuickMenu();
    });
  }, []);
  return <NavRoot root={isOverlay ? <DesktopOverlay /> : <Root />} />;
}

render(<App />, document.getElementById("app")!);
