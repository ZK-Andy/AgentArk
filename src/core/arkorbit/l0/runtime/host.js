(function () {
  const pending = new Map();
  let nextRequestId = 1;
  let readyAcked = false;

  function currentOrbitId() {
    if (typeof window.__ARKORBIT_ORBIT_ID === "string") {
      return window.__ARKORBIT_ORBIT_ID;
    }
    const pathMatch = window.location.pathname.match(
      /\/api\/arkorbit\/orbits\/([^/]+)\/index$/,
    );
    if (pathMatch) {
      return decodeURIComponent(pathMatch[1]);
    }
    const params = new URLSearchParams(window.location.search);
    return params.get("orbit_id") || "";
  }

  function postToParent(message) {
    window.parent.postMessage({ arkorbit: message }, "*");
  }

  window.addEventListener("message", (event) => {
    const envelope = event.data && event.data.arkorbit;
    if (envelope && envelope.kind === "runtime_ack") {
      readyAcked = true;
      return;
    }
    if (!envelope || envelope.kind !== "resolve_result") return;
    const waiter = pending.get(envelope.requestId);
    if (!waiter) return;
    pending.delete(envelope.requestId);
    if (envelope.ok) {
      waiter.resolve({
        content: typeof envelope.content === "string" ? envelope.content : "",
        contentType:
          typeof envelope.contentType === "string"
            ? envelope.contentType
            : "text/javascript",
      });
    } else {
      waiter.reject(new Error(envelope.error || "Module resolve failed"));
    }
  });

  async function resolveText(path) {
    const requestId = String(nextRequestId++);
    const promise = new Promise((resolve, reject) => {
      pending.set(requestId, { resolve, reject });
      window.setTimeout(() => {
        const waiter = pending.get(requestId);
        if (!waiter) return;
        pending.delete(requestId);
        waiter.reject(new Error("Module resolve timed out"));
      }, 15000);
    });
    postToParent({
      kind: "resolve",
      requestId,
      orbitId: currentOrbitId(),
      path,
    });
    return promise;
  }

  async function importMod(path) {
    const resolved = await resolveText(path);
    const blob = new Blob([resolved.content], {
      type: resolved.contentType || "text/javascript",
    });
    const url = URL.createObjectURL(blob);
    try {
      return await import(url);
    } finally {
      URL.revokeObjectURL(url);
    }
  }

  async function mount(modName, selector) {
    const target = document.querySelector(selector || "#app");
    if (!target) throw new Error("Mount target not found");
    const mod = await importMod(`${modName}/index.js`);
    if (typeof mod.render === "function") {
      await mod.render(target, {
        orbitId: currentOrbitId(),
        importMod,
        resolveText,
      });
    }
    return mod;
  }

  function announceReady() {
    if (readyAcked) return;
    postToParent({ kind: "runtime_ready", orbitId: currentOrbitId() });
  }

  window.__arkorbit = {
    orbitId: currentOrbitId(),
    resolveText,
    importMod,
    mount,
  };

  announceReady();
  window.setTimeout(announceReady, 0);
  window.addEventListener("DOMContentLoaded", announceReady, { once: true });
  window.addEventListener("load", announceReady, { once: true });

  let readyAttempts = 0;
  const readyTimer = window.setInterval(() => {
    if (readyAcked || readyAttempts >= 20) {
      window.clearInterval(readyTimer);
      return;
    }
    readyAttempts += 1;
    announceReady();
  }, 250);
})();
