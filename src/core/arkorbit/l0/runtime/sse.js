export function connectOrbitEvents(orbitId, onEvent) {
  if (!orbitId || typeof EventSource === "undefined") return null;
  const source = new EventSource(
    `/api/arkorbit/orbits/${encodeURIComponent(orbitId)}/events`,
  );
  source.addEventListener("file_changed", (event) => {
    if (typeof onEvent === "function") onEvent(event);
  });
  return source;
}
