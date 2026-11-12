import "@fontsource/figtree/400.css";
import "@fontsource/figtree/600.css";
import "@fontsource/fraunces/600.css";
import { render } from "preact";
import { useEffect, useState } from "preact/hooks";
import { call, on, SessionInfo } from "./api";
import { showError } from "./components/Dialog";
import { BatteryToastCard, DownloadToastCard, showBatteryToast, showDownloadToast, ToastHost } from "./components/Toast";
import { startInput } from "./input";
import { nav, NavRoot, setAnimationsEnabled } from "./nav";
import { DesktopOverlay } from "./screens/DesktopOverlay";
import { addCompleted } from "./screens/Downloads";
import { GameSession } from "./screens/GameSession";
import { Home } from "./screens/Home";
import { promptImport } from "./screens/ImportScreen";
import { openQuickMenu } from "./screens/QuickMenu";
import "./styles.css";

const isOverlay = location.hash === "#overlay";
if (isOverlay) document.documentElement.classList.add("overlay-window");
/** Janela do aviso por cima do jogo: `#toast-15` (bateria) ou `#done-<nome do jogo>` (download). */
const toastPercent = /^#toast-(\d+)$/.exec(location.hash)?.[1];
const doneTitle = /^#done-(.+)$/.exec(location.hash)?.[1];

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
    void on<number>("battery-low", showBatteryToast);
    void on<{ id: string; title: string }>("download-done", (done) => {
      addCompleted(done.id, done.title);
      showDownloadToast(done.title);
    });
  }, []);
  return session ? <GameSession session={session} /> : <Home />;
}

function App() {
  useEffect(() => {
    if (isOverlay) {
      void startInput(() => {}, false);
      return;
    }
    void call<boolean>("reduce_animations_get").then((reduce) => setAnimationsEnabled(!reduce));
    void startInput(async () => {
      // Enquanto o jogo abre, a tela só oferece cancelar.
      const session = await call<SessionInfo | null>("session_info");
      if (session?.phase === "launching") return;
      // A combinação sempre mostra o menu a partir da base, seja qual for a tela aberta.
      nav.reset();
      void openQuickMenu();
    });
  }, []);
  return (
    <>
      <NavRoot root={isOverlay ? <DesktopOverlay /> : <Root />} />
      {!isOverlay && <ToastHost />}
    </>
  );
}

if (toastPercent !== undefined || doneTitle !== undefined) {
  document.documentElement.classList.add("toast-window");
  render(
    doneTitle !== undefined
      ? <DownloadToastCard title={decodeURIComponent(doneTitle)} />
      : <BatteryToastCard percent={Number(toastPercent)} />,
    document.getElementById("app")!,
  );
} else {
  render(<App />, document.getElementById("app")!);
}
