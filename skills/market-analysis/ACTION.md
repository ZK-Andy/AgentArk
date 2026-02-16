---
name: market-analysis
description: Find BSE penny stocks below Rs.500 that could grow 10x in 6/12/24 months. Performs deep analysis with validation searches for each stock.
version: "1.0.0"
metadata:
  emoji: "📈"
  requires:
    tools: ["web_search"]
---

# BSE Penny Stock Analyzer

Find 5 BSE penny stocks (below Rs.500) with potential to blow up in 6-24 months.

## Workflow

### Step 1: Initial Search
Search for penny stock opportunities:
- "BSE penny stocks below 500 multibagger 2026"
- "small cap stocks India high growth potential"
- "emerging sector stocks BSE India"

### Step 2: Select 5 Candidates
Pick stocks based on:
- Price below Rs.500
- Recent positive momentum
- Strong sector tailwinds
- Good trading volume

### Step 3: Deep Validation (FOR EACH STOCK)
Run validation searches:
- "[Company] quarterly results Q3 2025"
- "[Company] promoter holding changes"
- "[Company] order book news"
- "[Company] expansion capex plans"
- "[Company] debt equity ratio"

### Step 4: Verify Analysis
For each stock, confirm:
- Financials are improving (revenue, profit growth)
- Promoter holding stable or increasing
- No red flags (fraud, regulatory issues)
- Clear growth catalyst exists

### Step 5: Price Predictions
Estimate targets based on:
- Current PE vs sector PE
- Revenue growth trajectory
- Peer comparison
- Market sentiment

## Output Format

```
BSE PENNY STOCK ANALYSIS REPORT
Date: [Today]

STOCK 1: [Company Name]
BSE Code: XXXXXX
Current Price: Rs.XXX
Sector: XXX
Market Cap: Rs.XXX Cr

VALIDATION RESULTS:
- Q3 Results: [Good/Bad] - Revenue up X%, Profit up X%
- Promoter Holding: X% (up/down from X%)
- Debt/Equity: X.X (healthy/concerning)
- Recent News: [Summary]

WHY IT CAN BLOW UP:
1. [Catalyst 1]
2. [Catalyst 2]
3. [Catalyst 3]

RISKS:
- [Risk 1]
- [Risk 2]

PRICE TARGETS:
- 6 months: Rs.XXX (X% upside) - [reasoning]
- 12 months: Rs.XXX (X% upside) - [reasoning]
- 24 months: Rs.XXX (X% upside) - [reasoning]

CONFIDENCE: High/Medium/Low

---
[Repeat for all 5 stocks]

SUMMARY TABLE:
| Stock | Price | 6M Target | 12M Target | 24M Target | Confidence |
|-------|-------|-----------|------------|------------|------------|

DISCLAIMER: Educational only. Not investment advice. Do your own research. Consult SEBI-registered advisor before investing.
```
