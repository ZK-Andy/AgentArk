export function barChart(el, values, options = {}) {
  const width = options.width || 520;
  const height = options.height || 240;
  const max = Math.max(1, ...values.map((item) => Number(item.value) || 0));
  const barWidth = width / Math.max(1, values.length);
  const bars = values
    .map((item, index) => {
      const value = Number(item.value) || 0;
      const barHeight = Math.round((value / max) * (height - 44));
      const x = index * barWidth + 8;
      const y = height - barHeight - 28;
      const label = String(item.label || index + 1);
      return `<g>
        <rect x="${x}" y="${y}" width="${Math.max(8, barWidth - 16)}" height="${barHeight}" rx="4"></rect>
        <text x="${x}" y="${height - 8}">${label}</text>
      </g>`;
    })
    .join("");
  el.innerHTML = `<svg viewBox="0 0 ${width} ${height}" role="img">${bars}</svg>`;
}

export function render(el) {
  barChart(el, [
    { label: "A", value: 4 },
    { label: "B", value: 7 },
    { label: "C", value: 5 },
  ]);
}
