//! Web Search Actions
//!
//! Supports multiple search backends:
//! - SearXNG (self-hosted, reliable)
//! - Serper API (Google results)
//! - Brave Search API
//! - DuckDuckGo (scraping, no API key needed)

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

/// Search result from any backend
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub title: String,
    pub url: String,
    pub snippet: String,
    pub source: String,
}

/// Search response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResponse {
    pub query: String,
    pub results: Vec<SearchResult>,
    pub backend: String,
}

/// Search backend configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum SearchBackend {
    /// SearXNG self-hosted instance
    SearXNG {
        base_url: String,
    },
    /// Serper API (Google results)
    Serper {
        api_key: String,
    },
    /// Brave Search API
    Brave {
        api_key: String,
    },
    /// DuckDuckGo (no API key, uses HTML scraping)
    DuckDuckGo,
}

/// Web search client
pub struct SearchClient {
    backend: SearchBackend,
    client: reqwest::Client,
}

impl SearchClient {
    pub fn new(backend: SearchBackend) -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
            .build()
            .expect("Failed to create HTTP client");

        Self { backend, client }
    }

    /// Perform a web search
    pub async fn search(&self, query: &str, num_results: usize) -> Result<SearchResponse> {
        match &self.backend {
            SearchBackend::SearXNG { base_url } => {
                self.search_searxng(base_url, query, num_results).await
            }
            SearchBackend::Serper { api_key } => {
                self.search_serper(api_key, query, num_results).await
            }
            SearchBackend::Brave { api_key } => {
                self.search_brave(api_key, query, num_results).await
            }
            SearchBackend::DuckDuckGo => {
                self.search_duckduckgo(query, num_results).await
            }
        }
    }

    /// Search using SearXNG instance
    async fn search_searxng(
        &self,
        base_url: &str,
        query: &str,
        num_results: usize,
    ) -> Result<SearchResponse> {
        #[derive(Deserialize)]
        struct SearXNGResponse {
            results: Vec<SearXNGResult>,
        }

        #[derive(Deserialize)]
        struct SearXNGResult {
            title: String,
            url: String,
            content: Option<String>,
        }

        let url = format!(
            "{}/search?q={}&format=json&categories=general",
            base_url.trim_end_matches('/'),
            urlencoding::encode(query)
        );

        let response: SearXNGResponse = self
            .client
            .get(&url)
            .send()
            .await?
            .json()
            .await?;

        let results = response
            .results
            .into_iter()
            .take(num_results)
            .map(|r| SearchResult {
                title: r.title,
                url: r.url,
                snippet: r.content.unwrap_or_default(),
                source: "searxng".to_string(),
            })
            .collect();

        Ok(SearchResponse {
            query: query.to_string(),
            results,
            backend: "searxng".to_string(),
        })
    }

    /// Search using Serper API (Google results)
    async fn search_serper(
        &self,
        api_key: &str,
        query: &str,
        num_results: usize,
    ) -> Result<SearchResponse> {
        #[derive(Serialize)]
        struct SerperRequest {
            q: String,
            num: usize,
        }

        #[derive(Deserialize)]
        struct SerperResponse {
            organic: Option<Vec<SerperResult>>,
        }

        #[derive(Deserialize)]
        struct SerperResult {
            title: String,
            link: String,
            snippet: Option<String>,
        }

        let request = SerperRequest {
            q: query.to_string(),
            num: num_results,
        };

        let response: SerperResponse = self
            .client
            .post("https://google.serper.dev/search")
            .header("X-API-KEY", api_key)
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await?
            .json()
            .await?;

        let results = response
            .organic
            .unwrap_or_default()
            .into_iter()
            .map(|r| SearchResult {
                title: r.title,
                url: r.link,
                snippet: r.snippet.unwrap_or_default(),
                source: "serper".to_string(),
            })
            .collect();

        Ok(SearchResponse {
            query: query.to_string(),
            results,
            backend: "serper".to_string(),
        })
    }

    /// Search using Brave Search API
    async fn search_brave(
        &self,
        api_key: &str,
        query: &str,
        num_results: usize,
    ) -> Result<SearchResponse> {
        #[derive(Deserialize)]
        struct BraveResponse {
            web: Option<BraveWebResults>,
        }

        #[derive(Deserialize)]
        struct BraveWebResults {
            results: Vec<BraveResult>,
        }

        #[derive(Deserialize)]
        struct BraveResult {
            title: String,
            url: String,
            description: Option<String>,
        }

        let url = format!(
            "https://api.search.brave.com/res/v1/web/search?q={}&count={}",
            urlencoding::encode(query),
            num_results
        );

        let response: BraveResponse = self
            .client
            .get(&url)
            .header("X-Subscription-Token", api_key)
            .header("Accept", "application/json")
            .send()
            .await?
            .json()
            .await?;

        let results = response
            .web
            .map(|w| w.results)
            .unwrap_or_default()
            .into_iter()
            .map(|r| SearchResult {
                title: r.title,
                url: r.url,
                snippet: r.description.unwrap_or_default(),
                source: "brave".to_string(),
            })
            .collect();

        Ok(SearchResponse {
            query: query.to_string(),
            results,
            backend: "brave".to_string(),
        })
    }

    /// Search using DuckDuckGo (HTML scraping - no API key needed)
    async fn search_duckduckgo(
        &self,
        query: &str,
        num_results: usize,
    ) -> Result<SearchResponse> {
        // DuckDuckGo HTML search
        let url = format!(
            "https://html.duckduckgo.com/html/?q={}",
            urlencoding::encode(query)
        );

        let html = self
            .client
            .get(&url)
            .send()
            .await?
            .text()
            .await?;

        // Simple HTML parsing for results
        let mut results = Vec::new();

        // Look for result divs - basic regex-style parsing
        // In production, use a proper HTML parser like scraper
        let mut remaining = html.as_str();

        while results.len() < num_results {
            // Find result link
            let Some(link_start) = remaining.find("class=\"result__a\"") else {
                break;
            };
            remaining = &remaining[link_start..];

            let Some(href_start) = remaining.find("href=\"") else {
                break;
            };
            remaining = &remaining[href_start + 6..];

            let Some(href_end) = remaining.find('"') else {
                break;
            };
            let url = &remaining[..href_end];
            remaining = &remaining[href_end..];

            // Get title
            let Some(title_start) = remaining.find('>') else {
                break;
            };
            remaining = &remaining[title_start + 1..];

            let Some(title_end) = remaining.find("</a>") else {
                break;
            };
            let title = html_decode(&remaining[..title_end]);
            remaining = &remaining[title_end..];

            // Get snippet
            let snippet = if let Some(snippet_start) = remaining.find("class=\"result__snippet\"") {
                let temp = &remaining[snippet_start..];
                if let Some(s_start) = temp.find('>') {
                    let temp = &temp[s_start + 1..];
                    if let Some(s_end) = temp.find("</a>").or_else(|| temp.find("</span>")) {
                        html_decode(&temp[..s_end])
                    } else {
                        String::new()
                    }
                } else {
                    String::new()
                }
            } else {
                String::new()
            };

            // Decode DuckDuckGo redirect URL
            let actual_url = if url.starts_with("//duckduckgo.com/l/") {
                // Extract actual URL from redirect
                if let Some(uddg_start) = url.find("uddg=") {
                    let encoded = &url[uddg_start + 5..];
                    if let Some(end) = encoded.find('&') {
                        urlencoding::decode(&encoded[..end])
                            .map(|s| s.to_string())
                            .unwrap_or_else(|_| url.to_string())
                    } else {
                        urlencoding::decode(encoded)
                            .map(|s| s.to_string())
                            .unwrap_or_else(|_| url.to_string())
                    }
                } else {
                    url.to_string()
                }
            } else {
                url.to_string()
            };

            results.push(SearchResult {
                title,
                url: actual_url,
                snippet,
                source: "duckduckgo".to_string(),
            });
        }

        Ok(SearchResponse {
            query: query.to_string(),
            results,
            backend: "duckduckgo".to_string(),
        })
    }
}

/// Simple HTML entity decoder
fn html_decode(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ")
        .replace("<b>", "")
        .replace("</b>", "")
        .replace("<span>", "")
        .replace("</span>", "")
        .trim()
        .to_string()
}

/// Search action arguments
#[derive(Debug, Deserialize)]
pub struct SearchArgs {
    pub query: String,
    #[serde(default = "default_num_results")]
    pub num_results: usize,
    #[serde(default)]
    pub backend: Option<String>,
}

fn default_num_results() -> usize {
    5
}

/// Execute a web search
pub async fn execute_search(
    args: &SearchArgs,
    config: &SearchConfig,
) -> Result<String> {
    let backend = match args.backend.as_deref() {
        Some("searxng") => config.searxng.clone().ok_or_else(|| {
            anyhow!("SearXNG not configured")
        })?,
        Some("serper") => config.serper.clone().ok_or_else(|| {
            anyhow!("Serper not configured")
        })?,
        Some("brave") => config.brave.clone().ok_or_else(|| {
            anyhow!("Brave not configured")
        })?,
        Some("duckduckgo") | None => SearchBackend::DuckDuckGo,
        Some(other) => return Err(anyhow!("Unknown search backend: {}", other)),
    };

    let client = SearchClient::new(backend);
    let response = client.search(&args.query, args.num_results).await?;

    // Format results
    let mut output = format!("Search results for: {}\n\n", response.query);
    for (i, result) in response.results.iter().enumerate() {
        output.push_str(&format!(
            "{}. {}\n   {}\n   {}\n\n",
            i + 1,
            result.title,
            result.url,
            result.snippet
        ));
    }

    Ok(output)
}

/// Search configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SearchConfig {
    pub searxng: Option<SearchBackend>,
    pub serper: Option<SearchBackend>,
    pub brave: Option<SearchBackend>,
}
