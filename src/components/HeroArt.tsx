import { useState } from "preact/hooks";
import { artUrl } from "../api";

/** Fundo das telas: a arte grande do jogo, a capa ampliada e desfocada, ou só a cor de fundo. */
export function HeroArt({ hero, art }: { hero: string | null; art: string | null }) {
  const [broken, setBroken] = useState<string[]>([]);
  const heroUrl = artUrl(hero);
  const artSrc = artUrl(art);
  const useHero = heroUrl && !broken.includes(heroUrl);
  const src = useHero ? heroUrl : artSrc && !broken.includes(artSrc) ? artSrc : null;
  return (
    <div class="hero-art" aria-hidden="true">
      {src && (
        <img
          key={src}
          src={src}
          alt=""
          class={useHero ? "" : "from-cover"}
          onError={() => setBroken((list) => [...list, src])}
        />
      )}
    </div>
  );
}
