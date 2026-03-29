---
name: daily-briefing
description: "Comprehensive daily briefing: weather, calendar, tasks, email highlights, and news"
version: "1.0.0"
permissions: [network, research]
metadata:
  emoji: "🌅"
  requires:
    tools: ["web_search", "list_tasks", "gmail_scan"]
---

# Daily Briefing

Generate a comprehensive, personalized daily briefing for the user.

## Workflow

### Step 1: Weather
Search the web for current weather conditions in the user's location (ask if unknown).
Include: temperature, conditions, high/low, precipitation chance.

### Step 2: Calendar Overview
If Google Calendar is connected, list today's events and upcoming meetings.
Highlight any scheduling conflicts or back-to-back meetings.
If not connected, skip this section.

### Step 3: Task Priorities
List pending tasks from the task queue.
Sort by importance/urgency score (Eisenhower matrix).
Highlight Q1 (urgent + important) tasks that need immediate attention.
Summarize Q2 (important, not urgent) tasks for planning.

### Step 4: Email Highlights
If Gmail is connected, scan for unread important emails.
Summarize the top 5 most important unread messages.
Flag any emails that need immediate response.
If not connected, skip this section.

### Step 5: News & Trends
Search for 2-3 relevant news items based on the user's interests and recent conversations.
Keep each summary to 1-2 sentences.

### Step 6: Daily Quote
Include a brief motivational or relevant quote to start the day.

## Output Format

Present the briefing in a clean, structured format:

```
Good morning! Here's your briefing for [date]:

**Weather**: [conditions]

**Today's Schedule**: [events or "No events scheduled"]

**Priority Tasks**:
1. [Q1 task] - URGENT
2. [Q2 task] - Important

**Email Highlights**:
- [sender]: [subject] - [one-line summary]

**In the News**:
- [headline]: [brief summary]

Have a productive day!
```
