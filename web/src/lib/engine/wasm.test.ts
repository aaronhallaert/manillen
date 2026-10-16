import { describe, expect, it } from 'vitest';
import { toPlain } from './wasm';

describe('toPlain', () => {
  it('strips proxies so the result survives structured cloning', () => {
    const target = { level: 'hard', nested: { a: [1, 2] } };
    const proxied = new Proxy(target, {
      get(t, k) {
        return Reflect.get(t, k);
      },
    });
    const plain = toPlain([proxied, 7, 'x']);
    expect(plain).toEqual([target, 7, 'x']);
    // structuredClone throws on exotic objects; a plain copy must not throw
    expect(() => structuredClone(plain)).not.toThrow();
  });
});
