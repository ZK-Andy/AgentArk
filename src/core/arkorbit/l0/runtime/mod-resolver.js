export async function importMod(path) {
  if (!window.__arkorbit || typeof window.__arkorbit.importMod !== "function") {
    throw new Error("ArkOrbit runtime host is not loaded");
  }
  return window.__arkorbit.importMod(path);
}

export async function resolveText(path) {
  if (!window.__arkorbit || typeof window.__arkorbit.resolveText !== "function") {
    throw new Error("ArkOrbit runtime host is not loaded");
  }
  return window.__arkorbit.resolveText(path);
}
