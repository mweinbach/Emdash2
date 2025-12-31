import { Ghostty, init } from 'ghostty-web';

let ghosttyInstance: Ghostty | null = null;
let ghosttyLoadPromise: Promise<Ghostty | null> | null = null;

export async function loadGhostty(): Promise<Ghostty | null> {
  if (ghosttyInstance) return ghosttyInstance;
  if (!ghosttyLoadPromise) {
    const wasmUrl = new URL('../../assets/wasm/ghostty-vt.wasm', import.meta.url).toString();
    ghosttyLoadPromise = Ghostty.load(wasmUrl)
      .then((ghostty) => {
        ghosttyInstance = ghostty;
        return ghostty;
      })
      .catch(async (error) => {
        ghosttyLoadPromise = null;
        console.warn('[ghostty] failed to load custom wasm, falling back', error);
        await init();
        return null;
      });
  }
  return ghosttyLoadPromise;
}

export function getGhostty(): Ghostty | null {
  return ghosttyInstance;
}
