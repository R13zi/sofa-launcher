import { describe, expect, it } from "vitest";
import { finishLeaving, popLayer, popToBase } from "./layerStack";

describe("pilha de telas", () => {
  it("a camada sai da pilha na hora e continua só para a animação", () => {
    const next = popLayer({ stack: ["base", "detalhes"], leaving: [] });
    expect(next.stack).toEqual(["base"]);
    expect(next.leaving).toEqual(["detalhes"]);
    expect(finishLeaving(next, ["detalhes"]).leaving).toEqual([]);
  });

  it("a base nunca sai", () => {
    expect(popLayer({ stack: ["base"], leaving: [] })).toEqual({ stack: ["base"], leaving: [] });
  });

  it("voltar várias vezes rápido não quebra a pilha", () => {
    let state = { stack: ["base", "a", "b", "c"], leaving: [] as string[] };
    for (let i = 0; i < 6; i++) state = popLayer(state);
    expect(state.stack).toEqual(["base"]);
    expect(state.leaving).toEqual(["c", "b", "a"]);
    state = finishLeaving(state, ["c"]);
    expect(state.leaving).toEqual(["b", "a"]);
  });

  it("voltar à base leva todas as outras", () => {
    const next = popToBase({ stack: ["base", "a", "b"], leaving: ["x"] });
    expect(next.stack).toEqual(["base"]);
    expect(next.leaving).toEqual(["x", "a", "b"]);
  });
});
