import { nav, useActions } from "../nav";

function BusyDialog({ title, message }: { title: string; message?: string }) {
  // Nada a fazer até terminar: voltar não cancela uma remoção pela metade.
  useActions({ back: () => {}, confirm: () => {} });
  return (
    <div class="dialog busy">
      <h1>{title}</h1>
      {message && <p class="muted">{message}</p>}
      <div class="session-bar"><span /></div>
    </div>
  );
}

/** Mostra uma espera bloqueante enquanto `task` roda. */
export async function runBusy<T>(title: string, task: () => Promise<T>, message?: string): Promise<T> {
  nav.push(<BusyDialog title={title} message={message} />, true);
  const depth = nav.depth();
  try {
    return await task();
  } finally {
    // A combinação pode ter voltado à tela inicial no meio do caminho.
    if (nav.depth() === depth) nav.pop();
  }
}
