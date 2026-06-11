export function shouldRenderFileAsMarkdown(path = "", _content = ""): boolean {
  const normalizedPath = path.trim().replace(/\\/g, "/").split(/[?#]/, 1)[0] || "";
  const fileName = normalizedPath.slice(normalizedPath.lastIndexOf("/") + 1);
  return /\.(md|markdown)$/i.test(fileName);
}
