export function render(el, ctx = {}) {
  const html = ctx.html || "<main><h1>Empty HTML widget</h1></main>";
  el.innerHTML = html;
}
