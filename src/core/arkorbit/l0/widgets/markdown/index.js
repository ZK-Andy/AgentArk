export function render(el) {
  el.innerHTML = `
    <section class="orbit-empty-canvas" aria-label="Empty Orbit canvas">
      <div class="orbit-empty-topline">
        <span>Canvas</span>
        <span>Ready</span>
      </div>
      <div class="orbit-empty-reticle" aria-hidden="true">
        <span></span>
        <span></span>
        <span></span>
        <span></span>
      </div>
      <div class="orbit-empty-rail orbit-empty-rail-left" aria-hidden="true"></div>
      <div class="orbit-empty-rail orbit-empty-rail-bottom" aria-hidden="true"></div>
      <div class="orbit-empty-node orbit-empty-node-a" aria-hidden="true"></div>
      <div class="orbit-empty-node orbit-empty-node-b" aria-hidden="true"></div>
      <div class="orbit-empty-node orbit-empty-node-c" aria-hidden="true"></div>
    </section>
  `;
}
