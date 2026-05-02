import { createElement, type ReactNode } from "react";

// Lightweight per-line syntax highlighter, ported from the in-house tokenizer
// already used by `renderChatMarkdown` in `pages/ChatPage.tsx`. Kept regex /
// per-line so it works on streamed partial content and tiny snippets without
// dragging in Prism / Shiki / react-syntax-highlighter.
//
// The token shape matches what the existing chat code-block CSS expects
// (`code-token-keyword` etc. classes), so styling stays consistent across the
// chat bubble and the right-pane FileView / AppDeployView.

export type CodeLanguage =
  | "markup"
  | "script"
  | "css"
  | "json"
  | "python"
  | "sql"
  | "shell"
  | "markdown"
  | "config"
  | "text";

export type CodeToken = {
  text: string;
  className?: string;
};

export function guessCodeLanguage(fileName = "", content = ""): CodeLanguage {
  const normalizedName = fileName.trim().toLowerCase();
  if (/\.(html?|xml|svg)$/.test(normalizedName)) return "markup";
  if (/\.(css|scss|less)$/.test(normalizedName)) return "css";
  if (/\.(json)$/.test(normalizedName)) return "json";
  if (/\.(py|pyw)$/.test(normalizedName)) return "python";
  if (/\.(sql)$/.test(normalizedName)) return "sql";
  if (/\.(sh|bash|zsh|fish|ps1)$/.test(normalizedName)) return "shell";
  if (/\.(md|markdown)$/.test(normalizedName)) return "markdown";
  if (/\.(ya?ml|toml|ini|env)$/.test(normalizedName)) return "config";
  if (
    /\.(js|jsx|ts|tsx|mjs|cjs|java|kt|go|rs|php|rb|c|cc|cpp|cs)$/.test(
      normalizedName,
    )
  ) {
    return "script";
  }
  const trimmed = content.trim();
  if (!trimmed) return "text";
  if (
    trimmed.startsWith("<!DOCTYPE") ||
    trimmed.startsWith("<html") ||
    /^<[\w-]+/.test(trimmed)
  )
    return "markup";
  if (trimmed.startsWith("{") || trimmed.startsWith("[")) return "json";
  if (/^\s*#\s/.test(trimmed) || /^\s*[-*+]\s/.test(trimmed)) return "markdown";
  if (/^\s*(def |class |import |from )/.test(trimmed)) return "python";
  if (
    /^\s*SELECT\b|^\s*WITH\b|^\s*INSERT\b|^\s*UPDATE\b|^\s*CREATE\b/i.test(
      trimmed,
    )
  )
    return "sql";
  if (/^\s*(const |let |var |function |import |export )/.test(trimmed))
    return "script";
  return "text";
}

function tokenizeByPattern(
  line: string,
  pattern: RegExp,
  classify: (value: string) => string | undefined,
): CodeToken[] {
  const tokens: CodeToken[] = [];
  let lastIndex = 0;
  pattern.lastIndex = 0;
  for (const match of line.matchAll(pattern)) {
    const value = match[0];
    const start = match.index ?? 0;
    if (start > lastIndex) {
      tokens.push({ text: line.slice(lastIndex, start) });
    }
    tokens.push({ text: value, className: classify(value) });
    lastIndex = start + value.length;
  }
  if (lastIndex < line.length) {
    tokens.push({ text: line.slice(lastIndex) });
  }
  return tokens.length > 0 ? tokens : [{ text: line }];
}

function highlightMarkupLine(line: string): CodeToken[] {
  return tokenizeByPattern(
    line,
    /<!--.*?-->|<\/?[A-Za-z][\w:-]*|\/?>|[A-Za-z_:][-A-Za-z0-9_:.]*(?==)|"(?:[^"\\]|\\.)*"|'(?:[^'\\]|\\.)*'/g,
    (value) => {
      if (value.startsWith("<!--")) return "comment";
      if (
        value.startsWith("</") ||
        value.startsWith("<") ||
        value === "/>" ||
        value === ">"
      )
        return "tag";
      if (value.startsWith('"') || value.startsWith("'")) return "string";
      return "attr";
    },
  );
}

function highlightCssLine(line: string): CodeToken[] {
  return tokenizeByPattern(
    line,
    /\/\*.*?\*\/|@[A-Za-z-]+|--?[\w-]+(?=\s*:)|#[0-9a-fA-F]{3,8}\b|\b\d+(?:\.\d+)?(?:px|rem|em|vh|vw|%|s|ms|deg)?\b|"(?:[^"\\]|\\.)*"|'(?:[^'\\]|\\.)*'|[{}():;,.]/g,
    (value) => {
      if (value.startsWith("/*")) return "comment";
      if (value.startsWith("@")) return "keyword";
      if (value.startsWith("--") || /^[A-Za-z-]+$/.test(value)) return "attr";
      if (value.startsWith("#")) return "number";
      if (value.startsWith('"') || value.startsWith("'")) return "string";
      if (/^\d/.test(value)) return "number";
      return "punctuation";
    },
  );
}

function highlightJsonLine(line: string): CodeToken[] {
  return tokenizeByPattern(
    line,
    /"(?:[^"\\]|\\.)*"(?=\s*:)|"(?:[^"\\]|\\.)*"|\b(?:true|false|null)\b|-?\b\d+(?:\.\d+)?\b|[{}\[\],:]/g,
    (value) => {
      if (value.startsWith('"')) return value.endsWith(":") ? "attr" : "string";
      if (/^(true|false|null)$/.test(value)) return "keyword";
      if (/^-?\d/.test(value)) return "number";
      return "punctuation";
    },
  );
}

function highlightPythonLine(line: string): CodeToken[] {
  return tokenizeByPattern(
    line,
    /#.*$|"(?:[^"\\]|\\.)*"|'(?:[^'\\]|\\.)*'|\b(?:def|class|import|from|return|if|elif|else|for|while|try|except|finally|with|as|pass|break|continue|lambda|yield|async|await|True|False|None|in|is|and|or|not)\b|\b(?:print|len|range|dict|list|set|tuple|str|int|float|bool)\b|-?\b\d+(?:\.\d+)?\b/g,
    (value) => {
      if (value.startsWith("#")) return "comment";
      if (value.startsWith('"') || value.startsWith("'")) return "string";
      if (/^-?\d/.test(value)) return "number";
      if (
        /^(print|len|range|dict|list|set|tuple|str|int|float|bool)$/.test(value)
      )
        return "builtin";
      return "keyword";
    },
  );
}

function highlightSqlLine(line: string): CodeToken[] {
  return tokenizeByPattern(
    line,
    /--.*$|"(?:[^"\\]|\\.)*"|'(?:[^'\\]|\\.)*'|\b(?:SELECT|FROM|WHERE|GROUP BY|ORDER BY|INSERT|INTO|VALUES|UPDATE|SET|DELETE|CREATE|TABLE|JOIN|LEFT|RIGHT|INNER|OUTER|ON|AND|OR|NOT|NULL|AS|LIMIT|OFFSET|WITH|UNION|DISTINCT)\b|-?\b\d+(?:\.\d+)?\b/gi,
    (value) => {
      if (value.startsWith("--")) return "comment";
      if (value.startsWith('"') || value.startsWith("'")) return "string";
      if (/^-?\d/.test(value)) return "number";
      return "keyword";
    },
  );
}

function highlightShellLine(line: string): CodeToken[] {
  return tokenizeByPattern(
    line,
    /#.*$|"(?:[^"\\]|\\.)*"|'(?:[^'\\]|\\.)*'|\$(?:\w+|{[^}]+})|\b(?:if|then|else|fi|for|do|done|case|esac|export|sudo|echo|cd|ls|cat|grep|find|curl|npm|node|python|pip|cargo|git|docker)\b|-?\b\d+(?:\.\d+)?\b/g,
    (value) => {
      if (value.startsWith("#")) return "comment";
      if (value.startsWith('"') || value.startsWith("'")) return "string";
      if (value.startsWith("$")) return "builtin";
      if (/^-?\d/.test(value)) return "number";
      return "keyword";
    },
  );
}

function highlightMarkdownLine(line: string): CodeToken[] {
  return tokenizeByPattern(
    line,
    /^#{1,6}\s.*$|^\s*[-*+]\s.*$|`[^`]+`|\*\*[^*]+\*\*|__[^_]+__|\[[^\]]+\]\([^)]+\)/g,
    (value) => {
      if (value.startsWith("#")) return "keyword";
      if (/^\s*[-*+]\s/.test(value)) return "punctuation";
      if (value.startsWith("`")) return "string";
      if (
        value.startsWith("[") ||
        value.startsWith("**") ||
        value.startsWith("__")
      )
        return "builtin";
      return undefined;
    },
  );
}

function highlightConfigLine(line: string): CodeToken[] {
  return tokenizeByPattern(
    line,
    /#.*$|;.*$|[A-Za-z_][\w.-]*(?=\s*[:=])|"(?:[^"\\]|\\.)*"|'(?:[^'\\]|\\.)*'|\b(?:true|false|null)\b|-?\b\d+(?:\.\d+)?\b/g,
    (value) => {
      if (value.startsWith("#") || value.startsWith(";")) return "comment";
      if (value.startsWith('"') || value.startsWith("'")) return "string";
      if (/^(true|false|null)$/i.test(value)) return "keyword";
      if (/^-?\d/.test(value)) return "number";
      return "attr";
    },
  );
}

function highlightScriptLine(line: string): CodeToken[] {
  return tokenizeByPattern(
    line,
    /\/\/.*$|\/\*.*?\*\/|"(?:[^"\\]|\\.)*"|'(?:[^'\\]|\\.)*'|`(?:[^`\\]|\\.)*`|\b(?:const|let|var|function|return|if|else|for|while|switch|case|break|continue|async|await|class|new|import|export|from|try|catch|finally|throw|extends|implements|interface|type|public|private|protected|static|readonly|true|false|null|undefined)\b|\b(?:document|window|fetch|console|Math|Date|Promise|JSON|Array|Object|String|Number|Boolean|DOMParser|setTimeout|setInterval|clearTimeout|clearInterval)\b|=>|-?\b\d+(?:\.\d+)?\b/g,
    (value) => {
      if (value.startsWith("//") || value.startsWith("/*")) return "comment";
      if (
        value.startsWith('"') ||
        value.startsWith("'") ||
        value.startsWith("`")
      )
        return "string";
      if (value === "=>") return "operator";
      if (/^-?\d/.test(value)) return "number";
      if (
        /^(document|window|fetch|console|Math|Date|Promise|JSON|Array|Object|String|Number|Boolean|DOMParser|setTimeout|setInterval|clearTimeout|clearInterval)$/.test(
          value,
        )
      ) {
        return "builtin";
      }
      return "keyword";
    },
  );
}

export function highlightCodeLine(
  line: string,
  language: CodeLanguage,
): CodeToken[] {
  switch (language) {
    case "markup":
      return highlightMarkupLine(line);
    case "css":
      return highlightCssLine(line);
    case "json":
      return highlightJsonLine(line);
    case "python":
      return highlightPythonLine(line);
    case "sql":
      return highlightSqlLine(line);
    case "shell":
      return highlightShellLine(line);
    case "markdown":
      return highlightMarkdownLine(line);
    case "config":
      return highlightConfigLine(line);
    case "script":
      return highlightScriptLine(line);
    default:
      return [{ text: line }];
  }
}

export function renderCodeBlockLines(
  content: string,
  options?: {
    fileName?: string;
    startLine?: number;
    activeLine?: number | null;
  },
): ReactNode[] {
  const language = guessCodeLanguage(options?.fileName, content);
  const startLine = options?.startLine ?? 1;
  const activeLine = options?.activeLine ?? null;
  return content.split(/\r?\n/).map((line, index) => {
    const lineNumber = startLine + index;
    const tokens = highlightCodeLine(line, language);
    return createElement(
      "span",
      {
        key: `${options?.fileName || "code"}-${lineNumber}`,
        className: `code-line${activeLine === lineNumber ? " code-line-active" : ""}`,
      },
      createElement(
        "span",
        { className: "code-line-number" },
        lineNumber,
      ),
      createElement(
        "span",
        { className: "code-line-content" },
        ...tokens.map((token, tokenIndex) =>
          createElement(
            "span",
            {
              key: `${lineNumber}-${tokenIndex}`,
              className: token.className
                ? `code-token code-token-${token.className}`
                : undefined,
            },
            token.text,
          ),
        ),
      ),
      "\n",
    );
  });
}
