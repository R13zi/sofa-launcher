import { useRef, useState } from "preact/hooks";
import { artUrl } from "../api";

interface Shown {
  src: string;
  fromCover: boolean;
}

/**
 * Fundo das telas: o banner do jogo ou, sem nenhum, a capa ampliada e desfocada.
 * A imagem anterior fica por baixo enquanto a nova surge, para a troca não piscar.
 */
export function HeroArt({ hero, art }: { hero: string | null; art: string | null }) {
  const [broken, setBroken] = useState<string[]>([]);
  const heroUrl = artUrl(hero);
  const coverUrl = artUrl(art);
  const useHero = !!heroUrl && !broken.includes(heroUrl);
  const src = useHero ? heroUrl : coverUrl && !broken.includes(coverUrl) ? coverUrl : null;

  const last = useRef<Shown | null>(null);
  const previous = useRef<Shown | null>(null);
  if (src !== (last.current?.src ?? null)) {
    previous.current = last.current;
    last.current = src ? { src, fromCover: !useHero } : null;
  }
  const below = previous.current;

  return (
    <div class="hero-art" aria-hidden="true">
      {below && <img key={below.src} src={below.src} alt="" class={below.fromCover ? "from-cover" : ""} />}
      {src && src !== below?.src && (
        <img
          key={src}
          src={src}
          alt=""
          class={`hero-current ${useHero ? "" : "from-cover"}`}
          onError={() => setBroken((list) => [...list, src])}
        />
      )}
    </div>
  );
}
