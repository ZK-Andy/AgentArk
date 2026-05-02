export function render(el) {
  const state = { items: ["Plan", "Build", "Review"] };
  const paint = () => {
    el.innerHTML = `
      <form class="todo-form">
        <input aria-label="New item" name="item" />
        <button type="submit">Add</button>
      </form>
      <ul>${state.items.map((item) => `<li>${item}</li>`).join("")}</ul>
    `;
    el.querySelector("form").addEventListener("submit", (event) => {
      event.preventDefault();
      const input = event.currentTarget.elements.item;
      const value = input.value.trim();
      if (value) state.items.push(value);
      input.value = "";
      paint();
    });
  };
  paint();
}
