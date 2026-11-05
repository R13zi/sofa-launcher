/**
 * Pilha de telas com as que estão saindo. Uma camada sai da pilha na hora, então os comandos já
 * valem para a de baixo, e só continua desenhada durante a animação de saída.
 */
export interface LayerState<T> {
  stack: T[];
  leaving: T[];
}

export function popLayer<T>({ stack, leaving }: LayerState<T>): LayerState<T> {
  if (stack.length <= 1) return { stack, leaving };
  return { stack: stack.slice(0, -1), leaving: [...leaving, stack[stack.length - 1]] };
}

export function popToBase<T>({ stack, leaving }: LayerState<T>): LayerState<T> {
  return { stack: stack.slice(0, 1), leaving: [...leaving, ...stack.slice(1)] };
}

export function finishLeaving<T>({ stack, leaving }: LayerState<T>, done: T[]): LayerState<T> {
  return { stack, leaving: leaving.filter((layer) => !done.includes(layer)) };
}
