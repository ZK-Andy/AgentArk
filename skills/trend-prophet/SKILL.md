---
name: trend-prophet
description: Analyze latest AI research from arxiv and generate 3/6/12 month predictions with LinkedIn post
version: "1.0.0"
metadata:
  emoji: "🔮"
  requires:
    tools: ["web_search"]
---

# Trend Prophet - AI Research Trend Analyzer

Analyze the latest AI research from arxiv and generate predictions for the next 3, 6, and 12 months, along with a ready-to-post LinkedIn article.

## What this action does

1. **Search** arxiv for latest AI/ML research papers (past 7-30 days)
2. **Identify** emerging trends, breakthrough techniques, and rising research areas
3. **Analyze** which trends will likely dominate in 3/6/12 months
4. **Generate** a professional LinkedIn post summarizing the insights

## Execution Steps

### Step 1: Research Scan
Search for:
- "arxiv AI machine learning latest papers 2026"
- "arxiv transformer LLM research February 2026"
- "arxiv breakthrough AI papers this week"
- "AI research trends 2026 emerging"

### Step 2: Identify Key Trends
From search results, identify:
- New architectures or techniques gaining traction
- Problems being solved in novel ways
- Cross-domain applications emerging
- Open-source releases and benchmarks

### Step 3: Trend Analysis
For each identified trend:
- Current state of research
- Key papers and researchers
- Industry adoption signals
- Potential impact timeline

### Step 4: Predictions
Generate predictions for:
- **3 months**: What will be hot topics at next major conference
- **6 months**: What techniques will enter production systems
- **12 months**: What paradigm shifts may occur

### Step 5: LinkedIn Post
Create a professional LinkedIn post that:
- Opens with a hook/insight
- Summarizes 3-5 key trends
- Includes predictions
- Ends with call-to-action
- Uses appropriate hashtags
- Is 800-1200 characters

## Output Format

```
# 🔮 AI Trend Prophet Report
Generated: [Date]

## Executive Summary
[2-3 sentences on overall AI research direction]

---

## Top 5 Emerging Trends

### 1. [Trend Name]
**What it is**: [Brief explanation]
**Key papers**: [arxiv links/titles]
**Why it matters**: [Impact explanation]
**Adoption timeline**: [Estimate]

### 2. [Trend Name]
...

---

## Predictions

### 3-Month Outlook
- [Prediction 1]
- [Prediction 2]
- [Prediction 3]

### 6-Month Outlook
- [Prediction 1]
- [Prediction 2]
- [Prediction 3]

### 12-Month Outlook
- [Prediction 1]
- [Prediction 2]
- [Prediction 3]

---

## LinkedIn Post (Ready to Copy)

[Hook sentence that grabs attention]

I've been analyzing the latest AI research from arxiv, and here's what's coming:

🔹 [Trend 1]: [One-liner]
🔹 [Trend 2]: [One-liner]
🔹 [Trend 3]: [One-liner]

My predictions:
📅 3 months: [Key prediction]
📅 6 months: [Key prediction]
📅 12 months: [Key prediction]

The future of AI is being written right now in research labs. Which trend excites you most?

#AI #MachineLearning #ArtificialIntelligence #TechTrends #Innovation #FutureOfAI

---

## Sources
[List of arxiv papers and articles referenced]
```

## Tips for Best Results

- Run weekly to stay current
- Cross-reference with Papers With Code trending
- Check Google Scholar citations for validation
- Follow key researchers on Twitter/X for context
