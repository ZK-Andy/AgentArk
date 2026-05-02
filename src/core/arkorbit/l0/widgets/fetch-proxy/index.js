export async function fetchThroughOrbit(ctx, url, init) {
  if (!ctx || typeof ctx.fetchPublic !== "function") {
    throw new Error("ArkOrbit fetch proxy is unavailable in this context");
  }
  return ctx.fetchPublic(url, init);
}

export async function fetchTextThroughOrbit(ctx, url, init) {
  if (!ctx || typeof ctx.fetchText !== "function") {
    throw new Error("ArkOrbit text fetch helper is unavailable in this context");
  }
  return ctx.fetchText(url, init);
}

export async function fetchJsonThroughOrbit(ctx, url, init) {
  if (!ctx || typeof ctx.fetchJson !== "function") {
    throw new Error("ArkOrbit JSON fetch helper is unavailable in this context");
  }
  return ctx.fetchJson(url, init);
}
