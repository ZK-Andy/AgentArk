export function table(el, columns, rows) {
  const thead = columns.map((column) => `<th scope="col">${column}</th>`).join("");
  const tbody = rows
    .map((row) => `<tr>${columns.map((column) => `<td>${row[column] ?? ""}</td>`).join("")}</tr>`)
    .join("");
  el.innerHTML = `<table><thead><tr>${thead}</tr></thead><tbody>${tbody}</tbody></table>`;
}

export function render(el) {
  table(el, ["Name", "Status"], [{ Name: "Orbit", Status: "Ready" }]);
}
