// Search view for web_search / search_files / lookup-style steps.

import Box from "@mui/material/Box";
import Typography from "@mui/material/Typography";
import SearchRoundedIcon from "@mui/icons-material/SearchRounded";

import type { ChatStepCard } from "../types";

export interface SearchViewProps {
  card: ChatStepCard;
}

function splitResults(body: string): string[] {
  if (!body) return [];
  const parts = body
    .split(/\r?\n\r?\n+|^\s*\d+[.)]\s+/m)
    .map((s) => s.trim())
    .filter(Boolean);
  return parts.slice(0, 12);
}

function pickBody(card: ChatStepCard): string {
  return (
    card.payloadView?.body ||
    card.rawDetailFull ||
    card.detailFull ||
    ""
  );
}

export function SearchView({ card }: SearchViewProps) {
  const body = pickBody(card);
  const query =
    (card.detail || "").split(/\r?\n/)[0]?.trim() ||
    card.summary ||
    card.label;
  const results = splitResults(body);
  return (
    <Box className="cview cview-search">
      <Box className="cview-search-head">
        <SearchRoundedIcon className="cview-search-icon" aria-hidden="true" />
        <span className="cview-search-query" title={query}>
          {query}
        </span>
      </Box>
      {results.length > 0 ? (
        <ol className="cview-search-results">
          {results.map((entry, idx) => (
            <li key={idx} className="cview-search-result">
              <pre className="cview-search-result-body">{entry}</pre>
            </li>
          ))}
        </ol>
      ) : (
        <Typography variant="body2" className="cview-search-empty">
          No results captured for this query.
        </Typography>
      )}
    </Box>
  );
}

export default SearchView;
