//! Embedded Web UI
//!
//! Serves a single-page web application that communicates via HTTP polling.
//! No WebSockets for security - uses standard REST endpoints.

/// The embedded HTML/CSS/JS for the web UI
pub const WEB_UI_HTML: &str = r##"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>CogniArk</title>
    <link rel="icon" type="image/svg+xml" href="/logo.svg">
    <link rel="icon" type="image/png" href="/logo.png">
    <link rel="apple-touch-icon" href="/logo.png">
    <style>
        @import url('https://fonts.googleapis.com/css2?family=Inter:wght@300;400;500;600;700&family=JetBrains+Mono:wght@400;500&display=swap');

        :root {
            --bg-primary: #0a0a0f;
            --bg-secondary: #12121a;
            --bg-tertiary: #1a1a25;
            --bg-card: rgba(26, 26, 37, 0.8);
            --bg-glass: rgba(26, 26, 37, 0.6);
            --text-primary: #e8e8f0;
            --text-secondary: #a0a0b8;
            --text-muted: #606078;
            --accent: #6366f1;
            --accent-glow: rgba(99, 102, 241, 0.4);
            --accent-secondary: #8b5cf6;
            --success: #22c55e;
            --success-glow: rgba(34, 197, 94, 0.3);
            --warning: #f59e0b;
            --error: #ef4444;
            --border: rgba(255, 255, 255, 0.08);
            --border-glow: rgba(99, 102, 241, 0.3);
            --gradient-1: linear-gradient(135deg, #6366f1 0%, #8b5cf6 50%, #a855f7 100%);
            --gradient-2: linear-gradient(135deg, #0ea5e9 0%, #6366f1 100%);
        }

        * {
            margin: 0;
            padding: 0;
            box-sizing: border-box;
        }
        a {
            color: var(--accent);
            text-decoration: underline;
            cursor: pointer;
        }
        a:hover {
            color: var(--accent-secondary);
        }

        body {
            font-family: 'Inter', -apple-system, BlinkMacSystemFont, sans-serif;
            background: var(--bg-primary);
            color: var(--text-primary);
            height: 100vh;
            display: flex;
            flex-direction: column;
            overflow: hidden;
        }

        /* Animated background */
        body::before {
            content: '';
            position: fixed;
            top: 0;
            left: 0;
            right: 0;
            bottom: 0;
            background:
                radial-gradient(ellipse at 20% 20%, rgba(99, 102, 241, 0.15) 0%, transparent 50%),
                radial-gradient(ellipse at 80% 80%, rgba(139, 92, 246, 0.1) 0%, transparent 50%),
                radial-gradient(ellipse at 50% 50%, rgba(14, 165, 233, 0.05) 0%, transparent 70%);
            pointer-events: none;
            z-index: -1;
        }

        /* Header */
        header {
            background: var(--bg-glass);
            backdrop-filter: blur(20px);
            padding: 16px 24px;
            border-bottom: 1px solid var(--border);
            display: flex;
            justify-content: space-between;
            align-items: center;
            position: relative;
            z-index: 100;
        }

        .logo {
            font-size: 1.5em;
            font-weight: 700;
            background: var(--gradient-1);
            -webkit-background-clip: text;
            -webkit-text-fill-color: transparent;
            background-clip: text;
            letter-spacing: -0.5px;
            display: flex;
            align-items: center;
        }
        .logo img, .logo object {
            height: 40px;
            width: 40px;
            margin-right: 10px;
            -webkit-background-clip: unset;
            background-clip: unset;
        }

        .header-actions {
            display: flex;
            align-items: center;
            gap: 16px;
        }

        .status {
            display: flex;
            align-items: center;
            gap: 12px;
        }

        .status-indicator {
            display: flex;
            align-items: center;
            gap: 8px;
            font-size: 0.85em;
            color: var(--text-secondary);
            padding: 6px 12px;
            background: var(--bg-tertiary);
            border-radius: 20px;
            border: 1px solid var(--border);
        }

        .status-dot {
            width: 8px;
            height: 8px;
            border-radius: 50%;
            background: var(--success);
            box-shadow: 0 0 10px var(--success-glow);
            animation: pulse 2s infinite;
        }

        @keyframes pulse {
            0%, 100% { opacity: 1; }
            50% { opacity: 0.6; }
        }

        .status-dot.error {
            background: var(--error);
            box-shadow: 0 0 10px rgba(239, 68, 68, 0.4);
        }

        /* Main container */
        .container {
            display: flex;
            flex: 1;
            overflow: hidden;
        }

        /* Sidebar */
        .sidebar {
            width: 220px;
            background: var(--bg-glass);
            backdrop-filter: blur(20px);
            border-right: 1px solid var(--border);
            display: flex;
            flex-direction: column;
            padding: 16px 0;
        }

        .nav-item {
            padding: 12px 20px;
            margin: 2px 12px;
            cursor: pointer;
            border-radius: 10px;
            transition: all 0.2s ease;
            color: var(--text-secondary);
            font-weight: 500;
            font-size: 0.95em;
            display: flex;
            align-items: center;
            gap: 12px;
        }

        .nav-item:hover {
            background: rgba(99, 102, 241, 0.1);
            color: var(--text-primary);
        }

        .nav-item.active {
            background: var(--gradient-1);
            color: white;
            box-shadow: 0 4px 15px var(--accent-glow);
        }
        .nav-item.disabled {
            opacity: 0.4;
            pointer-events: none;
        }

        .nav-icon {
            font-size: 1.1em;
            width: 20px;
            text-align: center;
        }

        .nav-section {
            padding: 20px 20px 8px;
            font-size: 0.7em;
            text-transform: uppercase;
            color: var(--text-muted);
            letter-spacing: 1.5px;
            font-weight: 600;
        }

        /* Main content */
        .main {
            flex: 1;
            display: flex;
            flex-direction: column;
            overflow: hidden;
            position: relative;
        }

        /* Chat view */
        .chat-container {
            flex: 1;
            display: flex;
            flex-direction: column;
            overflow: hidden;
        }

        .chat-header {
            padding: 16px 24px;
            background: var(--bg-glass);
            backdrop-filter: blur(10px);
            border-bottom: 1px solid var(--border);
            display: flex;
            justify-content: space-between;
            align-items: center;
        }

        .chat-title {
            font-weight: 600;
            font-size: 1.1em;
            color: var(--text-primary);
        }

        .chat-subtitle {
            font-size: 0.85em;
            color: var(--text-muted);
            margin-top: 2px;
        }

        .messages {
            flex: 1;
            overflow-y: auto;
            padding: 24px;
            display: flex;
            flex-direction: column;
            gap: 16px;
        }

        .messages::-webkit-scrollbar {
            width: 6px;
        }

        .messages::-webkit-scrollbar-track {
            background: transparent;
        }

        .messages::-webkit-scrollbar-thumb {
            background: var(--border);
            border-radius: 3px;
        }

        .message {
            max-width: 75%;
            padding: 14px 18px;
            border-radius: 16px;
            line-height: 1.6;
            font-size: 0.95em;
            position: relative;
            animation: messageIn 0.3s ease;
        }

        @keyframes messageIn {
            from {
                opacity: 0;
                transform: translateY(10px);
            }
            to {
                opacity: 1;
                transform: translateY(0);
            }
        }

        .message-wrapper {
            display: flex;
            gap: 12px;
            align-items: flex-start;
            max-width: 85%;
        }

        .message-wrapper.user {
            align-self: flex-end;
            flex-direction: row-reverse;
        }

        .message-wrapper.assistant {
            align-self: flex-start;
        }

        .message-avatar {
            width: 32px;
            height: 32px;
            min-width: 32px;
            border-radius: 50%;
            flex-shrink: 0;
            display: flex;
            align-items: center;
            justify-content: center;
            overflow: hidden;
            font-size: 1.1em;
        }

        .message-avatar.user {
            background: var(--gradient-1);
            color: white;
        }

        .message-avatar.assistant {
            background: var(--bg-tertiary);
            border: 1px solid var(--border);
        }

        .message-avatar img {
            width: 100%;
            height: 100%;
            object-fit: contain;
        }

        .message.user {
            align-self: flex-end;
            background: var(--gradient-1);
            color: white;
            border-bottom-right-radius: 4px;
        }

        .message.assistant {
            align-self: flex-start;
            background: var(--bg-card);
            backdrop-filter: blur(10px);
            border: 1px solid var(--border);
            border-bottom-left-radius: 4px;
        }

        .message.system {
            align-self: center;
            background: rgba(245, 158, 11, 0.1);
            border: 1px solid rgba(245, 158, 11, 0.2);
            color: var(--warning);
            font-size: 0.85em;
            border-radius: 8px;
        }

        .message-time {
            font-size: 0.7em;
            opacity: 0.6;
            margin-top: 6px;
        }

        /* Markdown Styles */
        .md-h2 {
            font-size: 1.4em;
            font-weight: 700;
            margin: 16px 0 8px 0;
            color: var(--text-primary);
            border-bottom: 1px solid var(--border);
            padding-bottom: 6px;
        }

        .md-h3 {
            font-size: 1.2em;
            font-weight: 600;
            margin: 14px 0 6px 0;
            color: var(--text-primary);
        }

        .md-h4 {
            font-size: 1.05em;
            font-weight: 600;
            margin: 12px 0 4px 0;
            color: var(--accent);
        }

        .md-p {
            margin: 8px 0;
            line-height: 1.6;
        }

        .md-hr {
            border: none;
            border-top: 1px solid var(--border);
            margin: 16px 0;
        }

        .md-ul, .md-ol {
            margin: 8px 0;
            padding-left: 24px;
        }

        .md-li, .md-li-num {
            margin: 4px 0;
            line-height: 1.5;
        }

        .md-inline-code {
            background: var(--bg-tertiary);
            padding: 2px 6px;
            border-radius: 4px;
            font-family: 'JetBrains Mono', monospace;
            font-size: 0.9em;
            color: var(--accent);
        }

        .code-block {
            background: var(--bg-primary);
            border: 1px solid var(--border);
            border-radius: 8px;
            padding: 12px 16px;
            margin: 12px 0;
            overflow-x: auto;
            font-family: 'JetBrains Mono', monospace;
            font-size: 0.85em;
            line-height: 1.5;
        }

        .code-block code {
            color: var(--text-secondary);
        }

        .md-table {
            width: 100%;
            border-collapse: collapse;
            margin: 12px 0;
            font-size: 0.9em;
        }

        .md-table th, .md-table td {
            border: 1px solid var(--border);
            padding: 8px 12px;
            text-align: left;
        }

        .md-table th {
            background: var(--bg-tertiary);
            font-weight: 600;
            color: var(--text-primary);
        }

        .md-table tr:nth-child(even) {
            background: rgba(255, 255, 255, 0.02);
        }

        .md-table tr:hover {
            background: rgba(99, 102, 241, 0.1);
        }

        .input-area {
            padding: 20px 24px;
            background: var(--bg-glass);
            backdrop-filter: blur(20px);
            border-top: 1px solid var(--border);
            display: flex;
            gap: 12px;
        }

        .input-wrapper {
            flex: 1;
            position: relative;
        }

        .input-wrapper input {
            width: 100%;
            padding: 14px 20px;
            background: var(--bg-secondary);
            border: 1px solid var(--border);
            border-radius: 12px;
            color: var(--text-primary);
            font-size: 0.95em;
            font-family: inherit;
            transition: all 0.2s ease;
        }

        .input-wrapper input:focus {
            outline: none;
            border-color: var(--accent);
            box-shadow: 0 0 0 3px var(--accent-glow);
        }

        .input-wrapper input::placeholder {
            color: var(--text-muted);
        }

        .btn {
            padding: 14px 24px;
            border: none;
            border-radius: 12px;
            cursor: pointer;
            font-weight: 600;
            font-size: 0.95em;
            font-family: inherit;
            transition: all 0.2s ease;
            display: flex;
            align-items: center;
            gap: 8px;
        }

        .btn-primary {
            background: var(--gradient-1);
            color: white;
            box-shadow: 0 4px 15px var(--accent-glow);
        }

        .btn-primary:hover {
            transform: translateY(-2px);
            box-shadow: 0 6px 20px var(--accent-glow);
        }

        .btn-primary:disabled {
            opacity: 0.5;
            cursor: not-allowed;
            transform: none;
        }

        .spinner {
            display: inline-block;
            width: 14px;
            height: 14px;
            border: 2px solid rgba(255,255,255,0.3);
            border-radius: 50%;
            border-top-color: white;
            animation: spin 0.8s linear infinite;
            margin-right: 6px;
            vertical-align: middle;
        }

        @keyframes spin {
            to { transform: rotate(360deg); }
        }

        .btn:disabled {
            opacity: 0.6;
            cursor: not-allowed;
        }

        .btn-secondary {
            background: var(--bg-tertiary);
            color: var(--text-primary);
            border: 1px solid var(--border);
        }

        .btn-secondary:hover {
            background: var(--bg-card);
            border-color: var(--accent);
        }

        .btn-ghost {
            background: transparent;
            color: var(--text-secondary);
            padding: 8px 12px;
        }

        .btn-ghost:hover {
            background: rgba(99, 102, 241, 0.1);
            color: var(--accent);
        }

        .action-mode-btn {
            border: none;
            transition: all 0.2s;
        }

        .action-mode-btn.active {
            background: var(--accent) !important;
            color: white;
        }

        .btn-icon {
            padding: 10px;
            border-radius: 10px;
        }

        .btn-small {
            padding: 6px 12px;
            font-size: 0.85em;
        }

        /* Modal */
        .modal {
            position: fixed;
            top: 0;
            left: 0;
            right: 0;
            bottom: 0;
            background: rgba(0, 0, 0, 0.8);
            backdrop-filter: blur(10px);
            display: flex;
            align-items: center;
            justify-content: center;
            z-index: 1000;
        }

        .modal-content {
            background: var(--bg-secondary);
            border: 1px solid var(--border);
            border-radius: 16px;
            width: 90%;
            max-width: 600px;
            display: flex;
            flex-direction: column;
            box-shadow: 0 20px 60px rgba(0, 0, 0, 0.5);
        }

        .modal-header {
            padding: 20px 24px;
            border-bottom: 1px solid var(--border);
            display: flex;
            justify-content: space-between;
            align-items: center;
        }

        .modal-header h3 {
            margin: 0;
            background: var(--gradient-1);
            -webkit-background-clip: text;
            -webkit-text-fill-color: transparent;
            background-clip: text;
        }

        .modal-body {
            padding: 24px;
            overflow-y: auto;
        }

        .modal-footer {
            padding: 16px 24px;
            border-top: 1px solid var(--border);
            display: flex;
            justify-content: flex-end;
            gap: 12px;
        }

        /* View styles */
        .view {
            flex: 1;
            overflow-y: auto;
            padding: 24px;
        }

        .view::-webkit-scrollbar {
            width: 6px;
        }

        .view::-webkit-scrollbar-track {
            background: transparent;
        }

        .view::-webkit-scrollbar-thumb {
            background: var(--border);
            border-radius: 3px;
        }

        .view-header {
            margin-bottom: 24px;
        }

        .view h2 {
            font-size: 1.5em;
            font-weight: 700;
            color: var(--text-primary);
            margin-bottom: 8px;
        }

        .view-subtitle {
            color: var(--text-muted);
            font-size: 0.95em;
        }

        .card {
            background: var(--bg-card);
            backdrop-filter: blur(10px);
            border-radius: 16px;
            padding: 20px;
            margin-bottom: 16px;
            border: 1px solid var(--border);
            transition: all 0.2s ease;
        }

        .card:hover {
            border-color: rgba(99, 102, 241, 0.3);
            box-shadow: 0 8px 30px rgba(0, 0, 0, 0.3);
        }

        .card-title {
            font-weight: 600;
            color: var(--text-primary);
            margin-bottom: 8px;
            font-size: 1.05em;
        }
        details[open] .setup-arrow {
            transform: rotate(90deg);
        }
        details summary::-webkit-details-marker { display: none; }

        .card-description {
            font-size: 0.9em;
            color: var(--text-secondary);
            line-height: 1.5;
        }

        .card-meta {
            font-size: 0.8em;
            color: var(--text-muted);
            margin-top: 12px;
            padding-top: 12px;
            border-top: 1px solid var(--border);
        }

        /* Status panel */
        .status-panel {
            display: grid;
            grid-template-columns: repeat(auto-fit, minmax(180px, 1fr));
            gap: 16px;
            margin-bottom: 24px;
        }

        .stat-card {
            background: var(--bg-card);
            backdrop-filter: blur(10px);
            padding: 24px;
            border-radius: 16px;
            border: 1px solid var(--border);
            text-align: center;
            position: relative;
            overflow: hidden;
        }

        .stat-card::before {
            content: '';
            position: absolute;
            top: 0;
            left: 0;
            right: 0;
            height: 3px;
            background: var(--gradient-1);
        }

        .stat-value {
            font-size: 2.5em;
            font-weight: 700;
            background: var(--gradient-1);
            -webkit-background-clip: text;
            -webkit-text-fill-color: transparent;
            background-clip: text;
        }

        .stat-label {
            font-size: 0.85em;
            color: var(--text-muted);
            margin-top: 8px;
            font-weight: 500;
        }

        /* DID display */
        .did-display {
            background: var(--bg-secondary);
            padding: 12px 16px;
            border-radius: 10px;
            font-family: 'JetBrains Mono', monospace;
            font-size: 0.8em;
            word-break: break-all;
            color: var(--text-secondary);
            border: 1px solid var(--border);
        }

        /* Typing indicator */
        .typing {
            display: flex;
            gap: 6px;
            padding: 14px 18px;
        }

        .typing span {
            width: 10px;
            height: 10px;
            background: var(--accent);
            border-radius: 50%;
            animation: typing 1.4s infinite;
        }

        .typing span:nth-child(2) { animation-delay: 0.2s; }
        .typing span:nth-child(3) { animation-delay: 0.4s; }

        @keyframes typing {
            0%, 60%, 100% { transform: translateY(0); opacity: 0.4; }
            30% { transform: translateY(-8px); opacity: 1; }
        }

        /* Form styles */
        .form-group {
            margin-bottom: 20px;
        }

        .form-group label {
            display: block;
            margin-bottom: 8px;
            color: var(--text-secondary);
            font-size: 0.9em;
            font-weight: 500;
        }

        .form-input, .form-select, .form-textarea {
            width: 100%;
            padding: 12px 16px;
            background: var(--bg-secondary);
            border: 1px solid var(--border);
            border-radius: 10px;
            color: var(--text-primary);
            font-size: 0.95em;
            font-family: inherit;
            transition: all 0.2s ease;
        }

        .form-textarea {
            resize: vertical;
            min-height: 100px;
        }

        .form-input:focus, .form-select:focus, .form-textarea:focus {
            outline: none;
            border-color: var(--accent);
            box-shadow: 0 0 0 3px var(--accent-glow);
        }

        /* Override browser autofill light background for ALL inputs */
        input:-webkit-autofill,
        input:-webkit-autofill:hover,
        input:-webkit-autofill:focus,
        input:-webkit-autofill:active,
        textarea:-webkit-autofill,
        textarea:-webkit-autofill:hover,
        textarea:-webkit-autofill:focus,
        textarea:-webkit-autofill:active,
        select:-webkit-autofill,
        select:-webkit-autofill:hover,
        select:-webkit-autofill:focus,
        select:-webkit-autofill:active {
            -webkit-box-shadow: 0 0 0 30px var(--bg-secondary) inset !important;
            -webkit-text-fill-color: var(--text-primary) !important;
            caret-color: var(--text-primary) !important;
            transition: background-color 5000s ease-in-out 0s;
        }

        .input-wrapper input:-webkit-autofill,
        .input-wrapper input:-webkit-autofill:hover,
        .input-wrapper input:-webkit-autofill:focus,
        .input-wrapper input:-webkit-autofill:active {
            -webkit-box-shadow: 0 0 0 30px var(--bg-secondary) inset !important;
            -webkit-text-fill-color: var(--text-primary) !important;
        }

        .form-select {
            cursor: pointer;
            appearance: none;
            background-image: url("data:image/svg+xml,%3csvg xmlns='http://www.w3.org/2000/svg' fill='none' viewBox='0 0 20 20'%3e%3cpath stroke='%236366f1' stroke-linecap='round' stroke-linejoin='round' stroke-width='1.5' d='M6 8l4 4 4-4'/%3e%3c/svg%3e");
            background-position: right 12px center;
            background-repeat: no-repeat;
            background-size: 20px;
            padding-right: 40px;
        }

        .form-hint {
            display: block;
            margin-top: 6px;
            font-size: 0.8em;
            color: var(--text-muted);
        }

        .form-checkbox {
            display: flex;
            align-items: center;
            gap: 10px;
            cursor: pointer;
        }

        .form-checkbox input {
            width: 20px;
            height: 20px;
            accent-color: var(--accent);
            cursor: pointer;
        }

        .alert {
            padding: 14px 18px;
            border-radius: 10px;
            font-size: 0.9em;
            display: flex;
            align-items: center;
            gap: 10px;
        }

        .alert-success {
            background: rgba(34, 197, 94, 0.15);
            border: 1px solid rgba(34, 197, 94, 0.3);
            color: var(--success);
        }

        .alert-error {
            background: rgba(239, 68, 68, 0.15);
            border: 1px solid rgba(239, 68, 68, 0.3);
            color: var(--error);
        }
        .settings-lock-banner {
            margin-top: 12px;
            margin-bottom: 16px;
            padding: 12px 14px;
            border-radius: 8px;
            background: rgba(245, 158, 11, 0.12);
            border: 1px solid rgba(245, 158, 11, 0.35);
            color: var(--warning);
            font-size: 0.9em;
        }

        /* Action badge */
        .action-badge {
            display: inline-flex;
            align-items: center;
            gap: 6px;
            padding: 4px 10px;
            background: rgba(99, 102, 241, 0.15);
            border: 1px solid rgba(99, 102, 241, 0.3);
            border-radius: 6px;
            font-size: 0.8em;
            color: var(--accent);
            font-weight: 500;
        }

        /* Trace view styles */
        .feature-item {
            background: var(--bg-secondary);
            padding: 16px;
            border-radius: 12px;
            border: 1px solid var(--border);
        }

        .feature-icon {
            font-size: 1.5em;
            margin-bottom: 8px;
        }

        .feature-name {
            font-weight: 600;
            color: var(--text-primary);
            margin-bottom: 4px;
        }

        .feature-desc {
            font-size: 0.85em;
            color: var(--text-muted);
            line-height: 1.4;
        }

        .trace-empty {
            color: var(--text-muted);
            text-align: center;
            padding: 24px;
            font-style: italic;
        }

        .trace-step {
            display: flex;
            gap: 12px;
            padding: 12px 0;
            border-bottom: 1px solid var(--border);
        }

        .trace-step:last-child {
            border-bottom: none;
        }

        .trace-icon {
            width: 32px;
            height: 32px;
            border-radius: 8px;
            display: flex;
            align-items: center;
            justify-content: center;
            font-size: 1em;
            flex-shrink: 0;
        }

        .trace-icon.info { background: rgba(99, 102, 241, 0.2); }
        .trace-icon.success { background: rgba(34, 197, 94, 0.2); }
        .trace-icon.warning { background: rgba(245, 158, 11, 0.2); }
        .trace-icon.thinking { background: rgba(139, 92, 246, 0.2); }

        .trace-content {
            flex: 1;
        }

        .trace-title {
            font-weight: 600;
            color: var(--text-primary);
            font-size: 0.95em;
        }

        .trace-detail {
            font-size: 0.85em;
            color: var(--text-secondary);
            margin-top: 4px;
        }

        .trace-time {
            font-size: 0.75em;
            color: var(--text-muted);
            margin-top: 4px;
        }

        .trace-data {
            background: var(--bg-secondary);
            padding: 8px 12px;
            border-radius: 6px;
            font-family: 'JetBrains Mono', monospace;
            font-size: 0.8em;
            margin-top: 8px;
            overflow-x: auto;
            color: var(--text-secondary);
        }

        .proof-item {
            display: flex;
            justify-content: space-between;
            align-items: center;
            padding: 12px 0;
            border-bottom: 1px solid var(--border);
        }

        .proof-item:last-child {
            border-bottom: none;
        }

        .proof-id {
            font-family: 'JetBrains Mono', monospace;
            font-size: 0.8em;
            color: var(--accent);
        }

        .proof-time {
            font-size: 0.8em;
            color: var(--text-muted);
        }

        /* Hidden views */
        .hidden {
            display: none !important;
        }

        /* Responsive */
        @media (max-width: 768px) {
            .sidebar {
                width: 70px;
            }
            .nav-item span,
            .nav-section {
                display: none;
            }
            .nav-item {
                justify-content: center;
                padding: 12px;
            }
            .nav-icon {
                font-size: 1.2em;
            }
            .message {
                max-width: 85%;
            }
            .chat-header {
                padding: 12px 16px;
            }
            .messages {
                padding: 16px;
            }
            .input-area {
                padding: 16px;
            }
        }

        /* Empty state */
        .empty-state {
            text-align: center;
            padding: 48px 24px;
            color: var(--text-muted);
        }

        .empty-state-icon {
            font-size: 3em;
            margin-bottom: 16px;
            opacity: 0.5;
        }

        /* Trace history item styles */
        .trace-history-item:last-child {
            border-bottom: none !important;
        }

        .trace-history-item:hover {
            background: rgba(99, 102, 241, 0.1);
        }

        /* Task result modal backdrop click */
        .modal-backdrop {
            position: fixed;
            top: 0;
            left: 0;
            right: 0;
            bottom: 0;
            background: rgba(0, 0, 0, 0.5);
        }

        /* Code blocks in messages */
        .message code {
            background: rgba(0, 0, 0, 0.3);
            padding: 2px 6px;
            border-radius: 4px;
            font-family: 'JetBrains Mono', monospace;
            font-size: 0.9em;
        }

        .message pre {
            background: rgba(0, 0, 0, 0.3);
            padding: 12px;
            border-radius: 8px;
            overflow-x: auto;
            margin: 8px 0;
        }

        .message pre code {
            background: transparent;
            padding: 0;
        }
    </style>
</head>
<body>
    <header>
        <div class="logo"><img src="/logo.svg" alt="CogniArk" onerror="this.onerror=null; this.src='/logo.png';">CogniArk</div>
        <div class="header-actions">
            <div class="status">
                <div class="status-indicator">
                    <div class="status-dot" id="connectionStatus"></div>
                    <span id="connectionText">Connected</span>
                </div>
            </div>
        </div>
    </header>

    <div class="container">
        <nav class="sidebar">
            <div class="nav-item active" data-view="home" onclick="switchView('home')">
                <span class="nav-icon">🏠</span>
                <span>Home</span>
            </div>
            <div class="nav-item" data-view="chat" onclick="switchView('chat')">
                <span class="nav-icon">💬</span>
                <span>Chat</span>
            </div>
            <div class="nav-item" data-view="tasks" onclick="switchView('tasks')">
                <span class="nav-icon">📋</span>
                <span>Tasks</span>
            </div>
            <div class="nav-item" data-view="actions" onclick="switchView('actions')">
                <span class="nav-icon">⚡</span>
                <span>Actions</span>
            </div>
            <div class="nav-item" data-view="memory" onclick="switchView('memory')">
                <span class="nav-icon">🧠</span>
                <span>Memory</span>
            </div>
            <div class="nav-item" data-view="goals" onclick="switchView('goals')">
                <span class="nav-icon">🎯</span>
                <span>Goals</span>
            </div>
            <div class="nav-item" data-view="telegram" onclick="switchView('telegram')">
                <span class="nav-icon">📱</span>
                <span>Telegram</span>
            </div>
            <div class="nav-section">System</div>
            <div class="nav-item" data-view="trace" onclick="switchView('trace')">
                <span class="nav-icon">🔬</span>
                <span>Trace</span>
            </div>
            <div class="nav-item" data-view="status" onclick="switchView('status')">
                <span class="nav-icon">📊</span>
                <span>Status</span>
            </div>
            <div class="nav-item" data-view="settings" onclick="switchView('settings')">
                <span class="nav-icon">⚙️</span>
                <span>Settings</span>
            </div>
        </nav>

        <main class="main">
            <!-- Home View -->
            <div class="view hidden" id="homeView">
                <div class="view-header" style="display: flex; justify-content: space-between; align-items: center;">
                    <div>
                        <h2>🏠 Daily Brief</h2>
                        <p class="view-subtitle">Your assistant dashboard and daily flow</p>
                    </div>
                    <button class="btn btn-secondary" onclick="loadHome()" style="padding: 8px 16px;">🔄 Refresh</button>
                </div>

                <div class="card">
                    <div class="card-title">🗓️ Today at a Glance</div>
                    <div id="homeSummary" style="margin-top: 12px;">
                        <div class="empty-state">
                            <div class="empty-state-icon">⏳</div>
                            <p>Loading your daily brief...</p>
                        </div>
                    </div>
                </div>

                <div class="card" style="margin-top: 16px;">
                    <div class="card-title">⚡ Quick Actions</div>
                    <div style="display: flex; gap: 12px; flex-wrap: wrap; margin-top: 12px;">
                        <button class="btn btn-primary" onclick="seedQuickTask('Create a daily summary of my tasks and recent activity')">Create Daily Summary</button>
                        <button class="btn" onclick="seedQuickTask('Plan my day based on my priorities and tasks')">Plan My Day</button>
                        <button class="btn" onclick="seedQuickTask('Review outstanding tasks and suggest next steps')">Review Tasks</button>
                    </div>
                </div>

                <div class="card" style="margin-top: 16px;">
                    <div class="card-title">📥 Inbox Highlights</div>
                    <div id="homeInbox" style="margin-top: 12px;"></div>
                </div>

                <div class="card" style="margin-top: 16px;">
                    <div class="card-title">🧾 Latest Receipts</div>
                    <div id="homeReceipts" style="margin-top: 12px;"></div>
                </div>
            </div>

            <!-- Chat View -->
            <div class="chat-container hidden" id="chatView">
                <div class="chat-header">
                    <div>
                        <div class="chat-title">New Conversation</div>
                        <div class="chat-subtitle" id="chatSubtitle">Ask me anything or try an action</div>
                    </div>
                    <button class="btn btn-ghost" onclick="startNewConversation()" title="Start New Conversation">
                        <span style="font-size: 1.2em;">🔄</span> New Chat
                    </button>
                </div>
                <div class="messages" id="messages">
                    <div class="message assistant">
                        <strong>Welcome!</strong> I'm your AI assistant.
                        <br><br>
                        Before we dive in, I'd love to know a bit about you:
                        <br><br>
                        <strong>1.</strong> What's your name?<br>
                        <strong>2.</strong> Where are you located?<br>
                        <strong>3.</strong> What do you mainly want help with?
                        <br><br>
                        <em style="color: var(--text-muted);">You can answer in one line like: "John, New York, coding" or just start chatting!</em>
                        <br><br>
                        <span class="action-badge">💡 Tip: Configure your LLM in Settings first</span>
                    </div>
                </div>
                <div class="input-area">
                    <div class="input-wrapper">
                        <input type="text" id="chatInput" placeholder="Type your message..." autocomplete="off">
                    </div>
                    <button class="btn btn-primary" id="sendBtn">
                        Send →
                    </button>
                </div>
            </div>

            <!-- Actions View -->
            <div class="view hidden" id="actionsView">
                <div class="view-header" style="display: flex; justify-content: space-between; align-items: center;">
                    <div>
                        <h2>⚡ Available Actions</h2>
                        <p class="view-subtitle">AI-powered tools ready to assist you</p>
                    </div>
                    <button class="btn btn-primary" onclick="showCreateActionModal()">+ New Action</button>
                </div>
                <div id="actionsList"></div>
            </div>

            <!-- Action Editor Modal -->
            <div id="actionEditorModal" class="modal" style="display: none;">
                <div class="modal-content" style="max-width: 800px; max-height: 90vh;">
                    <div class="modal-header">
                        <h3 id="actionEditorTitle">Edit Action</h3>
                        <button class="btn btn-small" onclick="closeActionEditor()" style="background: transparent; font-size: 1.2em;">✕</button>
                    </div>

                    <!-- Mode Toggle (only for new actions) -->
                    <div id="actionModeToggle" style="display: none; padding: 0 24px; margin-bottom: 16px;">
                        <div style="display: flex; gap: 8px; background: var(--bg-secondary); border-radius: 8px; padding: 4px;">
                            <button class="btn action-mode-btn active" id="simpleModeBtn" onclick="setActionMode('simple')" style="flex: 1; padding: 8px;">
                                ✨ Simple (AI-Assisted)
                            </button>
                            <button class="btn action-mode-btn" id="advancedModeBtn" onclick="setActionMode('advanced')" style="flex: 1; padding: 8px; background: transparent;">
                                🔧 Advanced
                            </button>
                        </div>
                    </div>

                    <!-- Simple Mode (Guided) -->
                    <div id="simpleMode" class="modal-body" style="flex: 1; display: flex; flex-direction: column;">
                        <div class="form-group">
                            <label>Action Name *</label>
                            <input type="text" id="simpleActionName" class="form-input" placeholder="e.g., market-analysis, daily-summary">
                            <small style="color: var(--text-muted);">Lowercase letters, numbers, and hyphens only</small>
                        </div>
                        <div class="form-group">
                            <label>What does this action do? *</label>
                            <input type="text" id="simpleActionDescription" class="form-input" placeholder="e.g., Analyze stock market trends and provide insights">
                        </div>
                        <div class="form-group" style="flex: 1;">
                            <label>Detailed Instructions</label>
                            <textarea id="simpleActionInstructions" class="form-input" style="min-height: 150px;" placeholder="Describe what the action should do step by step...

Example:
1. Search for latest market news
2. Analyze price trends for major indices
3. Look for significant events affecting markets
4. Generate a summary with key insights"></textarea>
                        </div>
                        <div class="form-group">
                            <label>Example Search Queries (one per line)</label>
                            <textarea id="simpleActionQueries" class="form-input" style="min-height: 80px;" placeholder="latest stock market news
S&P 500 price today
market analysis today"></textarea>
                        </div>
                        <div id="generateStatus" style="margin-top: 8px; padding: 12px; border-radius: 8px; display: none;"></div>
                    </div>

                    <!-- Advanced Mode (Full Editor) -->
                    <div id="advancedMode" class="modal-body" style="flex: 1; display: none; flex-direction: column;">
                        <div class="form-group" id="actionNameGroup" style="display: none;">
                            <label>Action Name</label>
                            <input type="text" id="actionEditorName" class="form-input" placeholder="my-action (lowercase, no spaces)">
                        </div>
                        <div class="form-group" style="flex: 1; display: flex; flex-direction: column;">
                            <label>ACTION.md Content</label>
                            <textarea id="actionEditorContent" class="form-input" style="flex: 1; min-height: 400px; font-family: 'JetBrains Mono', monospace; font-size: 0.9em;" placeholder="---
name: my-action
description: What this action does
version: 1.0.0
---

# My Action

## Workflow

1. First step
2. Second step

## Search Queries
- search query 1
- search query 2"></textarea>
                        </div>
                    </div>

                    <div class="modal-footer" style="display: flex; gap: 12px; justify-content: space-between;">
                        <button id="deleteActionBtn" class="btn" style="background: var(--error);" onclick="deleteAction()">🗑️ Delete</button>
                        <div style="display: flex; gap: 12px;">
                            <button class="btn" onclick="closeActionEditor()">Cancel</button>
                            <button id="generateActionBtn" class="btn btn-primary" onclick="generateActionWithAI()" style="display: none;">🤖 Generate with AI</button>
                            <button id="saveActionBtn" class="btn btn-primary" onclick="saveAction()">💾 Save</button>
                        </div>
                    </div>
                </div>
            </div>

            <!-- Action Preview Modal (after AI generation) -->
            <div id="actionPreviewModal" class="modal" style="display: none;">
                <div class="modal-content" style="max-width: 800px; max-height: 90vh;">
                    <div class="modal-header">
                        <h3>🔍 Review Generated Action</h3>
                        <button class="btn btn-small" onclick="closeActionPreview()" style="background: transparent; font-size: 1.2em;">✕</button>
                    </div>
                    <div class="modal-body" style="flex: 1; display: flex; flex-direction: column;">
                        <div style="background: var(--bg-secondary); padding: 16px; border-radius: 10px; margin-bottom: 16px;">
                            <p style="margin: 0; color: var(--text-muted);">
                                Review the AI-generated action below. You can Accept it, Edit it further, or Cancel.
                            </p>
                        </div>
                        <div class="form-group" style="flex: 1; display: flex; flex-direction: column;">
                            <label>Generated ACTION.md</label>
                            <textarea id="actionPreviewContent" class="form-input" style="flex: 1; min-height: 400px; font-family: 'JetBrains Mono', monospace; font-size: 0.9em;"></textarea>
                        </div>
                    </div>
                    <div class="modal-footer" style="display: flex; gap: 12px; justify-content: flex-end;">
                        <button class="btn" onclick="closeActionPreview()" style="background: var(--error);">❌ Cancel</button>
                        <button class="btn" onclick="editGeneratedAction()">✏️ Edit More</button>
                        <button class="btn btn-primary" onclick="acceptGeneratedAction()">✅ Accept & Save</button>
                    </div>
                </div>
            </div>

            <!-- Tasks View (consolidated: includes inbox, routines, all tasks) -->
            <div class="view hidden" id="tasksView">
                <div class="view-header" style="display: flex; justify-content: space-between; align-items: center;">
                    <div>
                        <h2>📋 Tasks</h2>
                        <p class="view-subtitle">All tasks, approvals, and scheduled routines</p>
                    </div>
                    <button class="btn btn-secondary" onclick="loadTasks()" style="padding: 8px 16px;">🔄 Refresh</button>
                </div>
                <div style="display: flex; gap: 8px; margin-bottom: 16px; flex-wrap: wrap;">
                    <button class="btn btn-small task-filter active" data-filter="all" onclick="filterTasks('all')" style="padding: 6px 14px; border-radius: 20px; font-size: 0.85em;">All</button>
                    <button class="btn btn-small task-filter" data-filter="pending" onclick="filterTasks('pending')" style="padding: 6px 14px; border-radius: 20px; font-size: 0.85em;">Pending</button>
                    <button class="btn btn-small task-filter" data-filter="routines" onclick="filterTasks('routines')" style="padding: 6px 14px; border-radius: 20px; font-size: 0.85em;">Routines</button>
                    <button class="btn btn-small task-filter" data-filter="completed" onclick="filterTasks('completed')" style="padding: 6px 14px; border-radius: 20px; font-size: 0.85em;">Completed</button>
                    <button class="btn btn-small task-filter" data-filter="failed" onclick="filterTasks('failed')" style="padding: 6px 14px; border-radius: 20px; font-size: 0.85em;">Failed</button>
                </div>
                    <div class="card">
                        <div class="card-title">Create New Task</div>
                        <form id="createTaskForm">
                            <div class="form-group">
                                <label>Description</label>
                                <input type="text" id="taskDescription" class="form-input" placeholder="What should the bot do?" required>
                            </div>
                        <div class="form-group">
                            <label>Schedule</label>
                            <select id="taskSchedule" class="form-select" onchange="toggleCustomCron()">
                                <option value="">Run once (now)</option>
                                <option value="*/5 * * * *">Every 5 minutes</option>
                                <option value="*/10 * * * *">Every 10 minutes</option>
                                <option value="*/30 * * * *">Every 30 minutes</option>
                                <option value="0 * * * *">Every hour</option>
                                <option value="0 */2 * * *">Every 2 hours</option>
                                <option value="0 */6 * * *">Every 6 hours</option>
                                <option value="0 9 * * *">Daily at 9 AM</option>
                                <option value="0 21 * * *">Every night at 9 PM</option>
                                <option value="0 0 * * *">Daily at midnight</option>
                                <option value="0 9 * * 1">Weekly (Monday 9 AM)</option>
                                <option value="0 9 * * 1-5">Weekdays at 9 AM</option>
                                <option value="0 9 1 * *">Monthly (1st at 9 AM)</option>
                                <option value="custom">Custom cron expression...</option>
                            </select>
                            <span class="form-hint">Select when this task should run</span>
                        </div>
                            <div class="form-group" id="customCronGroup" style="display: none;">
                                <label>Custom Cron Expression</label>
                                <input type="text" id="taskCustomCron" class="form-input" placeholder="* * * * * (min hour day month weekday)">
                                <span class="form-hint">Format: minute hour day month weekday (e.g., "0 */4 * * *" for every 4 hours)</span>
                            </div>
                        <div class="form-group">
                            <label>Refinement Prompt (Optional)</label>
                            <textarea id="taskRefinePrompt" class="form-input" style="min-height: 80px;" placeholder="Add constraints or preferences (e.g., 'Use web search, summarize in bullets, ask before executing risky steps')"></textarea>
                            <span class="form-hint">This is sent to the planner to refine the task plan</span>
                        </div>
                        <div class="form-group">
                            <label class="form-checkbox">
                                <input type="checkbox" id="taskRequireApproval">
                                <span>Require approval before execution</span>
                            </label>
                        </div>
                        <button type="submit" class="btn btn-primary">Generate Plan</button>
                        </form>
                        <div id="taskStatus" style="margin-top: 12px;"></div>
                    </div>
                <h3 style="margin: 24px 0 16px; color: var(--text-primary);">Scheduled Tasks</h3>
                <div id="tasksList">
                    <div class="empty-state">
                        <div class="empty-state-icon">📭</div>
                        <p>No tasks scheduled yet</p>
                    </div>
                </div>
            </div>

            <!-- Memory View -->
            <div class="view hidden" id="memoryView">
                <div class="view-header" style="display: flex; justify-content: space-between; align-items: center;">
                    <div>
                        <h2>🧠 Memory</h2>
                        <p class="view-subtitle">What the assistant knows about you</p>
                    </div>
                    <button class="btn btn-secondary" onclick="loadMemory()" style="padding: 8px 16px;">🔄 Refresh</button>
                </div>
                <div class="card" id="memoryCard"><div class="trace-empty">Loading memory...</div></div>
            </div>

            <!-- Goals View -->
            <div class="view hidden" id="goalsView">
                <div class="view-header" style="display: flex; justify-content: space-between; align-items: center;">
                    <div>
                        <h2>🎯 Goals</h2>
                        <p class="view-subtitle">Daily focus and recurring outcomes</p>
                    </div>
                    <button class="btn btn-secondary" onclick="loadGoals()" style="padding: 8px 16px;">🔄 Refresh</button>
                </div>
                <div class="card">
                    <div class="card-title">Add Goal</div>
                    <div style="display: flex; gap: 10px; flex-wrap: wrap; margin-top: 12px;">
                        <input type="text" id="goalInput" class="form-input" placeholder="e.g., Ship v1 onboarding flow" style="flex: 1; min-width: 220px;">
                        <button class="btn btn-primary" onclick="addGoal()">Add</button>
                    </div>
                </div>
                <div class="card" style="margin-top: 16px;" id="goalsList"><div class="trace-empty">No goals yet. Add one above.</div></div>
            </div>

            <!-- Task Plan Modal -->
            <div id="taskPlanModal" class="modal" style="display: none;">
                <div class="modal-content" style="max-width: 800px; max-height: 90vh;">
                    <div class="modal-header">
                        <h3>🧠 Review Task Plan</h3>
                        <button class="btn btn-small" onclick="closeTaskPlanModal()" style="background: transparent; font-size: 1.2em;">✖</button>
                    </div>
                    <div class="modal-body" style="flex: 1; display: flex; flex-direction: column;">
                        <div style="background: var(--bg-secondary); padding: 16px; border-radius: 10px; margin-bottom: 16px;">
                            <p style="margin: 0; color: var(--text-muted);">
                                Review and edit the plan below. You can Save to add it to the task queue, or Regenerate with a new prompt.
                            </p>
                        </div>
                        <div class="form-group" style="flex: 1; display: flex; flex-direction: column;">
                            <label>Plan (JSON)</label>
                            <textarea id="taskPlanContent" class="form-input" style="flex: 1; min-height: 400px; font-family: 'JetBrains Mono', monospace; font-size: 0.9em;"></textarea>
                        </div>
                        <div class="form-group">
                            <label>Regenerate Prompt (Optional)</label>
                            <textarea id="taskPlanRegeneratePrompt" class="form-input" style="min-height: 80px;" placeholder="Tell the planner what to change"></textarea>
                        </div>
                        <div id="taskPlanStatus" style="margin-top: 8px;"></div>
                    </div>
                    <div class="modal-footer" style="display: flex; gap: 12px; justify-content: space-between;">
                        <button class="btn" onclick="closeTaskPlanModal()" style="background: var(--error);">Cancel</button>
                        <div style="display: flex; gap: 12px;">
                            <button class="btn" onclick="regenerateTaskPlan()">Regenerate</button>
                            <button class="btn btn-primary" onclick="saveTaskPlan()">Save Task</button>
                        </div>
                    </div>
                </div>
            </div>

            <!-- Task Edit Modal -->
            <div id="taskEditModal" class="modal" style="display: none;">
                <div class="modal-content" style="max-width: 800px; max-height: 90vh;">
                    <div class="modal-header">
                        <h3>✏️ Edit Task</h3>
                        <button class="btn btn-small" onclick="closeTaskEditModal()" style="background: transparent; font-size: 1.2em;">✖</button>
                    </div>
                    <div class="modal-body" style="flex: 1; display: flex; flex-direction: column;">
                        <div class="form-group">
                            <label>Description</label>
                            <input type="text" id="taskEditDescription" class="form-input">
                        </div>
                        <div class="form-group">
                            <label>Cron (optional)</label>
                            <input type="text" id="taskEditCron" class="form-input" placeholder="* * * * *">
                        </div>
                        <div class="form-group" style="flex: 1; display: flex; flex-direction: column;">
                            <label>Plan Arguments (JSON)</label>
                            <textarea id="taskEditArguments" class="form-input" style="flex: 1; min-height: 300px; font-family: 'JetBrains Mono', monospace; font-size: 0.9em;"></textarea>
                        </div>
                        <div id="taskEditStatus" style="margin-top: 8px;"></div>
                    </div>
                    <div class="modal-footer" style="display: flex; gap: 12px; justify-content: space-between;">
                        <button class="btn" onclick="closeTaskEditModal()" style="background: var(--error);">Cancel</button>
                        <button class="btn btn-primary" onclick="saveTaskEdit()">Save Changes</button>
                    </div>
                </div>
            </div>

            <!-- Telegram View -->
            <div class="view hidden" id="telegramView">
                <div class="view-header" style="display: flex; justify-content: space-between; align-items: center;">
                    <div>
                        <h2>📱 Telegram Messages</h2>
                        <p class="view-subtitle">All conversations from Telegram</p>
                    </div>
                    <button class="btn btn-secondary" onclick="loadTelegramMessages()" style="padding: 8px 16px;">🔄 Refresh</button>
                </div>

                <!-- Telegram Status -->
                <div class="card" id="telegramStatusCard">
                    <div class="card-title">📡 Connection Status</div>
                    <div id="telegramStatus" style="padding: 8px 0;">Checking...</div>
                </div>

                <!-- Telegram Messages List -->
                <div class="card">
                    <div class="card-title">💬 Recent Messages</div>
                    <div id="telegramMessagesList" style="max-height: 500px; overflow-y: auto;">
                        <div class="trace-empty">No Telegram messages yet. Send a message via Telegram to see it here.</div>
                    </div>
                </div>
            </div>

            <!-- Trace View - Internal Processing Visibility -->
            <div class="view hidden" id="traceView">
                <div class="view-header" style="display: flex; justify-content: space-between; align-items: center;">
                    <div>
                        <h2>🔬 Activity Log</h2>
                        <p class="view-subtitle">See what CogniArk is doing behind the scenes</p>
                    </div>
                    <button class="btn btn-secondary" onclick="loadTrace()" style="padding: 8px 16px;">🔄 Refresh</button>
                </div>

                <!-- Trace History List -->
                <div class="card">
                    <div class="card-title">📋 Recent Activity (Last 100)</div>
                    <div id="traceHistoryList" style="max-height: 600px; overflow-y: auto;">
                        <div class="trace-empty">No activity recorded yet. Send a message to get started!</div>
                    </div>
                </div>

                <!-- Quick Stats -->
                <div class="card" style="margin-top: 16px;">
                    <div class="card-title">📊 Quick Stats</div>
                    <div style="display: grid; grid-template-columns: repeat(auto-fit, minmax(120px, 1fr)); gap: 12px; margin-top: 12px;">
                        <div style="text-align: center; padding: 12px; background: var(--bg-secondary); border-radius: 8px;">
                            <div style="font-size: 1.5em; font-weight: 700; color: var(--accent);" id="traceCountStat">0</div>
                            <div style="font-size: 0.8em; color: var(--text-muted);">Total Traces</div>
                        </div>
                        <div style="text-align: center; padding: 12px; background: var(--bg-secondary); border-radius: 8px;">
                            <div style="font-size: 1.5em; font-weight: 700; color: var(--success);" id="avgDurationStat">-</div>
                            <div style="font-size: 0.8em; color: var(--text-muted);">Avg Response</div>
                        </div>
                    </div>
                </div>
            </div>

            <!-- Trace Detail Modal -->
            <div id="traceDetailModal" class="modal" style="display: none;">
                <div class="modal-content" style="max-width: 800px; max-height: 90vh;">
                    <div class="modal-header">
                        <h3>🔍 Trace Details</h3>
                        <button class="btn btn-small" onclick="closeTraceDetail()" style="background: transparent; font-size: 1.2em;">✕</button>
                    </div>
                    <div class="modal-body" style="overflow-y: auto; max-height: 70vh;">
                        <!-- Trace Summary -->
                        <div style="background: var(--bg-secondary); padding: 16px; border-radius: 10px; margin-bottom: 16px;">
                            <div style="display: flex; justify-content: space-between; flex-wrap: wrap; gap: 12px;">
                                <div>
                                    <div style="font-size: 0.8em; color: var(--text-muted);">Message</div>
                                    <div id="traceDetailMessage" style="font-weight: 500; margin-top: 4px;">-</div>
                                </div>
                                <div>
                                    <div style="font-size: 0.8em; color: var(--text-muted);">Duration</div>
                                    <div id="traceDetailDuration" style="font-weight: 500; margin-top: 4px; color: var(--accent);">-</div>
                                </div>
                                <div>
                                    <div style="font-size: 0.8em; color: var(--text-muted);">Channel</div>
                                    <div id="traceDetailChannel" style="font-weight: 500; margin-top: 4px;">-</div>
                                </div>
                            </div>
                        </div>

                        <!-- Steps Timeline -->
                        <div style="margin-bottom: 16px;">
                            <div style="font-weight: 600; margin-bottom: 12px; color: var(--text-primary);">⏱️ Execution Steps</div>
                            <div id="traceDetailSteps" style="border-left: 2px solid var(--accent); padding-left: 16px; margin-left: 8px;">
                                <!-- Steps will be inserted here -->
                            </div>
                        </div>

                        <!-- Response Preview -->
                        <div>
                            <div style="font-weight: 600; margin-bottom: 12px; color: var(--text-primary);">💬 Response</div>
                            <div id="traceDetailResponse" style="background: var(--bg-secondary); padding: 16px; border-radius: 10px; max-height: 200px; overflow-y: auto; white-space: pre-wrap; font-size: 0.9em;">
                                -
                            </div>
                        </div>
                    </div>
                    <div class="modal-footer">
                        <button class="btn" onclick="closeTraceDetail()">Close</button>
                    </div>
                </div>
            </div>

            <!-- Integration Configure Modal -->
            <div id="configureIntModal" class="modal" style="display: none;">
                <div class="modal-content" style="max-width: 500px;">
                    <div class="modal-header">
                        <h3 id="configIntTitle">Configure Integration</h3>
                        <button class="btn btn-small" onclick="closeConfigureModal()" style="background: transparent; font-size: 1.2em;">&#10005;</button>
                    </div>
                    <div class="modal-body">
                        <input type="hidden" id="configIntId">
                        <div class="form-group">
                            <label>Client ID</label>
                            <input type="text" id="configClientId" class="form-input" placeholder="OAuth Client ID">
                        </div>
                        <div class="form-group">
                            <label>Client Secret</label>
                            <input type="password" id="configClientSecret" class="form-input" placeholder="OAuth Client Secret">
                        </div>
                        <button class="btn btn-primary" onclick="saveIntegrationConfig()" style="width: 100%; margin-top: 12px;">Save & Connect</button>
                    </div>
                </div>
            </div>

            <!-- Status View -->
            <div class="view hidden" id="statusView">
                <div class="view-header">
                    <h2>📊 Agent Status</h2>
                    <p class="view-subtitle">System health and statistics</p>
                </div>
                <div class="status-panel">
                    <div class="stat-card">
                        <div class="stat-value" id="memoryCount">0</div>
                        <div class="stat-label">Memory Entries</div>
                    </div>
                    <div class="stat-card">
                        <div class="stat-value" id="actionsCount">0</div>
                        <div class="stat-label">Actions Loaded</div>
                    </div>
                    <div class="stat-card">
                        <div class="stat-value" id="tasksCount">0</div>
                        <div class="stat-label">Pending Tasks</div>
                    </div>
                </div>
                <div class="card">
                    <div class="card-title">🔐 Agent Identity (DID)</div>
                    <div class="did-display" id="agentDid">Loading...</div>
                </div>
                <div class="card">
                    <div class="card-title">📦 Version</div>
                    <div class="card-description" id="agentVersion">Loading...</div>
                </div>
            </div>

            <!-- Settings View -->
            <div class="view hidden" id="settingsView">
                <div class="view-header">
                    <h2>⚙️ Settings</h2>
                    <p class="view-subtitle">Configure your AI assistant</p>
                </div>
                <div id="settingsLockNotice" class="settings-lock-banner" style="display: none;">
                    <strong>Setup required:</strong> Please set a <strong>Bot Name</strong>, select an <strong>LLM Provider</strong>, enter your <strong>API key</strong> (or base URL for Ollama), and choose a <strong>Model</strong>, then click <strong>Save Settings</strong>. All other tabs are locked until setup is complete.
                </div>
                <form id="settingsForm">
                    <div class="card">
                        <div class="card-title">🤖 Bot Identity</div>
                        <div class="form-group">
                            <label>Bot Name</label>
                            <input type="text" id="botName" class="form-input" placeholder="CogniArk">
                            <span class="form-hint">What should I call myself?</span>
                        </div>
                        <div class="form-group">
                            <label>Personality</label>
                            <select id="botPersonality" class="form-select">
                                <option value="friendly">🤗 Friendly - Warm and approachable</option>
                                <option value="professional">💼 Professional - Formal and precise</option>
                                <option value="casual">😎 Casual - Relaxed and informal</option>
                                <option value="technical">🔧 Technical - Detailed and thorough</option>
                                <option value="creative">🎨 Creative - Imaginative and expressive</option>
                                <option value="concise">⚡ Concise - Brief and to the point</option>
                            </select>
                            <span class="form-hint">How should I communicate?</span>
                        </div>
                    </div>

                    <div class="card">
                        <div class="card-title">🕒 Daily Brief</div>
                        <div class="form-group">
                            <label>Timezone</label>
                            <input type="text" id="dailyTimezone" class="form-input" list="timezoneList" placeholder="e.g., America/New_York">
                            <datalist id="timezoneList">
                                <option value="UTC"></option>
                                <option value="America/New_York"></option>
                                <option value="America/Chicago"></option>
                                <option value="America/Denver"></option>
                                <option value="America/Los_Angeles"></option>
                                <option value="America/Phoenix"></option>
                                <option value="America/Toronto"></option>
                                <option value="America/Vancouver"></option>
                                <option value="America/Mexico_City"></option>
                                <option value="America/Sao_Paulo"></option>
                                <option value="America/Argentina/Buenos_Aires"></option>
                                <option value="Europe/London"></option>
                                <option value="Europe/Paris"></option>
                                <option value="Europe/Berlin"></option>
                                <option value="Europe/Amsterdam"></option>
                                <option value="Europe/Madrid"></option>
                                <option value="Europe/Rome"></option>
                                <option value="Europe/Warsaw"></option>
                                <option value="Europe/Istanbul"></option>
                                <option value="Africa/Cairo"></option>
                                <option value="Africa/Johannesburg"></option>
                                <option value="Asia/Dubai"></option>
                                <option value="Asia/Jerusalem"></option>
                                <option value="Asia/Kolkata"></option>
                                <option value="Asia/Bangkok"></option>
                                <option value="Asia/Singapore"></option>
                                <option value="Asia/Hong_Kong"></option>
                                <option value="Asia/Shanghai"></option>
                                <option value="Asia/Taipei"></option>
                                <option value="Asia/Tokyo"></option>
                                <option value="Asia/Seoul"></option>
                                <option value="Australia/Sydney"></option>
                                <option value="Australia/Melbourne"></option>
                                <option value="Pacific/Auckland"></option>
                            </datalist>
                            <span class="form-hint">Used to schedule the Daily Brief in your local time.</span>
                        </div>
                        <div class="form-group">
                            <label>Language</label>
                            <input type="text" id="dailyLanguage" class="form-input" placeholder="e.g., English, Spanish">
                            <span class="form-hint">Optional. Defaults to your current locale.</span>
                        </div>
                        <div class="form-group">
                            <label>Tone</label>
                            <select id="dailyTone" class="form-select">
                                <option value="">Default</option>
                                <option value="concise">Concise</option>
                                <option value="friendly">Friendly</option>
                                <option value="professional">Professional</option>
                                <option value="casual">Casual</option>
                                <option value="technical">Technical</option>
                                <option value="creative">Creative</option>
                            </select>
                            <span class="form-hint">Optional. Overrides the brief tone only.</span>
                        </div>
                        <div class="form-group">
                            <label>Email Format</label>
                            <select id="dailyEmailFormat" class="form-select">
                                <option value="">Default</option>
                                <option value="bullets">Bulleted summary</option>
                                <option value="sections">Sectioned summary</option>
                                <option value="narrative">Narrative paragraph</option>
                            </select>
                            <span class="form-hint">Optional. Used for email delivery.</span>
                        </div>
                        <div class="form-group">
                            <label>Push Channel</label>
                            <select id="dailyBriefChannel" class="form-select">
                                <option value="telegram">Telegram (recommended)</option>
                                <option value="email">Email via Gmail</option>
                            </select>
                            <span class="form-hint">Requires Gmail OAuth for email delivery.</span>
                        </div>
                    </div>

                    <div class="card">
                        <div class="card-title">🧠 LLM Provider</div>
                        <div class="form-group">
                            <label>Provider</label>
                            <select id="llmProvider" class="form-select">
                                <option value="ollama">Ollama (Local)</option>
                                <option value="anthropic">Anthropic Claude</option>
                                <option value="openai">OpenAI</option>
                                <option value="openrouter">OpenRouter</option>
                                <option value="openai-compatible">OpenAI-Compatible</option>
                            </select>
                        </div>
                        <div class="form-group" id="baseUrlGroup">
                            <label>Base URL</label>
                            <input type="text" id="llmBaseUrl" class="form-input" placeholder="https://openrouter.ai/api/v1">
                        </div>
                        <div class="form-group" id="apiKeyGroup" style="display:none;">
                            <label>API Key</label>
                            <input type="password" id="llmApiKey" class="form-input" placeholder="Enter API key...">
                            <span id="apiKeyStatus" class="form-hint"></span>
                        </div>
                        <div class="form-group">
                            <label>Model</label>
                            <input type="text" id="llmModel" class="form-input" placeholder="e.g., glm-4, gpt-4o, llama3.2">
                            <span class="form-hint">Common: glm-4, qwen/qwen-2.5-72b-instruct, claude-sonnet-4-20250514</span>
                        </div>
                    </div>

                    <div class="card">
                        <div class="card-title">🔄 Fallback LLM (Optional)</div>
                        <span class="form-hint" style="display: block; margin-bottom: 16px;">Used automatically if primary LLM fails</span>
                        <div class="form-group">
                            <label>Fallback Provider</label>
                            <select id="llmFallbackProvider" class="form-select">
                                <option value="">None (No fallback)</option>
                                <option value="ollama">Ollama (Local)</option>
                                <option value="anthropic">Anthropic Claude</option>
                                <option value="openai">OpenAI</option>
                                <option value="openrouter">OpenRouter</option>
                                <option value="openai-compatible">OpenAI-Compatible</option>
                            </select>
                        </div>
                        <div class="form-group" id="fallbackBaseUrlGroup" style="display:none;">
                            <label>Base URL</label>
                            <input type="text" id="llmFallbackBaseUrl" class="form-input" placeholder="https://openrouter.ai/api/v1">
                        </div>
                        <div class="form-group" id="fallbackApiKeyGroup" style="display:none;">
                            <label>API Key</label>
                            <input type="password" id="llmFallbackApiKey" class="form-input" placeholder="Enter API key...">
                            <span id="fallbackApiKeyStatus" class="form-hint"></span>
                        </div>
                        <div class="form-group" id="fallbackModelGroup" style="display:none;">
                            <label>Model</label>
                            <input type="text" id="llmFallbackModel" class="form-input" placeholder="e.g., gpt-4o, claude-sonnet-4-20250514">
                        </div>
                    </div>

                    <div class="card">
                        <div class="card-title">📱 Telegram Bot (Optional)</div>
                        <div class="form-group">
                            <label class="form-checkbox">
                                <input type="checkbox" id="telegramEnabled">
                                <span>Enable Telegram Bot</span>
                            </label>
                        </div>
                        <div id="telegramSettings" style="display:none;">
                            <div class="form-group">
                                <label>Bot Token</label>
                                <input type="password" id="telegramToken" class="form-input" placeholder="From @BotFather">
                                <span class="form-hint" id="telegramTokenStatus"></span>
                            </div>
                            <div class="form-group">
                                <label>Allowed User IDs</label>
                                <input type="text" id="telegramUsers" class="form-input" placeholder="123456789, 987654321">
                                <span class="form-hint">Comma-separated Telegram user IDs</span>
                            </div>
                        </div>
                    </div>

                    <div class="card">
                        <div class="card-title">✉️ Gmail Integration</div>
                        <div class="form-group">
                            <div id="gmailStatus" style="font-size: 0.9em; color: var(--text-muted);">Checking Gmail status...</div>
                        </div>

                        <details style="margin-bottom: 16px; border: 1px solid var(--border); border-radius: 8px; padding: 0;">
                            <summary style="padding: 12px 16px; cursor: pointer; font-weight: 600; font-size: 0.9em; color: var(--accent); user-select: none; list-style: none; display: flex; align-items: center; gap: 8px;">
                                <span class="setup-arrow" style="transition: transform 0.2s; display: inline-block;">&#9654;</span> How to get Gmail credentials (first-time setup)
                            </summary>
                            <div style="padding: 4px 16px 16px; font-size: 0.85em; line-height: 1.7; color: var(--text-secondary);">
                                <div style="margin-bottom: 8px; font-weight: 600; color: var(--text-primary);">Step 1: Create a Google Cloud Project</div>
                                <ol style="margin: 0 0 12px 18px; padding: 0;">
                                    <li>Go to <a href="https://console.cloud.google.com/" target="_blank" style="color: var(--accent);">console.cloud.google.com</a></li>
                                    <li>Click the project dropdown (top bar) &rarr; <b>New Project</b></li>
                                    <li>Name it anything (e.g. "CogniArk") &rarr; <b>Create</b></li>
                                </ol>

                                <div style="margin-bottom: 8px; font-weight: 600; color: var(--text-primary);">Step 2: Enable the Gmail API</div>
                                <ol style="margin: 0 0 12px 18px; padding: 0;">
                                    <li>In your project, go to <a href="https://console.cloud.google.com/apis/library/gmail.googleapis.com" target="_blank" style="color: var(--accent);">Gmail API</a></li>
                                    <li>Click <b>Enable</b></li>
                                </ol>

                                <div style="margin-bottom: 8px; font-weight: 600; color: var(--text-primary);">Step 3: Configure OAuth Consent Screen</div>
                                <ol style="margin: 0 0 12px 18px; padding: 0;">
                                    <li>Go to <a href="https://console.cloud.google.com/auth/audience" target="_blank" style="color: var(--accent);">Audience</a></li>
                                    <li>Select <b>External</b> user type &rarr; <b>Create</b></li>
                                    <li>Fill in App name, support email</li>
                                    <li>Go to <b>Audience</b> &rarr; set Publishing status to <b>In Production</b></li>
                                    <li>On the consent screen, click <b>Advanced</b> &rarr; <b>Go to app (unsafe)</b> when prompted</li>
                                </ol>

                                <div style="margin-bottom: 8px; font-weight: 600; color: var(--text-primary);">Step 4: Create OAuth Credentials</div>
                                <ol style="margin: 0 0 12px 18px; padding: 0;">
                                    <li>Go to <a href="https://console.cloud.google.com/apis/credentials" target="_blank" style="color: var(--accent);">Credentials</a></li>
                                    <li>Click <b>+ Create Credentials</b> &rarr; <b>OAuth client ID</b></li>
                                    <li>Application type: <b>Desktop app</b></li>
                                    <li>Name it anything &rarr; <b>Create</b></li>
                                    <li>Copy the <b>Client ID</b> and <b>Client Secret</b></li>
                                </ol>

                                <div style="margin-bottom: 8px; font-weight: 600; color: var(--text-primary);">Step 5: Connect</div>
                                <ol style="margin: 0 0 0 18px; padding: 0;">
                                    <li>Paste Client ID and Client Secret below</li>
                                    <li>Click <b>Connect Gmail</b></li>
                                    <li>Authorize in the Google popup</li>
                                </ol>
                            </div>
                        </details>

                        <div class="form-group">
                            <label>Gmail Client ID</label>
                            <input type="text" id="gmailClientId" class="form-input" placeholder="Google OAuth Client ID">
                        </div>
                        <div class="form-group">
                            <label>Gmail Client Secret</label>
                            <input type="password" id="gmailClientSecret" class="form-input" placeholder="Google OAuth Client Secret">
                        </div>
                        <div class="form-group" style="display: flex; gap: 12px; flex-wrap: wrap;">
                            <button type="button" class="btn btn-primary" id="gmailConnectBtn" onclick="connectGmail()">Connect Gmail</button>
                        </div>
                    </div>

                    <div class="card">
                        <div class="card-title">🎨 AI Media Generation (Optional)</div>
                        <p class="form-hint" style="margin-bottom: 16px;">Configure API keys for image and video generation. Leave blank if not needed.</p>

                        <div class="form-group">
                            <label>Replicate API Key</label>
                            <input type="password" id="replicateKey" class="form-input" placeholder="r8_...">
                            <span class="form-hint">Flux, SDXL, Stable Video - <a href="https://replicate.com/account/api-tokens" target="_blank" style="color: var(--accent);">Get key</a></span>
                        </div>

                        <div class="form-group">
                            <label>FAL.ai API Key</label>
                            <input type="password" id="falKey" class="form-input" placeholder="fal_...">
                            <span class="form-hint">Fast Flux, video models - <a href="https://fal.ai/dashboard/keys" target="_blank" style="color: var(--accent);">Get key</a></span>
                        </div>

                        <div class="form-group">
                            <label>Stability AI API Key</label>
                            <input type="password" id="stabilityKey" class="form-input" placeholder="sk-...">
                            <span class="form-hint">SDXL, Stable Video - <a href="https://platform.stability.ai/account/keys" target="_blank" style="color: var(--accent);">Get key</a></span>
                        </div>

                        <div class="form-group">
                            <label>Together.ai API Key</label>
                            <input type="password" id="togetherKey" class="form-input" placeholder="">
                            <span class="form-hint">Open source models - <a href="https://api.together.xyz/settings/api-keys" target="_blank" style="color: var(--accent);">Get key</a></span>
                        </div>

                        <div class="form-group">
                            <label>OpenAI API Key (DALL-E + Sora)</label>
                            <input type="password" id="dalleKey" class="form-input" placeholder="sk-...">
                            <span class="form-hint">DALL-E 3 images + Sora video - <a href="https://platform.openai.com/api-keys" target="_blank" style="color: var(--accent);">Get key</a></span>
                        </div>

                        <div class="form-group">
                            <label>Google AI API Key (Nano Banana / Gemini + Veo)</label>
                            <input type="password" id="googleAiKey" class="form-input" placeholder="AIza...">
                            <span class="form-hint">Gemini 2.0 Flash images + Veo video - <a href="https://aistudio.google.com/apikey" target="_blank" style="color: var(--accent);">Get key</a></span>
                        </div>

                        <div class="form-group">
                            <label>Runway ML API Key</label>
                            <input type="password" id="runwayKey" class="form-input" placeholder="">
                            <span class="form-hint">Gen-3 video generation - <a href="https://app.runwayml.com/settings/api-keys" target="_blank" style="color: var(--accent);">Get key</a></span>
                        </div>

                        <div class="form-group">
                            <label>Luma AI API Key</label>
                            <input type="password" id="lumaKey" class="form-input" placeholder="">
                            <span class="form-hint">Dream Machine video - <a href="https://lumalabs.ai/dream-machine/api" target="_blank" style="color: var(--accent);">Get key</a></span>
                        </div>

                        <div class="form-group">
                            <label>Default Image Provider</label>
                            <select id="defaultImageProvider" class="form-select">
                                <option value="">Auto (first configured)</option>
                                <option value="replicate">Replicate (Flux)</option>
                                <option value="fal">FAL.ai</option>
                                <option value="stability_ai">Stability AI</option>
                                <option value="together">Together.ai</option>
                                <option value="openai_dalle">OpenAI DALL-E</option>
                                <option value="google_gemini">Google Gemini (Nano Banana)</option>
                            </select>
                        </div>

                        <div class="form-group">
                            <label>Fallback Image Provider</label>
                            <select id="fallbackImageProvider" class="form-select">
                                <option value="">None</option>
                                <option value="replicate">Replicate (Flux)</option>
                                <option value="fal">FAL.ai</option>
                                <option value="stability_ai">Stability AI</option>
                                <option value="together">Together.ai</option>
                                <option value="openai_dalle">OpenAI DALL-E</option>
                                <option value="google_gemini">Google Gemini (Nano Banana)</option>
                            </select>
                            <span class="form-hint">Used if default provider fails</span>
                        </div>

                        <div class="form-group">
                            <label>Default Video Provider</label>
                            <select id="defaultVideoProvider" class="form-select">
                                <option value="">Auto (first configured)</option>
                                <option value="replicate">Replicate</option>
                                <option value="fal">FAL.ai</option>
                                <option value="stability_ai">Stability AI</option>
                                <option value="openai_sora">OpenAI Sora</option>
                                <option value="google_veo">Google Veo</option>
                                <option value="runway">Runway ML</option>
                                <option value="luma">Luma AI</option>
                            </select>
                        </div>

                        <div class="form-group">
                            <label>Fallback Video Provider</label>
                            <select id="fallbackVideoProvider" class="form-select">
                                <option value="">None</option>
                                <option value="replicate">Replicate</option>
                                <option value="fal">FAL.ai</option>
                                <option value="stability_ai">Stability AI</option>
                                <option value="openai_sora">OpenAI Sora</option>
                                <option value="google_veo">Google Veo</option>
                                <option value="runway">Runway ML</option>
                                <option value="luma">Luma AI</option>
                            </select>
                            <span class="form-hint">Used if default provider fails</span>
                        </div>
                    </div>

                    <div class="card">
                        <div class="card-title">🔗 Integrations</div>
                        <p class="form-hint" style="margin-bottom: 16px;">Connect external services to extend capabilities</p>
                        <div id="integrationsList">
                            <div class="loading">Loading integrations...</div>
                        </div>
                    </div>

                    <div style="margin-top: 24px; display: flex; gap: 12px; flex-wrap: wrap;">
                        <button type="submit" class="btn btn-primary" id="saveSettingsBtn" disabled>Save Settings</button>
                        <button type="button" class="btn btn-secondary" onclick="loadSettings()">Reset</button>
                        <button type="button" class="btn" style="background: var(--warning); margin-left: auto;" onclick="restartServer()">🔄 Restart Bot</button>
                    </div>
                    <div id="settingsStatus" style="margin-top: 16px;"></div>
                </form>
            </div>
        </main>
    </div>

    <script>
        // State
        let isProcessing = false;
        let currentView = 'chat';
        let messageCount = 0;
        let settingsBaseline = null;
        let settingsLocked = false;

        // Helper to extract status text from TaskStatus enum (handles both string and object variants)
        function getStatusText(status) {
            if (typeof status === 'string') return status;
            if (typeof status === 'object' && status !== null) {
                const keys = Object.keys(status);
                if (keys.length > 0) return keys[0];
            }
            return 'Unknown';
        }

        // Check if status matches a given variant name
        function isStatus(status, variant) {
            if (typeof status === 'string') return status === variant || status.startsWith(variant + ' ') || status.startsWith(variant + '{');
            if (typeof status === 'object' && status !== null) {
                return Object.keys(status).includes(variant);
            }
            return false;
        }

        // Elements
        const chatInput = document.getElementById('chatInput');
        const sendBtn = document.getElementById('sendBtn');
        const messages = document.getElementById('messages');
        const connectionStatus = document.getElementById('connectionStatus');
        const connectionText = document.getElementById('connectionText');

        // Valid views list
        const validViews = ['home', 'chat', 'tasks', 'actions', 'memory', 'goals', 'telegram', 'trace', 'status', 'settings'];

        function normalizeView(view) {
            if (!view) return 'chat';
            return validViews.includes(view) ? view : 'chat';
        }

        function setViewHash(view) {
            const hash = `#${view}`;
            if (location.hash !== hash) {
                history.replaceState(null, '', hash);
            }
        }

        function switchView(view, updateHash = true) {
            currentView = normalizeView(view);

            if (settingsLocked && currentView !== 'settings') {
                console.log('[CogniArk] Tab "' + view + '" blocked by settings lock - redirecting to settings');
                currentView = 'settings';
            }

            if (updateHash) setViewHash(currentView);
            localStorage.setItem('nyrbot:lastView', currentView);

            // Update nav
            document.querySelectorAll('.nav-item').forEach(item => {
                item.classList.toggle('active', item.dataset.view === currentView);
            });

              // Update views
              document.getElementById('homeView').classList.toggle('hidden', currentView !== 'home');
              document.getElementById('chatView').classList.toggle('hidden', currentView !== 'chat');
              document.getElementById('actionsView').classList.toggle('hidden', currentView !== 'actions');
              document.getElementById('tasksView').classList.toggle('hidden', currentView !== 'tasks');
              document.getElementById('memoryView').classList.toggle('hidden', currentView !== 'memory');
              document.getElementById('goalsView').classList.toggle('hidden', currentView !== 'goals');
              document.getElementById('telegramView').classList.toggle('hidden', currentView !== 'telegram');
              document.getElementById('traceView').classList.toggle('hidden', currentView !== 'trace');
              document.getElementById('statusView').classList.toggle('hidden', currentView !== 'status');
              document.getElementById('settingsView').classList.toggle('hidden', currentView !== 'settings');

              // Load data for view
              if (currentView === 'home') loadHome();
              if (currentView === 'actions') loadActions();
              if (currentView === 'tasks') loadTasks();
              if (currentView === 'memory') loadMemory();
              if (currentView === 'goals') loadGoals();
              if (currentView === 'telegram') loadTelegramMessages();
              if (currentView === 'trace') loadTrace();
              if (currentView === 'status') loadStatus();
              if (currentView === 'settings') loadSettings();

              // Auto-refresh active view every 5s
              if (window._viewRefreshInterval) clearInterval(window._viewRefreshInterval);
              const refreshable = { actions: loadActions, tasks: loadTasks, goals: loadGoals, trace: loadTrace, status: loadStatus };
              if (refreshable[currentView]) {
                  window._viewRefreshInterval = setInterval(refreshable[currentView], 5000);
              }
          }

        function getInitialView() {
            const hashView = location.hash.replace('#', '');
            if (hashView) return normalizeView(hashView);
            const saved = localStorage.getItem('nyrbot:lastView');
            return normalizeView(saved || 'home');
        }

        window.addEventListener('hashchange', () => {
            const hashView = location.hash.replace('#', '');
            switchView(normalizeView(hashView), false);
        });

        // Start new conversation
        async function startNewConversation() {
            try {
                await fetch('/chat/clear', {
                    method: 'POST',
                    headers: { 'Content-Type': 'application/json' },
                    body: JSON.stringify({ channel: 'web' })
                });
            } catch(e) { console.warn('Failed to clear server history:', e); }
            messages.innerHTML = `
                <div class="message assistant">
                    <strong>New conversation started!</strong>
                    <br><br>
                    How can I help you today?
                </div>
            `;
            messageCount = 0;
            document.getElementById('chatSubtitle').textContent = 'Fresh start - ask me anything';
            chatInput.focus();
        }

        // Chat functionality
        chatInput.addEventListener('keypress', (e) => {
            if (e.key === 'Enter' && !isProcessing) {
                sendMessage();
            }
        });

        sendBtn.addEventListener('click', () => {
            if (!isProcessing) sendMessage();
        });

        async function sendMessage() {
            const text = chatInput.value.trim();
            if (!text) return;

            chatInput.value = '';
            addMessage(text, 'user');
            messageCount++;

            isProcessing = true;
            sendBtn.disabled = true;

            // Show typing indicator
            const typingEl = document.createElement('div');
            typingEl.className = 'message assistant typing';
            typingEl.id = 'typing';
            typingEl.innerHTML = '<span></span><span></span><span></span>';
            messages.appendChild(typingEl);
            messages.scrollTop = messages.scrollHeight;

            try {
                const response = await fetch('/chat', {
                    method: 'POST',
                    headers: { 'Content-Type': 'application/json' },
                    body: JSON.stringify({ message: text, channel: 'web' })
                });

                const data = await response.json();

                // Remove typing indicator
                document.getElementById('typing')?.remove();

                if (response.ok) {
                    addMessage(data.response, 'assistant');
                    messageCount++;
                    document.getElementById('chatSubtitle').textContent = `${messageCount} messages in this conversation`;
                } else {
                    addMessage('Error: ' + (data.error || 'Unknown error'), 'system');
                }
            } catch (error) {
                document.getElementById('typing')?.remove();
                addMessage('Connection error: ' + error.message, 'system');
                updateConnectionStatus(false);
            }

            isProcessing = false;
            sendBtn.disabled = false;
        }

        function addMessage(text, role) {
            // Create wrapper for message + avatar
            const wrapper = document.createElement('div');
            wrapper.className = 'message-wrapper ' + role;

            // Create avatar
            const avatar = document.createElement('div');
            avatar.className = 'message-avatar ' + role;

            if (role === 'user') {
                avatar.innerHTML = '<svg viewBox="0 0 24 24" fill="currentColor" width="20" height="20"><path d="M12 12c2.21 0 4-1.79 4-4s-1.79-4-4-4-4 1.79-4 4 1.79 4 4 4zm0 2c-2.67 0-8 1.34-8 4v2h16v-2c0-2.66-5.33-4-8-4z"/></svg>';
            } else if (role === 'assistant') {
                avatar.innerHTML = '<svg viewBox="0 0 24 24" fill="none" width="20" height="20"><rect x="3" y="4" width="18" height="14" rx="3" stroke="currentColor" stroke-width="1.5" fill="var(--accent)"/><circle cx="9" cy="11" r="1.5" fill="var(--bg-primary)"/><circle cx="15" cy="11" r="1.5" fill="var(--bg-primary)"/><path d="M9 14.5c0 0 1.5 1.5 3 1.5s3-1.5 3-1.5" stroke="var(--bg-primary)" stroke-width="1.2" stroke-linecap="round"/></svg>';
            } else {
                avatar.innerHTML = '⚠️';
            }

            // Create message bubble
            const msg = document.createElement('div');
            msg.className = 'message ' + role;

            // Enhanced markdown rendering
            let formattedText = renderMarkdown(text);
            msg.innerHTML = formattedText;

            const time = document.createElement('div');
            time.className = 'message-time';
            time.textContent = new Date().toLocaleTimeString();
            msg.appendChild(time);

            // Assemble: avatar + message
            if (role === 'user') {
                wrapper.appendChild(msg);
                wrapper.appendChild(avatar);
            } else {
                wrapper.appendChild(avatar);
                wrapper.appendChild(msg);
            }

            messages.appendChild(wrapper);
            messages.scrollTop = messages.scrollHeight;
        }

        function seedQuickTask(prompt) {
            switchView('tasks');
            document.getElementById('taskDescription').value = prompt;
            document.getElementById('taskDescription').focus();
        }

        async function loadHome() {
            try {
                const [statusResp, tasksResp, traceResp] = await Promise.all([
                    fetch('/status'),
                    fetch('/tasks'),
                    fetch('/trace')
                ]);

                if (!statusResp.ok || !tasksResp.ok || !traceResp.ok) throw new Error('Server error');
                const status = await statusResp.json();
                const tasksData = await tasksResp.json();
                const traceData = await traceResp.json();

                const tasks = tasksData.tasks || [];
                const pending = tasks.filter(t => isStatus(t.status, 'Pending') || isStatus(t.status, 'AwaitingApproval'));
                const routines = tasks.filter(t => t.cron);

                const summaryEl = document.getElementById('homeSummary');
                summaryEl.innerHTML = `
                    <div style="display: grid; grid-template-columns: repeat(auto-fit, minmax(160px, 1fr)); gap: 12px;">
                        <div class="feature-item">
                            <div style="font-size: 1.6em; font-weight: 700; color: var(--accent);">${status.tasks_pending || 0}</div>
                            <div style="font-size: 0.85em; color: var(--text-muted);">Pending Tasks</div>
                        </div>
                        <div class="feature-item">
                            <div style="font-size: 1.6em; font-weight: 700; color: var(--success);">${routines.length}</div>
                            <div style="font-size: 0.85em; color: var(--text-muted);">Active Routines</div>
                        </div>
                        <div class="feature-item">
                            <div style="font-size: 1.6em; font-weight: 700; color: var(--warning);">${status.memory_entries || 0}</div>
                            <div style="font-size: 0.85em; color: var(--text-muted);">Memory Entries</div>
                        </div>
                        <div class="feature-item">
                            <div style="font-size: 1.6em; font-weight: 700; color: var(--accent);">${status.actions_loaded || 0}</div>
                            <div style="font-size: 0.85em; color: var(--text-muted);">Actions Loaded</div>
                        </div>
                    </div>
                `;

                const inboxEl = document.getElementById('homeInbox');
                if (pending.length === 0) {
                    inboxEl.innerHTML = '<div class="trace-empty">No pending tasks. You are all caught up.</div>';
                } else {
                    inboxEl.innerHTML = pending.slice(0, 3).map(t => `
                        <div class="trace-history-item" style="padding: 12px; border-bottom: 1px solid var(--border);">
                            <div style="font-weight: 600;">${escapeHtml(t.description)}</div>
                            <div style="font-size: 0.85em; color: var(--text-muted); margin-top: 4px;">
                                ${t.cron ? '⏰ ' + escapeHtml(t.cron) : '⏳ Pending'}
                            </div>
                        </div>
                    `).join('');
                }

                const receiptsEl = document.getElementById('homeReceipts');
                const proofs = (traceData.proofs || []).slice(0, 3);
                if (proofs.length === 0) {
                    receiptsEl.innerHTML = '<div class="trace-empty">No receipts yet.</div>';
                } else {
                    receiptsEl.innerHTML = proofs.map(p => `
                        <div class="trace-history-item" style="padding: 12px; border-bottom: 1px solid var(--border);">
                            <div style="font-weight: 600;">${escapeHtml(p.message_preview)}</div>
                            <div style="font-size: 0.8em; color: var(--text-muted); margin-top: 4px;">${escapeHtml(p.time || '')}</div>
                        </div>
                    `).join('');
                }
            } catch (error) {
                console.error('Failed to load home:', error);
                document.getElementById('homeSummary').innerHTML = '<div class="trace-empty">Failed to load daily brief.</div>';
            }
        }

        // Markdown renderer
        function renderMarkdown(text) {
            // Escape HTML first
            let html = text
                .replace(/&/g, '&amp;')
                .replace(/</g, '&lt;')
                .replace(/>/g, '&gt;');

            // Code blocks (```)
            html = html.replace(/```(\w*)\n([\s\S]*?)```/g, (match, lang, code) => {
                return `<pre class="code-block"><code>${code.trim()}</code></pre>`;
            });

            // Tables
            html = html.replace(/(\|.+\|[\r\n]+\|[-:| ]+\|[\r\n]+(?:\|.+\|[\r\n]*)+)/g, (match) => {
                const rows = match.trim().split('\n').filter(r => r.trim());
                if (rows.length < 2) return match;

                let tableHtml = '<table class="md-table"><thead><tr>';
                // Header row
                const headers = rows[0].split('|').filter(c => c.trim());
                headers.forEach(h => {
                    tableHtml += `<th>${h.trim()}</th>`;
                });
                tableHtml += '</tr></thead><tbody>';

                // Data rows (skip separator row)
                for (let i = 2; i < rows.length; i++) {
                    const cells = rows[i].split('|').filter(c => c.trim());
                    tableHtml += '<tr>';
                    cells.forEach(c => {
                        tableHtml += `<td>${c.trim()}</td>`;
                    });
                    tableHtml += '</tr>';
                }
                tableHtml += '</tbody></table>';
                return tableHtml;
            });

            // Headers
            html = html.replace(/^### (.+)$/gm, '<h4 class="md-h4">$1</h4>');
            html = html.replace(/^## (.+)$/gm, '<h3 class="md-h3">$1</h3>');
            html = html.replace(/^# (.+)$/gm, '<h2 class="md-h2">$1</h2>');

            // Horizontal rules
            html = html.replace(/^---+$/gm, '<hr class="md-hr">');

            // Bold and italic
            html = html.replace(/\*\*\*(.+?)\*\*\*/g, '<strong><em>$1</em></strong>');
            html = html.replace(/\*\*(.+?)\*\*/g, '<strong>$1</strong>');
            html = html.replace(/\*(.+?)\*/g, '<em>$1</em>');

            // Inline code
            html = html.replace(/`([^`]+)`/g, '<code class="md-inline-code">$1</code>');

            // Lists (unordered)
            html = html.replace(/^[\-\*] (.+)$/gm, '<li class="md-li">$1</li>');
            html = html.replace(/(<li class="md-li">.*<\/li>\n?)+/g, '<ul class="md-ul">$&</ul>');

            // Numbered lists
            html = html.replace(/^\d+\. (.+)$/gm, '<li class="md-li-num">$1</li>');
            html = html.replace(/(<li class="md-li-num">.*<\/li>\n?)+/g, '<ol class="md-ol">$&</ol>');

            // Paragraphs (double newlines)
            html = html.replace(/\n\n+/g, '</p><p class="md-p">');
            html = '<p class="md-p">' + html + '</p>';

            // Single newlines to <br> (but not inside code/pre)
            html = html.replace(/([^>])\n([^<])/g, '$1<br>$2');

            // Clean up empty paragraphs
            html = html.replace(/<p class="md-p"><\/p>/g, '');
            html = html.replace(/<p class="md-p">(\s*)<\/p>/g, '');

            return html;
        }

        // Actions
        async function loadActions() {
            try {
                const response = await fetch('/actions');
                const data = await response.json();

                const container = document.getElementById('actionsList');
                container.innerHTML = '';

                if (data.actions && data.actions.length > 0) {
                    // Group actions into three categories
                    const userActions = data.actions.filter(s => s.source === 'custom');
                    const bundledActions = data.actions.filter(s => s.source === 'bundled');
                    const systemActions = data.actions.filter(s => s.source === 'system');

                    // --- User Actions (user-created) ---
                    if (userActions.length > 0) {
                        const header = document.createElement('div');
                        header.style.cssText = 'display: flex; align-items: center; gap: 8px; margin: 0 0 12px 0;';
                        header.innerHTML = `
                            <span style="font-size: 1.1em;">👤</span>
                            <h4 style="color: var(--text-secondary); margin: 0; font-size: 0.85em; text-transform: uppercase; letter-spacing: 1px;">Your Actions</h4>
                            <span style="background: var(--success); color: #fff; font-size: 0.7em; padding: 2px 8px; border-radius: 8px;">${userActions.length}</span>
                        `;
                        container.appendChild(header);

                        userActions.forEach(action => {
                            const card = document.createElement('div');
                            card.className = 'card';
                            card.style.borderLeft = '3px solid var(--success)';
                            card.innerHTML = `
                                <div style="display: flex; justify-content: space-between; align-items: flex-start;">
                                    <div>
                                        <div class="card-title">👤 ${escapeHtml(action.name)}</div>
                                        <div class="card-description">${escapeHtml(action.description)}</div>
                                    </div>
                                    <div style="display: flex; gap: 6px;">
                                        <button class="btn btn-small" onclick="editAction('${action.name}')" style="padding: 6px 12px; font-size: 0.8em;">✏️ Edit</button>
                                    </div>
                                </div>
                                <div class="card-meta">
                                    <span class="action-badge" style="background: var(--accent);">v${action.version}</span>
                                    <span class="action-badge" style="background: var(--success);">user</span>
                                </div>
                            `;
                            container.appendChild(card);
                        });
                    }

                    // --- Bundled Actions (shipped with app) ---
                    if (bundledActions.length > 0) {
                        const header = document.createElement('div');
                        header.style.cssText = 'display: flex; align-items: center; gap: 8px; margin: 20px 0 12px 0;';
                        header.innerHTML = `
                            <span style="font-size: 1.1em;">📦</span>
                            <h4 style="color: var(--text-secondary); margin: 0; font-size: 0.85em; text-transform: uppercase; letter-spacing: 1px;">Bundled Actions</h4>
                            <span style="background: var(--accent-secondary); color: #fff; font-size: 0.7em; padding: 2px 8px; border-radius: 8px;">${bundledActions.length}</span>
                        `;
                        container.appendChild(header);

                        bundledActions.forEach(action => {
                            const card = document.createElement('div');
                            card.className = 'card';
                            card.style.borderLeft = '3px solid var(--accent-secondary)';
                            card.innerHTML = `
                                <div style="display: flex; justify-content: space-between; align-items: flex-start;">
                                    <div>
                                        <div class="card-title">📦 ${escapeHtml(action.name)}</div>
                                        <div class="card-description">${escapeHtml(action.description)}</div>
                                    </div>
                                    <button class="btn btn-small" onclick="editAction('${action.name}')" style="padding: 6px 12px; font-size: 0.8em;">✏️ Edit</button>
                                </div>
                                <div class="card-meta">
                                    <span class="action-badge" style="background: var(--accent);">v${action.version}</span>
                                    <span class="action-badge" style="background: var(--accent-secondary);">bundled</span>
                                </div>
                            `;
                            container.appendChild(card);
                        });
                    }

                    // --- System Actions (built-in, read-only) ---
                    if (systemActions.length > 0) {
                        const header = document.createElement('div');
                        header.style.cssText = 'display: flex; align-items: center; gap: 8px; margin: 20px 0 12px 0;';
                        header.innerHTML = `
                            <span style="font-size: 1.1em;">🔧</span>
                            <h4 style="color: var(--text-secondary); margin: 0; font-size: 0.85em; text-transform: uppercase; letter-spacing: 1px;">System Actions</h4>
                            <span style="background: var(--text-muted); color: #fff; font-size: 0.7em; padding: 2px 8px; border-radius: 8px;">${systemActions.length}</span>
                        `;
                        container.appendChild(header);

                        systemActions.forEach(action => {
                            const card = document.createElement('div');
                            card.className = 'card';
                            card.style.cssText = 'opacity: 0.8; border-left: 3px solid var(--text-muted);';
                            card.innerHTML = `
                                <div class="card-title">🔧 ${escapeHtml(action.name)}</div>
                                <div class="card-description">${escapeHtml(action.description)}</div>
                                <div class="card-meta">
                                    <span class="action-badge">v${action.version}</span>
                                    <span class="action-badge" style="background: var(--text-muted);">system</span>
                                </div>
                            `;
                            container.appendChild(card);
                        });
                    }
                } else {
                    container.innerHTML = `
                        <div class="empty-state">
                            <div class="empty-state-icon">🔧</div>
                            <p>No actions available yet</p>
                        </div>
                    `;
                }
            } catch (error) {
                console.error('Failed to load actions:', error);
            }
        }

        // Edit action
        let currentEditingAction = null;
        let isCreatingNewAction = false;

        async function editAction(actionName) {
            try {
                const response = await fetch(`/actions/${encodeURIComponent(actionName)}`);
                if (!response.ok) throw new Error('Failed to load action');
                const data = await response.json();

                isCreatingNewAction = false;
                currentEditingAction = actionName;
                document.getElementById('actionEditorTitle').textContent = `Edit Action: ${actionName}`;
                document.getElementById('actionNameGroup').style.display = 'none';
                document.getElementById('actionEditorContent').value = data.content;
                document.getElementById('deleteActionBtn').style.display = 'block';

                // Hide mode toggle and show advanced mode for editing
                document.getElementById('actionModeToggle').style.display = 'none';
                document.getElementById('simpleMode').style.display = 'none';
                document.getElementById('advancedMode').style.display = 'flex';
                document.getElementById('generateActionBtn').style.display = 'none';
                document.getElementById('saveActionBtn').style.display = 'block';

                document.getElementById('actionEditorModal').style.display = 'flex';
            } catch (error) {
                alert('Failed to load action: ' + error.message);
            }
        }

        function closeActionEditor() {
            document.getElementById('actionEditorModal').style.display = 'none';
            currentEditingAction = null;
            isCreatingNewAction = false;
            document.getElementById('generateStatus').style.display = 'none';
        }

        function closeActionPreview() {
            document.getElementById('actionPreviewModal').style.display = 'none';
        }

        function setActionMode(mode) {
            currentActionMode = mode;
            document.getElementById('simpleModeBtn').classList.toggle('active', mode === 'simple');
            document.getElementById('advancedModeBtn').classList.toggle('active', mode === 'advanced');
            document.getElementById('simpleModeBtn').style.background = mode === 'simple' ? 'var(--accent)' : 'transparent';
            document.getElementById('advancedModeBtn').style.background = mode === 'advanced' ? 'var(--accent)' : 'transparent';
            document.getElementById('simpleMode').style.display = mode === 'simple' ? 'flex' : 'none';
            document.getElementById('advancedMode').style.display = mode === 'advanced' ? 'flex' : 'none';
            document.getElementById('generateActionBtn').style.display = mode === 'simple' ? 'block' : 'none';
            document.getElementById('saveActionBtn').style.display = mode === 'advanced' ? 'block' : 'none';
        }

        function showCreateActionModal() {
            isCreatingNewAction = true;
            currentEditingAction = null;
            document.getElementById('actionEditorTitle').textContent = 'Create New Action';

            // Show mode toggle for new actions
            document.getElementById('actionModeToggle').style.display = 'block';

            // Reset simple mode fields
            document.getElementById('simpleActionName').value = '';
            document.getElementById('simpleActionDescription').value = '';
            document.getElementById('simpleActionInstructions').value = '';
            document.getElementById('simpleActionQueries').value = '';
            document.getElementById('generateStatus').style.display = 'none';

            // Reset advanced mode fields
            document.getElementById('actionNameGroup').style.display = 'block';
            document.getElementById('actionEditorName').value = '';
            document.getElementById('actionEditorContent').value = `---
name: my-new-action
description: Describe what this action does
version: 1.0.0
---

# My New Action

Brief description of the action's purpose.

## Workflow

1. First, the agent will...
2. Then it will...
3. Finally, it will...

## Search Queries
- example search query 1
- example search query 2

## Output Format

Describe the expected output format here.
`;
            document.getElementById('deleteActionBtn').style.display = 'none';

            // Set to simple mode by default
            setActionMode('simple');

            document.getElementById('actionEditorModal').style.display = 'flex';
        }

        async function generateActionWithAI() {
            const name = document.getElementById('simpleActionName').value.trim();
            const description = document.getElementById('simpleActionDescription').value.trim();
            const instructions = document.getElementById('simpleActionInstructions').value.trim();
            const queries = document.getElementById('simpleActionQueries').value.trim();

            if (!name) {
                alert('Please enter an action name');
                return;
            }
            if (!/^[a-z0-9-]+$/.test(name)) {
                alert('Action name must be lowercase letters, numbers, and hyphens only');
                return;
            }
            if (!description) {
                alert('Please describe what the action does');
                return;
            }

            const statusDiv = document.getElementById('generateStatus');
            statusDiv.style.display = 'block';
            statusDiv.style.background = 'var(--bg-secondary)';
            statusDiv.innerHTML = '<span style="color: var(--accent);">🤖 Generating action with AI...</span>';

            generatedActionName = name;

            // Build prompt for AI
            const prompt = `Generate a complete ACTION.md file for an AI action with these details:

Name: ${name}
Description: ${description}
${instructions ? `Instructions: ${instructions}` : ''}
${queries ? `Example search queries:\n${queries}` : ''}

Generate the action in the exact ACTION.md format with frontmatter (name, description, version), workflow steps, search queries section, and output format. Make it detailed and professional.`;

            try {
                const response = await fetch('/chat', {
                    method: 'POST',
                    headers: { 'Content-Type': 'application/json' },
                    body: JSON.stringify({ message: prompt, channel: 'action-generator' })
                });

                if (response.ok) {
                    const data = await response.json();
                    let content = data.response;

                    // Try to extract the action content if wrapped in markdown code block
                    const codeBlockMatch = content.match(/```(?:markdown|md)?\s*([\s\S]*?)```/);
                    if (codeBlockMatch) {
                        content = codeBlockMatch[1].trim();
                    }

                    // If content doesn't start with ---, add default frontmatter
                    if (!content.startsWith('---')) {
                        content = `---
name: ${name}
description: ${description}
version: 1.0.0
---

${content}`;
                    }

                    statusDiv.innerHTML = '<span style="color: var(--success);">✓ Action generated! Opening preview...</span>';

                    // Show preview modal
                    setTimeout(() => {
                        document.getElementById('actionPreviewContent').value = content;
                        document.getElementById('actionPreviewModal').style.display = 'flex';
                    }, 500);
                } else {
                    const error = await response.json();
                    statusDiv.style.background = 'rgba(239, 68, 68, 0.1)';
                    statusDiv.innerHTML = `<span style="color: var(--error);">❌ Failed: ${error.error || 'Unknown error'}</span>`;
                }
            } catch (error) {
                statusDiv.style.background = 'rgba(239, 68, 68, 0.1)';
                statusDiv.innerHTML = `<span style="color: var(--error);">❌ Error: ${error.message}</span>`;
            }
        }

        function editGeneratedAction() {
            const content = document.getElementById('actionPreviewContent').value;
            closeActionPreview();

            // Switch to advanced mode with the generated content
            setActionMode('advanced');
            document.getElementById('actionEditorName').value = generatedActionName;
            document.getElementById('actionEditorContent').value = content;
        }

        async function acceptGeneratedAction() {
            const content = document.getElementById('actionPreviewContent').value;

            try {
                const response = await fetch('/actions', {
                    method: 'POST',
                    headers: { 'Content-Type': 'application/json' },
                    body: JSON.stringify({ name: generatedActionName, content })
                });

                if (response.ok) {
                    closeActionPreview();
                    closeActionEditor();
                    loadActions();
                    alert('Action created successfully!');
                } else {
                    const error = await response.json();
                    alert('Failed to create action: ' + (error.error || 'Unknown error'));
                }
            } catch (error) {
                alert('Failed to create action: ' + error.message);
            }
        }

        async function saveAction() {
            const content = document.getElementById('actionEditorContent').value;

            if (isCreatingNewAction) {
                // Creating new action
                const actionName = document.getElementById('actionEditorName').value.trim();
                if (!actionName) {
                    alert('Please enter an action name');
                    return;
                }
                if (!/^[a-z0-9-]+$/.test(actionName)) {
                    alert('Action name must be lowercase letters, numbers, and hyphens only');
                    return;
                }

                try {
                    const response = await fetch('/actions', {
                        method: 'POST',
                        headers: { 'Content-Type': 'application/json' },
                        body: JSON.stringify({ name: actionName, content })
                    });

                    if (response.ok) {
                        closeActionEditor();
                        loadActions();
                        alert('Action created successfully!');
                    } else {
                        const error = await response.json();
                        alert('Failed to create action: ' + (error.error || 'Unknown error'));
                    }
                } catch (error) {
                    alert('Failed to create action: ' + error.message);
                }
            } else if (currentEditingAction) {
                // Updating existing action
                try {
                    const response = await fetch(`/actions/${encodeURIComponent(currentEditingAction)}`, {
                        method: 'POST',
                        headers: { 'Content-Type': 'application/json' },
                        body: JSON.stringify({ content })
                    });

                    if (response.ok) {
                        closeActionEditor();
                        loadActions();
                        alert('Action saved successfully!');
                    } else {
                        const error = await response.json();
                        alert('Failed to save action: ' + (error.error || 'Unknown error'));
                    }
                } catch (error) {
                    alert('Failed to save action: ' + error.message);
                }
            }
        }

        async function deleteAction() {
            if (!currentEditingAction) return;

            if (!confirm(`Are you sure you want to delete the action "${currentEditingAction}"? This cannot be undone.`)) {
                return;
            }

            try {
                const response = await fetch(`/actions/${encodeURIComponent(currentEditingAction)}`, {
                    method: 'DELETE'
                });

                if (response.ok) {
                    closeActionEditor();
                    loadActions();
                    alert('Action deleted successfully!');
                } else {
                    const error = await response.json();
                    alert('Failed to delete action: ' + (error.error || 'Unknown error'));
                }
            } catch (error) {
                alert('Failed to delete action: ' + error.message);
            }
        }

        // Tasks
        let currentTaskFilter = 'all';
        function filterTasks(filter) {
            currentTaskFilter = filter;
            document.querySelectorAll('.task-filter').forEach(b =>
                b.classList.toggle('active', b.dataset.filter === filter));
            loadTasks();
        }

        async function loadTasks() {
            try {
                const response = await fetch('/tasks');
                const data = await response.json();

                const container = document.getElementById('tasksList');
                container.innerHTML = '';
                taskCache.clear();

                let tasks = data.tasks || [];

                // Apply filter
                switch (currentTaskFilter) {
                    case 'pending':
                        tasks = tasks.filter(t => isStatus(t.status, 'Pending') || isStatus(t.status, 'AwaitingApproval'));
                        break;
                    case 'routines':
                        tasks = tasks.filter(t => t.cron);
                        break;
                    case 'completed':
                        tasks = tasks.filter(t => isStatus(t.status, 'Completed'));
                        break;
                    case 'failed':
                        tasks = tasks.filter(t => isStatus(t.status, 'Failed'));
                        break;
                }

                if (tasks.length > 0) {
                    tasks.forEach(task => {
                        taskCache.set(task.id, task);
                        const statusColor = isStatus(task.status, 'Completed') ? 'var(--success)' :
                                          isStatus(task.status, 'InProgress') ? 'var(--accent)' :
                                          isStatus(task.status, 'Pending') ? 'var(--warning)' : 'var(--error)';
                        const scheduleInfo = task.cron ? `<span class="action-badge">⏰ ${getScheduleLabel(task.cron)}</span>` : '';
                        const hasResult = task.result && task.result.trim();
                        const resultPreview = hasResult ? (task.result.length > 200 ? task.result.substring(0, 200) + '...' : task.result) : '';
                        const planSteps = renderPlanSteps(task.arguments);
                        const isAwaiting = isStatus(task.status, 'AwaitingApproval');
                        const card = document.createElement('div');
                        card.className = 'card';
                        card.style.cursor = hasResult ? 'pointer' : 'default';
                        card.innerHTML = `
                            <div style="display: flex; justify-content: space-between; align-items: flex-start;">
                                <div style="flex: 1;">
                                    <div class="card-title">${escapeHtml(task.description)}</div>
                                    <div class="card-description" style="margin-top: 8px;">
                                        <span class="action-badge">${escapeHtml(task.action)}</span>
                                        ${scheduleInfo}
                                    </div>
                                </div>
                                <div style="text-align: right;">
                                    <div style="display: flex; gap: 6px; justify-content: flex-end; margin-bottom: 6px;">
                                        ${isAwaiting ? `
                                            <button class="btn btn-ghost btn-small" onclick="approveTask('${task.id}')">Approve</button>
                                            <button class="btn btn-ghost btn-small" onclick="rejectTask('${task.id}')">Reject</button>
                                        ` : ''}
                                        <button class="btn btn-ghost btn-small" onclick="openTaskEditModal('${task.id}')">Edit</button>
                                        <button class="btn btn-ghost btn-small" onclick="deleteTask('${task.id}')">Delete</button>
                                    </div>
                                    <div style="color: ${statusColor}; font-weight: 500; font-size: 0.9em;">${escapeHtml(getStatusText(task.status))}</div>
                                    <div style="font-size: 0.75em; color: var(--text-muted); margin-top: 4px;">${escapeHtml(task.created_at)}</div>
                                </div>
                            </div>
                            ${planSteps}
                            ${hasResult ? `
                            <div style="margin-top: 12px; padding-top: 12px; border-top: 1px solid var(--border);">
                                <div style="font-size: 0.8em; color: var(--text-muted); margin-bottom: 6px;">Output:</div>
                                <div style="background: var(--bg-secondary); padding: 12px; border-radius: 8px; font-size: 0.85em; max-height: 150px; overflow-y: auto; white-space: pre-wrap;">${escapeHtml(resultPreview)}</div>
                                ${task.result.length > 200 ? '<button class="btn btn-ghost btn-small" onclick="showTaskResult(this)" style="margin-top: 8px; font-size: 0.8em;" data-result="' + encodeURIComponent(task.result) + '">View Full Result</button>' : ''}
                            </div>
                            ` : isStatus(task.status, 'Pending') ? `
                            <div style="margin-top: 12px; padding-top: 12px; border-top: 1px solid var(--border);">
                                <div style="font-size: 0.85em; color: var(--text-muted); font-style: italic;">Waiting to run...</div>
                            </div>
                            ` : ''}
                        `;
                        container.appendChild(card);
                    });
                } else {
                    const filterMsg = currentTaskFilter === 'all' ? 'No tasks scheduled yet' : 'No ' + currentTaskFilter + ' tasks';
                    container.innerHTML = `
                        <div class="empty-state">
                            <div class="empty-state-icon">📭</div>
                            <p>${filterMsg}</p>
                        </div>
                    `;
                }
            } catch (error) {
                console.error('Failed to load tasks:', error);
            }
        }

        async function approveTask(taskId) {
            try {
                const response = await fetch('/tasks/' + taskId + '/approve', { method: 'POST' });
                if (response.ok) {
                    loadTasks();
                } else {
                    const error = await response.json();
                    alert('Failed to approve: ' + (error.error || 'Unknown error'));
                }
            } catch (error) {
                alert('Failed to approve: ' + error.message);
            }
        }

        async function rejectTask(taskId) {
            try {
                const response = await fetch('/tasks/' + taskId + '/reject', { method: 'POST' });
                if (response.ok) {
                    loadTasks();
                } else {
                    const error = await response.json();
                    alert('Failed to reject: ' + (error.error || 'Unknown error'));
                }
            } catch (error) {
                alert('Failed to reject: ' + error.message);
            }
        }

        async function loadMemory() {
            try {
                const [profileResp, statusResp] = await Promise.all([
                    fetch('/profile'),
                    fetch('/status')
                ]);
                if (!profileResp.ok) throw new Error('Profile: server returned ' + profileResp.status);
                if (!statusResp.ok) throw new Error('Status: server returned ' + statusResp.status);
                const profile = await profileResp.json();
                const status = await statusResp.json();
                const card = document.getElementById('memoryCard');

                card.innerHTML = `
                    <div style="display: grid; gap: 12px;">
                        <div>
                            <div style="font-size: 0.85em; color: var(--text-muted);">Name</div>
                            <div style="font-weight: 600;">${escapeHtml(profile.name || 'Not set')}</div>
                        </div>
                        <div>
                            <div style="font-size: 0.85em; color: var(--text-muted);">Location</div>
                            <div style="font-weight: 600;">${escapeHtml(profile.location || 'Not set')}</div>
                        </div>
                        <div>
                            <div style="font-size: 0.85em; color: var(--text-muted);">Timezone</div>
                            <div style="font-weight: 600;">${escapeHtml(profile.timezone || 'Not set')}</div>
                        </div>
                        <div>
                            <div style="font-size: 0.85em; color: var(--text-muted);">Language</div>
                            <div style="font-weight: 600;">${escapeHtml(profile.language || 'Not set')}</div>
                        </div>
                        <div>
                            <div style="font-size: 0.85em; color: var(--text-muted);">Tone</div>
                            <div style="font-weight: 600;">${escapeHtml(profile.tone || 'Not set')}</div>
                        </div>
                        <div>
                            <div style="font-size: 0.85em; color: var(--text-muted);">Email Format</div>
                            <div style="font-weight: 600;">${escapeHtml(profile.email_format || 'Not set')}</div>
                        </div>
                        <div>
                            <div style="font-size: 0.85em; color: var(--text-muted);">Preferences</div>
                            <div style="font-weight: 600;">${escapeHtml(profile.preferences || 'Not set')}</div>
                        </div>
                        <div>
                            <div style="font-size: 0.85em; color: var(--text-muted);">Memory entries</div>
                            <div style="font-weight: 600;">${status.memory_entries || 0}</div>
                        </div>
                    </div>
                `;
            } catch (error) {
                console.error('Failed to load memory:', error);
                document.getElementById('memoryCard').innerHTML = '<div class="trace-empty" style="color: var(--warning);">Failed to load memory: ' + escapeHtml(error.message) + '</div>';
            }
        }

        async function loadGoals() {
            const container = document.getElementById('goalsList');
            try {
                const resp = await fetch('/goals');
                const data = await resp.json();
                const goals = data.goals || [];
                if (!goals.length) {
                    container.innerHTML = '<div class="trace-empty">No goals yet. Add one above.</div>';
                    return;
                }
                container.innerHTML = goals.map(g => `
                    <div class="trace-history-item" style="padding: 12px; border-bottom: 1px solid var(--border);">
                        <div style="display: flex; justify-content: space-between; align-items: center;">
                            <div style="font-weight: 600;">${escapeHtml(g.description)}</div>
                            <button class="btn btn-ghost btn-small" onclick="deleteGoal('${g.id}')">Delete</button>
                        </div>
                    </div>
                `).join('');
            } catch (e) {
                container.innerHTML = '<div class="trace-empty" style="color: var(--warning);">Failed to load goals</div>';
            }
        }

        async function addGoal() {
            const input = document.getElementById('goalInput');
            const value = input.value.trim();
            if (!value) return;
            await fetch('/goals', {
                method: 'POST',
                headers: {'Content-Type': 'application/json'},
                body: JSON.stringify({ description: value })
            });
            input.value = '';
            loadGoals();
        }

        async function deleteGoal(id) {
            await fetch('/goals/' + id, { method: 'DELETE' });
            loadGoals();
        }

        // Migrate legacy localStorage goals to server (one-time)
        async function migrateLocalGoals() {
            const legacy = JSON.parse(localStorage.getItem('nyrbot:goals') || '[]');
            if (legacy.length) {
                for (const g of legacy) {
                    await fetch('/goals', {
                        method: 'POST',
                        headers: {'Content-Type': 'application/json'},
                        body: JSON.stringify({ description: g })
                    });
                }
                localStorage.removeItem('nyrbot:goals');
            }
        }

        // Telegram Messages
        async function loadTelegramMessages() {
            try {
                // Check Telegram status from settings
                const settingsResp = await fetch('/settings');
                const settings = await settingsResp.json();
                const statusEl = document.getElementById('telegramStatus');

                if (settings.telegram_enabled) {
                    statusEl.innerHTML = `
                        <div style="display: flex; align-items: center; gap: 8px;">
                            <span style="color: var(--success);">●</span>
                            <span>Telegram Bot Enabled</span>
                        </div>
                        <div style="font-size: 0.85em; color: var(--text-muted); margin-top: 4px;">
                            Allowed users: ${settings.telegram_allowed_users?.length ? settings.telegram_allowed_users.join(', ') : 'All users'}
                        </div>
                    `;
                } else {
                    statusEl.innerHTML = `
                        <div style="display: flex; align-items: center; gap: 8px;">
                            <span style="color: var(--text-muted);">●</span>
                            <span>Telegram Bot Not Configured</span>
                        </div>
                        <div style="font-size: 0.85em; color: var(--text-muted); margin-top: 4px;">
                            Configure in <a href="#" onclick="switchView('settings')" style="color: var(--accent);">Settings</a>
                        </div>
                    `;
                }

                // Get trace history and filter for telegram messages
                const traceResp = await fetch('/trace');
                const traceData = await traceResp.json();

                const telegramMessages = (traceData.history || []).filter(t => t.channel === 'telegram');
                const listEl = document.getElementById('telegramMessagesList');

                if (telegramMessages.length > 0) {
                    listEl.innerHTML = telegramMessages.map(msg => {
                        const durationText = msg.duration_ms ? (msg.duration_ms < 1000 ? `${msg.duration_ms}ms` : `${(msg.duration_ms/1000).toFixed(1)}s`) : '-';
                        return `
                            <div class="trace-history-item" onclick="showTraceDetail('${msg.id}')" style="display: flex; align-items: center; gap: 12px; padding: 12px; border-bottom: 1px solid var(--border); cursor: pointer; transition: background 0.2s;" onmouseover="this.style.background='rgba(99,102,241,0.1)'" onmouseout="this.style.background='transparent'">
                                <div style="font-size: 1.2em;">👤</div>
                                <div style="flex: 1; min-width: 0;">
                                    <div style="font-weight: 500; color: var(--text-primary);">${escapeHtml(msg.message_preview)}</div>
                                    <div style="font-size: 0.8em; color: var(--text-muted); margin-top: 2px;">
                                        ${msg.step_count} steps • ${durationText}
                                    </div>
                                </div>
                                <div style="text-align: right; flex-shrink: 0;">
                                    <div style="font-size: 0.75em; color: var(--text-muted);">${msg.started_at}</div>
                                </div>
                                <div style="color: var(--text-muted);">›</div>
                            </div>
                        `;
                    }).join('');
                } else {
                    listEl.innerHTML = '<div class="trace-empty">No Telegram messages yet. Send a message via Telegram to see it here.</div>';
                }
            } catch (error) {
                console.error('Failed to load Telegram messages:', error);
            }
        }

        // Trace - Activity Log
        async function loadTrace() {
            try {
                const response = await fetch('/trace');
                const data = await response.json();

                const historyList = document.getElementById('traceHistoryList');
                const traceCountStat = document.getElementById('traceCountStat');
                const avgDurationStat = document.getElementById('avgDurationStat');

                if (data.history && data.history.length > 0) {
                    // Update stats
                    traceCountStat.textContent = data.history.length;
                    const durations = data.history.filter(t => t.duration_ms).map(t => t.duration_ms);
                    if (durations.length > 0) {
                        const avgMs = Math.round(durations.reduce((a, b) => a + b, 0) / durations.length);
                        avgDurationStat.textContent = avgMs < 1000 ? `${avgMs}ms` : `${(avgMs/1000).toFixed(1)}s`;
                    }

                    // Render history list
                    historyList.innerHTML = data.history.map(trace => {
                        const statusIcon = trace.status === 'completed' ? '✅' : '⏳';
                        const statusColor = trace.status === 'completed' ? 'var(--success)' : 'var(--accent)';
                        const durationText = trace.duration_ms ? (trace.duration_ms < 1000 ? `${trace.duration_ms}ms` : `${(trace.duration_ms/1000).toFixed(1)}s`) : '-';
                        return `
                            <div class="trace-history-item" onclick="showTraceDetail('${trace.id}')" style="display: flex; align-items: center; gap: 12px; padding: 12px; border-bottom: 1px solid var(--border); cursor: pointer; transition: background 0.2s;" onmouseover="this.style.background='rgba(99,102,241,0.1)'" onmouseout="this.style.background='transparent'">
                                <div style="font-size: 1.2em;">${statusIcon}</div>
                                <div style="flex: 1; min-width: 0;">
                                    <div style="font-weight: 500; color: var(--text-primary); white-space: nowrap; overflow: hidden; text-overflow: ellipsis;">${escapeHtml(trace.message_preview)}</div>
                                    <div style="font-size: 0.8em; color: var(--text-muted); margin-top: 2px;">
                                        <span style="color: ${statusColor};">${trace.status}</span> • ${trace.step_count} steps • ${trace.channel}
                                    </div>
                                </div>
                                <div style="text-align: right; flex-shrink: 0;">
                                    <div style="font-size: 0.85em; color: var(--accent);">${durationText}</div>
                                    <div style="font-size: 0.75em; color: var(--text-muted);">${trace.started_at}</div>
                                </div>
                                <div style="color: var(--text-muted);">›</div>
                            </div>
                        `;
                    }).join('');
                } else {
                    historyList.innerHTML = '<div class="trace-empty">No activity recorded yet. Send a message to get started!</div>';
                    traceCountStat.textContent = '0';
                    avgDurationStat.textContent = '-';
                }
            } catch (error) {
                console.error('Failed to load trace:', error);
            }
        }

        // Show trace detail modal
        async function showTraceDetail(traceId) {
            try {
                const response = await fetch(`/trace/${encodeURIComponent(traceId)}`);
                if (!response.ok) throw new Error('Trace not found');
                const trace = await response.json();

                // Fill in modal content
                document.getElementById('traceDetailMessage').textContent = trace.message;
                document.getElementById('traceDetailChannel').textContent = trace.channel || '-';
                document.getElementById('traceDetailDuration').textContent = trace.duration_ms ?
                    (trace.duration_ms < 1000 ? `${trace.duration_ms}ms` : `${(trace.duration_ms/1000).toFixed(1)}s`) : '-';

                // Render steps timeline
                const stepsContainer = document.getElementById('traceDetailSteps');
                if (trace.steps && trace.steps.length > 0) {
                    stepsContainer.innerHTML = trace.steps.map(step => `
                        <div style="position: relative; padding-bottom: 16px;">
                            <div style="position: absolute; left: -24px; top: 0; width: 12px; height: 12px; background: var(--accent); border-radius: 50%;"></div>
                            <div style="display: flex; justify-content: space-between; align-items: flex-start;">
                                <div>
                                    <div style="font-weight: 500; color: var(--text-primary);">${step.icon} ${step.title}</div>
                                    <div style="font-size: 0.85em; color: var(--text-secondary); margin-top: 2px;">${step.detail}</div>
                                    ${step.data ? `<div style="background: var(--bg-tertiary); padding: 8px; border-radius: 6px; margin-top: 6px; font-family: 'JetBrains Mono', monospace; font-size: 0.8em; white-space: pre-wrap;">${escapeHtml(step.data)}</div>` : ''}
                                </div>
                                <div style="font-size: 0.75em; color: var(--text-muted); white-space: nowrap; margin-left: 12px;">${step.time}</div>
                            </div>
                        </div>
                    `).join('');
                } else {
                    stepsContainer.innerHTML = '<div style="color: var(--text-muted); font-style: italic;">No steps recorded</div>';
                }

                // Show response
                const responseEl = document.getElementById('traceDetailResponse');
                if (trace.response) {
                    responseEl.textContent = trace.response;
                } else {
                    responseEl.innerHTML = '<span style="color: var(--text-muted); font-style: italic;">No response recorded</span>';
                }

                // Show modal
                document.getElementById('traceDetailModal').style.display = 'flex';
            } catch (error) {
                console.error('Failed to load trace detail:', error);
                alert('Failed to load trace details: ' + error.message);
            }
        }

        function closeTraceDetail() {
            document.getElementById('traceDetailModal').style.display = 'none';
        }

        // Helper function to escape HTML
        function escapeHtml(text) {
            if (!text) return '';
            const div = document.createElement('div');
            div.textContent = text;
            return div.innerHTML;
        }

        // Show full task result in alert (simple approach)
        function showTaskResult(btn) {
            const result = decodeURIComponent(btn.dataset.result);
            alert(result);
        }

        // Toggle custom cron input
        function toggleCustomCron() {
            const scheduleSelect = document.getElementById('taskSchedule');
            const customGroup = document.getElementById('customCronGroup');
            customGroup.style.display = scheduleSelect.value === 'custom' ? 'block' : 'none';
        }

        // Validate JSON arguments in real-time
        let pendingPlan = null;
        let taskCache = new Map();
        let currentEditTaskId = null;

        function renderPlanSteps(args) {
            if (!args || !args.steps || !Array.isArray(args.steps) || args.steps.length === 0) {
                return '';
            }
            const steps = args.steps.map((step, index) => {
                const actionName = escapeHtml(step.action || 'unknown');
                const rationale = step.rationale ? `<div style="font-size: 0.8em; color: var(--text-muted); margin-top: 4px;">${escapeHtml(step.rationale)}</div>` : '';
                return `
                    <div style="padding: 8px 10px; background: var(--bg-secondary); border-radius: 8px; margin-bottom: 6px;">
                        <div style="font-weight: 600;">${index + 1}. ${actionName}</div>
                        ${rationale}
                    </div>
                `;
            }).join('');

            return `
                <div style="margin-top: 12px; padding-top: 12px; border-top: 1px solid var(--border);">
                    <div style="font-size: 0.8em; color: var(--text-muted); margin-bottom: 6px;">🧩 Planned Steps</div>
                    ${steps}
                </div>
            `;
        }

        function openTaskPlanModal() {
            const modal = document.getElementById('taskPlanModal');
            const content = document.getElementById('taskPlanContent');
            if (pendingPlan && pendingPlan.plan) {
                content.value = JSON.stringify(pendingPlan.plan, null, 2);
            }
            modal.style.display = 'flex';
        }

        function closeTaskPlanModal() {
            document.getElementById('taskPlanModal').style.display = 'none';
            document.getElementById('taskPlanStatus').innerHTML = '';
        }

        function openTaskEditModal(taskId) {
            const task = taskCache.get(taskId);
            if (!task) return;
            currentEditTaskId = taskId;
            document.getElementById('taskEditDescription').value = task.description || '';
            document.getElementById('taskEditCron').value = task.cron || '';
            document.getElementById('taskEditArguments').value = JSON.stringify(task.arguments || {}, null, 2);
            document.getElementById('taskEditStatus').innerHTML = '';
            document.getElementById('taskEditModal').style.display = 'flex';
        }

        function closeTaskEditModal() {
            currentEditTaskId = null;
            document.getElementById('taskEditModal').style.display = 'none';
            document.getElementById('taskEditStatus').innerHTML = '';
        }

        async function saveTaskEdit() {
            const statusDiv = document.getElementById('taskEditStatus');
            if (!currentEditTaskId) return;

            let args;
            try {
                args = JSON.parse(document.getElementById('taskEditArguments').value || '{}');
            } catch {
                statusDiv.innerHTML = '<div class="alert alert-error">Invalid JSON in arguments</div>';
                return;
            }

            const description = document.getElementById('taskEditDescription').value.trim();
            const cronVal = document.getElementById('taskEditCron').value.trim();

            try {
                const response = await fetch(`/tasks/${currentEditTaskId}`, {
                    method: 'POST',
                    headers: { 'Content-Type': 'application/json' },
                    body: JSON.stringify({
                        description: description || undefined,
                        arguments: args,
                        cron: cronVal || null
                    })
                });

                if (response.ok) {
                    closeTaskEditModal();
                    loadTasks();
                } else {
                    const error = await response.json();
                    statusDiv.innerHTML = `<div class="alert alert-error">${error.error || 'Failed to update task'}</div>`;
                }
            } catch (error) {
                statusDiv.innerHTML = `<div class="alert alert-error">Error: ${error.message}</div>`;
            }
        }

        async function deleteTask(taskId) {
            if (!confirm('Delete this task? This cannot be undone.')) {
                return;
            }
            try {
                const response = await fetch(`/tasks/${taskId}`, { method: 'DELETE' });
                if (response.ok) {
                    loadTasks();
                } else {
                    const error = await response.json();
                    alert('Failed to delete task: ' + (error.error || 'Unknown error'));
                }
            } catch (error) {
                alert('Failed to delete task: ' + error.message);
            }
        }

        async function saveTaskPlan() {
            const statusDiv = document.getElementById('taskPlanStatus');
            const planText = document.getElementById('taskPlanContent').value;
            const requireApproval = document.getElementById('taskRequireApproval').checked;

            let planJson;
            try {
                planJson = JSON.parse(planText);
            } catch {
                statusDiv.innerHTML = '<div class="alert alert-error">Plan JSON is invalid</div>';
                return;
            }

            const taskData = {
                description: pendingPlan?.description || 'Planned Task',
                action: 'plan',
                arguments: planJson,
                approval: requireApproval ? 'require' : 'auto'
            };
            if (pendingPlan?.cron) {
                taskData.cron = pendingPlan.cron;
            }

            try {
                const response = await fetch('/tasks', {
                    method: 'POST',
                    headers: { 'Content-Type': 'application/json' },
                    body: JSON.stringify(taskData)
                });

                if (response.ok) {
                    closeTaskPlanModal();
                    document.getElementById('taskDescription').value = '';
                    document.getElementById('taskSchedule').value = '';
                    document.getElementById('taskCustomCron').value = '';
                    document.getElementById('customCronGroup').style.display = 'none';
                    document.getElementById('taskRefinePrompt').value = '';
                    document.getElementById('taskRequireApproval').checked = false;
                    pendingPlan = null;
                    loadTasks();
                } else {
                    const error = await response.json();
                    statusDiv.innerHTML = `<div class="alert alert-error">${error.error || 'Failed to save task'}</div>`;
                }
            } catch (error) {
                statusDiv.innerHTML = `<div class="alert alert-error">Error: ${error.message}</div>`;
            }
        }

        async function regenerateTaskPlan() {
            const statusDiv = document.getElementById('taskPlanStatus');
            const prompt = document.getElementById('taskPlanRegeneratePrompt').value;
            const regenBtn = document.querySelector('#taskPlanModal .modal-footer button[onclick="regenerateTaskPlan()"]');

            if (!pendingPlan || !pendingPlan.description) {
                statusDiv.innerHTML = '<div class="alert alert-error">Missing task description</div>';
                return;
            }

            // Disable button and show loading state
            regenBtn.disabled = true;
            const originalText = regenBtn.textContent;
            regenBtn.innerHTML = '<span class="spinner"></span> Regenerating...';
            statusDiv.innerHTML = '<div class="alert" style="background: rgba(99, 102, 241, 0.1); border-color: var(--accent);">🧠 Regenerating plan...</div>';

            try {
                const response = await fetch('/tasks/plan', {
                    method: 'POST',
                    headers: { 'Content-Type': 'application/json' },
                    body: JSON.stringify({ description: pendingPlan.description, prompt })
                });

                if (response.ok) {
                    const data = await response.json();
                    pendingPlan.plan = data.plan;
                    document.getElementById('taskPlanContent').value = JSON.stringify(data.plan, null, 2);
                    statusDiv.innerHTML = '<div class="alert alert-success">Plan updated</div>';
                    setTimeout(() => statusDiv.innerHTML = '', 2000);
                } else {
                    const error = await response.json();
                    statusDiv.innerHTML = `<div class="alert alert-error">${error.error || 'Failed to regenerate plan'}</div>`;
                }
            } catch (error) {
                statusDiv.innerHTML = `<div class="alert alert-error">Error: ${error.message}</div>`;
            } finally {
                // Re-enable button
                regenBtn.disabled = false;
                regenBtn.textContent = originalText;
            }
        }

        // Task creation form
        document.getElementById('createTaskForm').addEventListener('submit', async (e) => {
            e.preventDefault();
            const statusDiv = document.getElementById('taskStatus');
            const submitBtn = e.target.querySelector('button[type="submit"]');

            const description = document.getElementById('taskDescription').value;
            let schedule = document.getElementById('taskSchedule').value;
            const customCron = document.getElementById('taskCustomCron').value;
            const refinePrompt = document.getElementById('taskRefinePrompt').value;

            // Use custom cron if selected
            if (schedule === 'custom') {
                if (!customCron.trim()) {
                    statusDiv.innerHTML = '<div class="alert alert-error">Please enter a custom cron expression</div>';
                    return;
                }
                schedule = customCron.trim();
            }

            // Disable button and show loading state
            submitBtn.disabled = true;
            const originalText = submitBtn.textContent;
            submitBtn.innerHTML = '<span class="spinner"></span> Generating Plan...';
            statusDiv.innerHTML = '<div class="alert" style="background: rgba(99, 102, 241, 0.1); border-color: var(--accent);">🧠 Planning with LLM... This may take a moment.</div>';

            try {
                const response = await fetch('/tasks/plan', {
                    method: 'POST',
                    headers: { 'Content-Type': 'application/json' },
                    body: JSON.stringify({ description, prompt: refinePrompt })
                });

                if (response.ok) {
                    const data = await response.json();
                    pendingPlan = {
                        description,
                        cron: schedule || null,
                        plan: data.plan
                    };
                    statusDiv.innerHTML = '';
                    openTaskPlanModal();
                } else {
                    const error = await response.json();
                    statusDiv.innerHTML = `<div class="alert alert-error">${error.error || 'Failed to generate plan'}</div>`;
                }
            } catch (error) {
                statusDiv.innerHTML = `<div class="alert alert-error">Error: ${error.message}</div>`;
            } finally {
                // Re-enable button
                submitBtn.disabled = false;
                submitBtn.textContent = originalText;
            }
        });

        function getScheduleLabel(cron) {
            const labels = {
                '*/5 * * * *': 'Every 5 min',
                '*/10 * * * *': 'Every 10 min',
                '*/30 * * * *': 'Every 30 min',
                '0 * * * *': 'Hourly',
                '0 */2 * * *': 'Every 2 hours',
                '0 */6 * * *': 'Every 6 hours',
                '0 9 * * *': 'Daily 9 AM',
                '0 21 * * *': 'Nightly 9 PM',
                '0 0 * * *': 'Midnight',
                '0 9 * * 1': 'Weekly',
                '0 9 * * 1-5': 'Weekdays',
                '0 9 1 * *': 'Monthly',
            };
            return labels[cron] || cron;
        }

        // Status
        async function loadStatus() {
            try {
                const response = await fetch('/status');
                const data = await response.json();

                document.getElementById('memoryCount').textContent = data.memory_entries || 0;
                document.getElementById('actionsCount').textContent = data.actions_loaded || 0;
                document.getElementById('tasksCount').textContent = data.tasks_pending || 0;
                document.getElementById('agentDid').textContent = data.did || 'Unknown';
                document.getElementById('agentVersion').textContent = `v${data.version}` || 'Unknown';

                updateConnectionStatus(true);
            } catch (error) {
                console.error('Failed to load status:', error);
                updateConnectionStatus(false);
            }
        }

        function updateConnectionStatus(connected) {
            connectionStatus.classList.toggle('error', !connected);
            connectionText.textContent = connected ? 'Connected' : 'Disconnected';
        }

        // Health check polling
        async function healthCheck() {
            try {
                const response = await fetch('/health');
                updateConnectionStatus(response.ok);
            } catch {
                updateConnectionStatus(false);
            }
        }

        // Settings
        const botName = document.getElementById('botName');
        const llmProvider = document.getElementById('llmProvider');
        const llmBaseUrl = document.getElementById('llmBaseUrl');
        const llmApiKey = document.getElementById('llmApiKey');
        const llmModel = document.getElementById('llmModel');
        const baseUrlGroup = document.getElementById('baseUrlGroup');
        const apiKeyGroup = document.getElementById('apiKeyGroup');
        const apiKeyStatus = document.getElementById('apiKeyStatus');
        const dailyTimezone = document.getElementById('dailyTimezone');
        const dailyLanguage = document.getElementById('dailyLanguage');
        const dailyTone = document.getElementById('dailyTone');
        const dailyEmailFormat = document.getElementById('dailyEmailFormat');
        const dailyBriefChannel = document.getElementById('dailyBriefChannel');
        const telegramEnabled = document.getElementById('telegramEnabled');
        const telegramSettings = document.getElementById('telegramSettings');
        const settingsForm = document.getElementById('settingsForm');
        const settingsStatus = document.getElementById('settingsStatus');
        const saveSettingsBtn = document.getElementById('saveSettingsBtn');
        const settingsLockNotice = document.getElementById('settingsLockNotice');

        function buildSettingsPayload() {
            // Map "openrouter" to "openai-compatible" for backend
            let provider = llmProvider.value;
            if (provider === 'openrouter') {
                provider = 'openai-compatible';
            }

            const botPersonality = document.getElementById('botPersonality');
            const tzValue = dailyTimezone ? dailyTimezone.value.trim() : '';
            const langValue = dailyLanguage ? dailyLanguage.value.trim() : '';
            const settings = {
                bot_name: botName.value || 'CogniArk',
                personality: botPersonality.value || 'friendly',
                timezone: tzValue || null,
                language: langValue || null,
                tone: dailyTone ? (dailyTone.value || null) : null,
                email_format: dailyEmailFormat ? (dailyEmailFormat.value || null) : null,
                daily_brief_channel: dailyBriefChannel ? dailyBriefChannel.value : 'telegram',
                llm_provider: provider,
                llm_model: llmModel.value,
                llm_base_url: llmBaseUrl.value || null,
                telegram_enabled: telegramEnabled.checked,
            };

            // Only send API key if it's not empty (user entered a new one)
            if (llmApiKey.value) {
                settings.llm_api_key = llmApiKey.value;
            }

            if (telegramEnabled.checked) {
                const tokenEl = document.getElementById('telegramToken');
                if (tokenEl.value) {
                    settings.telegram_bot_token = tokenEl.value;
                }
                const usersEl = document.getElementById('telegramUsers');
                settings.telegram_allowed_users = usersEl.value
                    ? usersEl.value.split(',').map(s => parseInt(s.trim())).filter(n => !isNaN(n))
                    : [];
            } else {
                settings.telegram_allowed_users = [];
            }

            // Media generation provider keys (only if user entered new values)
            const mediaKeys = {
                replicate: document.getElementById('replicateKey'),
                fal: document.getElementById('falKey'),
                stability_ai: document.getElementById('stabilityKey'),
                together: document.getElementById('togetherKey'),
                openai_dalle: document.getElementById('dalleKey'),
                google_gemini: document.getElementById('googleAiKey'),
                runway: document.getElementById('runwayKey'),
                luma: document.getElementById('lumaKey'),
            };

            settings.media_providers = {};
            for (const [key, el] of Object.entries(mediaKeys)) {
                if (el && el.value) {
                    settings.media_providers[key] = el.value;
                    // OpenAI key is shared between DALL-E and Sora
                    if (key === 'openai_dalle') {
                        settings.media_providers['openai_sora'] = el.value;
                    }
                    // Google key is shared between Gemini and Veo
                    if (key === 'google_gemini') {
                        settings.media_providers['google_veo'] = el.value;
                    }
                }
            }

            // Default/fallback media providers
            const defaultImageEl = document.getElementById('defaultImageProvider');
            const fallbackImageEl = document.getElementById('fallbackImageProvider');
            const defaultVideoEl = document.getElementById('defaultVideoProvider');
            const fallbackVideoEl = document.getElementById('fallbackVideoProvider');
            if (defaultImageEl.value) settings.default_image_provider = defaultImageEl.value;
            if (fallbackImageEl.value) settings.fallback_image_provider = fallbackImageEl.value;
            if (defaultVideoEl.value) settings.default_video_provider = defaultVideoEl.value;
            if (fallbackVideoEl.value) settings.fallback_video_provider = fallbackVideoEl.value;

            // Fallback LLM settings
            const llmFallbackProvider = document.getElementById('llmFallbackProvider');
            const llmFallbackModel = document.getElementById('llmFallbackModel');
            const llmFallbackBaseUrl = document.getElementById('llmFallbackBaseUrl');
            const llmFallbackApiKey = document.getElementById('llmFallbackApiKey');

            if (llmFallbackProvider.value) {
                settings.llm_fallback_provider = llmFallbackProvider.value;
                settings.llm_fallback_model = llmFallbackModel.value || null;
                settings.llm_fallback_base_url = llmFallbackBaseUrl.value || null;
                if (llmFallbackApiKey.value) {
                    settings.llm_fallback_api_key = llmFallbackApiKey.value;
                }
            }

            return settings;
        }

        function settingsFingerprint(settings) {
            const clone = { ...settings };
            // Do not include transient inputs
            delete clone.llm_api_key;
            delete clone.telegram_bot_token;
            delete clone.media_providers;
            return JSON.stringify(clone);
        }

        function setSettingsDirty(isDirty) {
            saveSettingsBtn.disabled = !isDirty;
        }

        function applySettingsLock(locked) {
            console.log('[CogniArk] Settings lock:', locked ? 'LOCKED (setup incomplete)' : 'UNLOCKED');
            settingsLocked = locked;
            settingsLockNotice.style.display = locked ? 'block' : 'none';
            document.querySelectorAll('.nav-item').forEach(item => {
                const view = item.dataset.view;
                const shouldDisable = locked && view !== 'settings';
                item.classList.toggle('disabled', shouldDisable);
            });
            if (locked && currentView !== 'settings') {
                switchView('settings');
            }
        }

        function recomputeSettingsDirty() {
            if (!settingsBaseline) {
                setSettingsDirty(false);
                return;
            }
            const current = buildSettingsPayload();
            const fingerprint = settingsFingerprint({
                bot_name: current.bot_name,
                personality: current.personality,
                timezone: current.timezone,
                language: current.language,
                tone: current.tone,
                email_format: current.email_format,
                daily_brief_channel: current.daily_brief_channel,
                llm_provider: current.llm_provider,
                llm_model: current.llm_model,
                llm_base_url: current.llm_base_url,
                telegram_enabled: current.telegram_enabled,
                telegram_allowed_users: current.telegram_allowed_users || []
            });
            setSettingsDirty(fingerprint !== settingsBaseline);
        }

        llmProvider.addEventListener('change', () => {
            updateProviderUI();
            recomputeSettingsDirty();
        });
        telegramEnabled.addEventListener('change', () => {
            telegramSettings.style.display = telegramEnabled.checked ? 'block' : 'none';
            recomputeSettingsDirty();
        });

        [botName, llmModel, llmBaseUrl, llmApiKey].forEach(el => {
            el.addEventListener('input', recomputeSettingsDirty);
        });
        if (dailyTimezone) dailyTimezone.addEventListener('input', recomputeSettingsDirty);
        if (dailyLanguage) dailyLanguage.addEventListener('input', recomputeSettingsDirty);
        if (dailyTone) dailyTone.addEventListener('change', recomputeSettingsDirty);
        if (dailyEmailFormat) dailyEmailFormat.addEventListener('change', recomputeSettingsDirty);
        if (dailyBriefChannel) dailyBriefChannel.addEventListener('change', recomputeSettingsDirty);

        const telegramToken = document.getElementById('telegramToken');
        const telegramUsers = document.getElementById('telegramUsers');
        if (telegramToken) telegramToken.addEventListener('input', recomputeSettingsDirty);
        if (telegramUsers) telegramUsers.addEventListener('input', recomputeSettingsDirty);

        // Track personality dropdown changes
        const botPersonalityEl = document.getElementById('botPersonality');
        if (botPersonalityEl) botPersonalityEl.addEventListener('change', recomputeSettingsDirty);

        function updateProviderUI() {
            const provider = llmProvider.value;

            // Show/hide base URL
            if (provider === 'ollama') {
                baseUrlGroup.style.display = 'block';
                llmBaseUrl.placeholder = 'http://localhost:11434';
                if (!llmBaseUrl.value) llmBaseUrl.value = 'http://localhost:11434';
            } else if (provider === 'openrouter') {
                baseUrlGroup.style.display = 'block';
                llmBaseUrl.value = 'https://openrouter.ai/api/v1';
                llmBaseUrl.placeholder = 'https://openrouter.ai/api/v1';
            } else if (provider === 'openai-compatible') {
                baseUrlGroup.style.display = 'block';
                llmBaseUrl.placeholder = 'https://your-api.com/v1';
            } else {
                baseUrlGroup.style.display = 'none';
                llmBaseUrl.value = '';
            }

            // Show/hide API key
            if (provider === 'ollama') {
                apiKeyGroup.style.display = 'none';
            } else {
                apiKeyGroup.style.display = 'block';
            }

            // Update model placeholder
            switch (provider) {
                case 'ollama':
                    llmModel.placeholder = 'llama3.2, qwen2.5, etc.';
                    break;
                case 'anthropic':
                    llmModel.placeholder = 'claude-sonnet-4-20250514';
                    break;
                case 'openai':
                    llmModel.placeholder = 'gpt-4o, gpt-4-turbo';
                    break;
                case 'openrouter':
                    llmModel.placeholder = 'glm-4, qwen/qwen-2.5-72b-instruct';
                    if (!llmModel.value) llmModel.value = 'glm-4';
                    break;
                case 'openai-compatible':
                    llmModel.placeholder = 'your-model-name';
                    break;
            }
        }

        function updateFallbackProviderUI() {
            const fallbackProvider = document.getElementById('llmFallbackProvider');
            const fallbackBaseUrlGroup = document.getElementById('fallbackBaseUrlGroup');
            const fallbackApiKeyGroup = document.getElementById('fallbackApiKeyGroup');
            const fallbackModelGroup = document.getElementById('fallbackModelGroup');
            const fallbackBaseUrl = document.getElementById('llmFallbackBaseUrl');
            const fallbackModel = document.getElementById('llmFallbackModel');

            const provider = fallbackProvider.value;

            if (!provider) {
                // No fallback selected - hide all fields
                fallbackBaseUrlGroup.style.display = 'none';
                fallbackApiKeyGroup.style.display = 'none';
                fallbackModelGroup.style.display = 'none';
                return;
            }

            // Show model field for all providers
            fallbackModelGroup.style.display = 'block';

            // Show/hide base URL
            if (provider === 'ollama') {
                fallbackBaseUrlGroup.style.display = 'block';
                fallbackBaseUrl.placeholder = 'http://localhost:11434';
            } else if (provider === 'openrouter') {
                fallbackBaseUrlGroup.style.display = 'block';
                fallbackBaseUrl.value = 'https://openrouter.ai/api/v1';
            } else if (provider === 'openai-compatible') {
                fallbackBaseUrlGroup.style.display = 'block';
                fallbackBaseUrl.placeholder = 'https://your-api.com/v1';
            } else {
                fallbackBaseUrlGroup.style.display = 'none';
                fallbackBaseUrl.value = '';
            }

            // Show/hide API key
            if (provider === 'ollama') {
                fallbackApiKeyGroup.style.display = 'none';
            } else {
                fallbackApiKeyGroup.style.display = 'block';
            }

            // Update model placeholder
            switch (provider) {
                case 'ollama': fallbackModel.placeholder = 'llama3.2, qwen2.5'; break;
                case 'anthropic': fallbackModel.placeholder = 'claude-sonnet-4-20250514'; break;
                case 'openai': fallbackModel.placeholder = 'gpt-4o, gpt-4-turbo'; break;
                case 'openrouter': fallbackModel.placeholder = 'glm-4, qwen/qwen-2.5-72b-instruct'; break;
                case 'openai-compatible': fallbackModel.placeholder = 'your-model-name'; break;
            }
        }

        // Add event listener for fallback provider change
        document.getElementById('llmFallbackProvider').addEventListener('change', () => {
            updateFallbackProviderUI();
            recomputeSettingsDirty();
        });

        async function loadSettings() {
            try {
                const response = await fetch('/settings');
                const data = await response.json();

                // Map openai-compatible with openrouter URL to "openrouter" option
                let provider = data.llm_provider || 'ollama';
                if (provider === 'openai-compatible' && data.llm_base_url && data.llm_base_url.includes('openrouter')) {
                    provider = 'openrouter';
                }

                // Load bot name and personality
                botName.value = data.bot_name || 'CogniArk';
                // Note: Header logo stays as "CogniArk" - bot name is for agent personality
                const botPersonality = document.getElementById('botPersonality');
                botPersonality.value = data.personality || 'friendly';
                if (dailyTimezone) {
                    const detected = Intl.DateTimeFormat().resolvedOptions().timeZone || '';
                    const tzValue = data.timezone || detected || '';
                    if (tzValue) {
                        ensureTimezoneOption(tzValue);
                    }
                    dailyTimezone.value = tzValue;
                }
                if (dailyLanguage) {
                    dailyLanguage.value = data.language || '';
                }
                if (dailyTone) {
                    dailyTone.value = data.tone || '';
                }
                if (dailyEmailFormat) {
                    dailyEmailFormat.value = data.email_format || '';
                }
                if (dailyBriefChannel) {
                    dailyBriefChannel.value = data.daily_brief_channel || 'telegram';
                }

                llmProvider.value = provider;
                llmModel.value = data.llm_model || '';
                llmBaseUrl.value = data.llm_base_url || '';

                if (data.has_api_key) {
                    apiKeyStatus.textContent = '✓ API key is configured';
                    apiKeyStatus.style.color = 'var(--success)';
                    llmApiKey.placeholder = '••••••••••••••••';
                } else {
                    apiKeyStatus.textContent = '⚠ No API key set';
                    apiKeyStatus.style.color = 'var(--warning)';
                    llmApiKey.placeholder = 'Enter API key...';
                }

                // Load fallback LLM settings
                const fallbackProvider = document.getElementById('llmFallbackProvider');
                const fallbackModel = document.getElementById('llmFallbackModel');
                const fallbackBaseUrl = document.getElementById('llmFallbackBaseUrl');
                const fallbackApiKeyStatus = document.getElementById('fallbackApiKeyStatus');
                const fallbackApiKey = document.getElementById('llmFallbackApiKey');

                if (data.llm_fallback_provider) {
                    let fbProvider = data.llm_fallback_provider;
                    if (fbProvider === 'openai-compatible' && data.llm_fallback_base_url && data.llm_fallback_base_url.includes('openrouter')) {
                        fbProvider = 'openrouter';
                    }
                    fallbackProvider.value = fbProvider;
                    fallbackModel.value = data.llm_fallback_model || '';
                    fallbackBaseUrl.value = data.llm_fallback_base_url || '';

                    if (data.has_fallback_api_key) {
                        fallbackApiKeyStatus.textContent = '✓ API key is configured';
                        fallbackApiKeyStatus.style.color = 'var(--success)';
                        fallbackApiKey.placeholder = '••••••••••••••••';
                    } else {
                        fallbackApiKeyStatus.textContent = '⚠ No API key set';
                        fallbackApiKeyStatus.style.color = 'var(--warning)';
                        fallbackApiKey.placeholder = 'Enter API key...';
                    }
                } else {
                    fallbackProvider.value = '';
                }
                updateFallbackProviderUI();

                telegramEnabled.checked = data.telegram_enabled || false;
                telegramSettings.style.display = data.telegram_enabled ? 'block' : 'none';

                // Show telegram token status
                const telegramTokenStatus = document.getElementById('telegramTokenStatus');
                const telegramTokenInput = document.getElementById('telegramToken');
                if (data.has_telegram_token) {
                    telegramTokenStatus.textContent = '✓ Bot token is configured';
                    telegramTokenStatus.style.color = 'var(--success)';
                    telegramTokenInput.placeholder = '••••••••••••••••';
                } else {
                    telegramTokenStatus.textContent = '⚠ No bot token set';
                    telegramTokenStatus.style.color = 'var(--warning)';
                    telegramTokenInput.placeholder = 'From @BotFather';
                }

                if (data.telegram_allowed_users && data.telegram_allowed_users.length > 0) {
                    document.getElementById('telegramUsers').value = data.telegram_allowed_users.join(', ');
                } else {
                    document.getElementById('telegramUsers').value = '';
                }

                updateProviderUI();
                const baseline = {
                    bot_name: botName.value || 'CogniArk',
                    personality: botPersonality.value || 'friendly',
                    timezone: dailyTimezone ? (dailyTimezone.value || null) : null,
                    language: dailyLanguage ? (dailyLanguage.value || null) : null,
                    tone: dailyTone ? (dailyTone.value || null) : null,
                    email_format: dailyEmailFormat ? (dailyEmailFormat.value || null) : null,
                    daily_brief_channel: dailyBriefChannel ? dailyBriefChannel.value : 'telegram',
                    llm_provider: provider,
                    llm_model: llmModel.value,
                    llm_base_url: llmBaseUrl.value || null,
                    telegram_enabled: telegramEnabled.checked,
                    telegram_allowed_users: data.telegram_allowed_users || []
                };
                settingsBaseline = settingsFingerprint(baseline);
                setSettingsDirty(false);
                console.log('[CogniArk] Settings loaded: settings_complete=' + data.settings_complete + ', bot_name=' + (data.bot_name || '(empty)') + ', llm_provider=' + (data.llm_provider || '(empty)') + ', has_api_key=' + data.has_api_key);
                applySettingsLock(!data.settings_complete);

                // Load media provider status
                loadMediaProviderStatus();

                // Load Gmail status
                refreshGmailStatus();

                // Load integrations
                loadIntegrations();
            } catch (error) {
                console.error('Failed to load settings:', error);
                showSettingsStatus('Failed to load settings: ' + error.message, 'error');
            }
        }

        // Load external integrations
        async function loadIntegrations() {
            const container = document.getElementById('integrationsList');
            try {
                const response = await fetch('/integrations');
                const data = await response.json();

                if (!data.integrations || data.integrations.length === 0) {
                    container.innerHTML = '<p style="color: var(--text-secondary); font-style: italic;">No integrations available</p>';
                    return;
                }

                let html = '';
                for (const int of data.integrations) {
                    const statusClass = int.status === 'connected' ? 'success' : (int.status === 'needs_auth' ? 'warning' : 'secondary');
                    const statusText = int.status === 'connected' ? 'Connected' :
                                       int.status === 'needs_auth' ? 'Not Connected' :
                                       int.status === 'not_configured' ? 'Not Configured' : 'Error';

                    let actionBtn = '';
                    if (int.status === 'needs_auth' && int.auth_url) {
                        actionBtn = `<a href="${int.auth_url}" target="_blank" class="btn btn-sm" style="background: var(--accent);">Connect</a>`;
                    } else if (int.status === 'connected') {
                        actionBtn = `<button onclick="disconnectIntegration('${int.id}')" class="btn btn-sm" style="background: var(--error);">Disconnect</button>`;
                    } else if (int.status === 'not_configured') {
                        actionBtn = `<button onclick="openConfigureModal('${int.id}', '${escapeHtml(int.name)}')" class="btn btn-sm" style="background: var(--accent);">Configure</button>`;
                    }

                    html += `
                        <div class="integration-item" style="display: flex; align-items: center; justify-content: space-between; padding: 12px; background: var(--bg-tertiary); border-radius: 8px; margin-bottom: 8px;">
                            <div style="display: flex; align-items: center; gap: 12px;">
                                <span style="font-size: 1.5em;">${int.icon}</span>
                                <div>
                                    <strong>${int.name}</strong>
                                    <div style="font-size: 0.85em; color: var(--text-secondary);">${int.description}</div>
                                </div>
                            </div>
                            <div style="display: flex; align-items: center; gap: 12px;">
                                <span class="status-badge" style="background: var(--${statusClass}); padding: 4px 8px; border-radius: 4px; font-size: 0.8em;">${statusText}</span>
                                ${actionBtn}
                            </div>
                        </div>
                    `;
                }
                container.innerHTML = html;
            } catch (error) {
                console.error('Failed to load integrations:', error);
                container.innerHTML = '<p style="color: var(--error);">Failed to load integrations</p>';
            }
        }

        async function disconnectIntegration(id) {
            if (!confirm('Disconnect this integration?')) return;
            try {
                await fetch(`/integrations/${id}/disconnect`, { method: 'POST' });
                loadIntegrations();
            } catch (error) {
                console.error('Failed to disconnect:', error);
                alert('Failed to disconnect: ' + error.message);
            }
        }

        function openConfigureModal(id, name) {
            document.getElementById('configIntId').value = id;
            document.getElementById('configIntTitle').textContent = 'Configure ' + name;
            document.getElementById('configClientId').value = '';
            document.getElementById('configClientSecret').value = '';
            document.getElementById('configureIntModal').style.display = 'flex';
        }
        function closeConfigureModal() {
            document.getElementById('configureIntModal').style.display = 'none';
        }
        async function saveIntegrationConfig() {
            const id = document.getElementById('configIntId').value;
            const clientId = document.getElementById('configClientId').value.trim();
            const clientSecret = document.getElementById('configClientSecret').value.trim();
            if (!clientId || !clientSecret) { alert('Both Client ID and Client Secret are required'); return; }
            try {
                const resp = await fetch('/integrations/' + id + '/configure', {
                    method: 'POST',
                    headers: {'Content-Type': 'application/json'},
                    body: JSON.stringify({ client_id: clientId, client_secret: clientSecret })
                });
                if (resp.ok) {
                    closeConfigureModal();
                    loadIntegrations();
                } else {
                    const err = await resp.json();
                    alert('Failed: ' + (err.error || 'Unknown error'));
                }
            } catch (e) {
                alert('Failed: ' + e.message);
            }
        }

        // Load and display media provider configuration status
        async function loadMediaProviderStatus() {
            try {
                const response = await fetch('/settings/media');
                if (!response.ok) return; // Endpoint might not exist yet

                const data = await response.json();

                // Map of provider keys to input element IDs
                const providerFields = {
                    replicate: 'replicateKey',
                    fal: 'falKey',
                    stability_ai: 'stabilityKey',
                    together: 'togetherKey',
                    openai_dalle: 'dalleKey',
                    google_gemini: 'googleAiKey',
                    runway: 'runwayKey',
                    luma: 'lumaKey',
                };

                // Update each field's placeholder and hint based on configured status
                for (const [provider, fieldId] of Object.entries(providerFields)) {
                    const field = document.getElementById(fieldId);
                    if (!field) continue;

                    const isConfigured = data.configured && data.configured.includes(provider);
                    const hint = field.nextElementSibling;

                    if (isConfigured) {
                        field.placeholder = '••••••••••••••••';
                        if (hint && !hint.querySelector('.configured-badge')) {
                            const badge = document.createElement('span');
                            badge.className = 'configured-badge';
                            badge.style.cssText = 'color: var(--success); margin-left: 8px;';
                            badge.textContent = '✓ Configured';
                            hint.insertBefore(badge, hint.firstChild);
                        }
                    }
                }

                // Set default/fallback providers if returned
                if (data.default_image_provider) {
                    const el = document.getElementById('defaultImageProvider');
                    if (el) el.value = data.default_image_provider;
                }
                if (data.fallback_image_provider) {
                    const el = document.getElementById('fallbackImageProvider');
                    if (el) el.value = data.fallback_image_provider;
                }
                if (data.default_video_provider) {
                    const el = document.getElementById('defaultVideoProvider');
                    if (el) el.value = data.default_video_provider;
                }
                if (data.fallback_video_provider) {
                    const el = document.getElementById('fallbackVideoProvider');
                    if (el) el.value = data.fallback_video_provider;
                }
            } catch (error) {
                console.log('Media provider status not available:', error.message);
            }
        }

        // Add event listeners for media provider fields to track dirty state
        ['replicateKey', 'falKey', 'stabilityKey', 'togetherKey', 'dalleKey', 'googleAiKey', 'runwayKey', 'lumaKey', 'defaultImageProvider', 'fallbackImageProvider', 'defaultVideoProvider', 'fallbackVideoProvider', 'llmFallbackProvider', 'llmFallbackModel', 'llmFallbackBaseUrl', 'llmFallbackApiKey'].forEach(id => {
            const el = document.getElementById(id);
            if (el) el.addEventListener('input', recomputeSettingsDirty);
            if (el) el.addEventListener('change', recomputeSettingsDirty);
        });

        settingsForm.addEventListener('submit', async (e) => {
            e.preventDefault();

            const settings = buildSettingsPayload();

            try {
                const response = await fetch('/settings', {
                    method: 'POST',
                    headers: { 'Content-Type': 'application/json' },
                    body: JSON.stringify(settings)
                });

                const data = await response.json();

                if (response.ok) {
                    showSettingsStatus('✓ Settings saved successfully!', 'success');
                    llmApiKey.value = '';
                    document.getElementById('telegramToken').value = '';
                    settingsBaseline = settingsFingerprint({
                        bot_name: settings.bot_name,
                        personality: settings.personality,
                        llm_provider: settings.llm_provider,
                        llm_model: settings.llm_model,
                        llm_base_url: settings.llm_base_url,
                        telegram_enabled: settings.telegram_enabled,
                        telegram_allowed_users: settings.telegram_allowed_users || []
                    });
                    setSettingsDirty(false);
                    applySettingsLock(false);
                    loadSettings();
                } else {
                    showSettingsStatus('Error: ' + (data.error || 'Unknown error'), 'error');
                }
            } catch (error) {
                showSettingsStatus('Failed to save: ' + error.message, 'error');
            }
        });

        function showSettingsStatus(message, type) {
            settingsStatus.innerHTML = `<div class="alert alert-${type}">${message}</div>`;
            setTimeout(() => settingsStatus.innerHTML = '', 5000);
        }


        async function refreshGmailStatus() {
            const statusEl = document.getElementById('gmailStatus');
            try {
                const resp = await fetch('/gmail/status');
                const data = await resp.json();
                if (data.connected) {
                    statusEl.innerHTML = '<span style="color: var(--success);">✓ Connected</span>';
                    document.getElementById('gmailConnectBtn').textContent = 'Reconnect';
                    statusEl.style.color = 'var(--success)';
                } else {
                    statusEl.textContent = 'Not connected';
                    statusEl.style.color = 'var(--warning)';
                }
            } catch (error) {
                statusEl.textContent = 'Failed to check Gmail status';
                statusEl.style.color = 'var(--error)';
            }
        }

        function ensureTimezoneOption(value) {
            if (!dailyTimezone || !dailyTimezone.list || !value) return;
            const exists = dailyTimezone.list.querySelector(`option[value="${value}"]`);
            if (!exists) {
                const opt = document.createElement('option');
                opt.value = value;
                dailyTimezone.list.appendChild(opt);
            }
        }

        async function connectGmail() {
            const clientId = document.getElementById('gmailClientId').value.trim();
            const clientSecret = document.getElementById('gmailClientSecret').value.trim();
            if (!clientId || !clientSecret) {
                showSettingsStatus('Enter both Client ID and Client Secret first.', 'error');
                return;
            }
            const btn = document.getElementById('gmailConnectBtn');
            btn.disabled = true;
            btn.textContent = 'Saving...';
            try {
                // Step 1: Save credentials
                const saveResp = await fetch('/gmail/configure', {
                    method: 'POST',
                    headers: {'Content-Type': 'application/json'},
                    body: JSON.stringify({ client_id: clientId, client_secret: clientSecret })
                });
                if (!saveResp.ok) {
                    const err = await saveResp.json();
                    showSettingsStatus('Failed to save credentials: ' + (err.error || 'Unknown error'), 'error');
                    btn.disabled = false; btn.textContent = 'Connect Gmail';
                    return;
                }
                // Step 2: Get auth URL and redirect
                btn.textContent = 'Redirecting...';
                const resp = await fetch('/gmail/oauth/start', { method: 'POST' });
                const data = await resp.json();
                if (!resp.ok) {
                    showSettingsStatus(data.error || 'Failed to start Gmail OAuth.', 'error');
                    btn.disabled = false; btn.textContent = 'Connect Gmail';
                    return;
                }
                // Open Google auth in new tab
                window.open(data.auth_url, '_blank');
                showSettingsStatus('Google authorization opened in new tab. Complete it there, then come back.', 'success');
                btn.disabled = false; btn.textContent = 'Connect Gmail';
                // Poll for connection status
                const poll = setInterval(async () => {
                    try {
                        const s = await fetch('/gmail/status');
                        const d = await s.json();
                        if (d.connected) {
                            clearInterval(poll);
                            showSettingsStatus('Gmail connected!', 'success');
                            refreshGmailStatus();
                        }
                    } catch(e) {}
                }, 3000);
            } catch (e) {
                showSettingsStatus('Error: ' + e.message, 'error');
                btn.disabled = false; btn.textContent = 'Connect Gmail';
            }
        }


        // Restart the server
        async function restartServer() {
            if (!confirm('Restart the bot? This will briefly disconnect all services.')) {
                return;
            }

            showSettingsStatus('🔄 Restarting...', 'warning');

            try {
                await fetch('/restart', { method: 'POST' });
                // Server will restart, so we'll lose connection
                showSettingsStatus('✓ Restart initiated. Reconnecting...', 'success');

                // Poll for server to come back up
                let attempts = 0;
                const checkServer = setInterval(async () => {
                    attempts++;
                    try {
                        const resp = await fetch('/health');
                        if (resp.ok) {
                            clearInterval(checkServer);
                            showSettingsStatus('✓ Bot restarted successfully!', 'success');
                            loadSettings();
                            updateConnectionStatus(true);
                        }
                    } catch {
                        if (attempts > 30) {
                            clearInterval(checkServer);
                            showSettingsStatus('Restart taking longer than expected. Please refresh the page.', 'warning');
                        }
                    }
                }, 1000);
            } catch (error) {
                // Expected - server is restarting
                showSettingsStatus('🔄 Restarting... Please wait.', 'warning');
            }
        }

        // Load user profile and set appropriate welcome message
        async function loadProfile() {
            try {
                const response = await fetch('/profile');
                const profile = await response.json();

                const messagesEl = document.getElementById('messages');

                if (profile.onboarding_complete && profile.name) {
                    // User already introduced - show friendly welcome back
                    messagesEl.innerHTML = `
                        <div class="message assistant">
                            <strong>Welcome back${profile.name ? ', ' + profile.name : ''}!</strong>
                            <br><br>
                            How can I help you today?
                            <br><br>
                            <span class="action-badge">💡 Tip: Try "run trend prophet" or ask me anything</span>
                        </div>
                    `;
                }
                // If not onboarded, keep the static welcome message asking for intro
            } catch (error) {
                console.error('Failed to load profile:', error);
            }
        }

        // Initialize
        loadProfile(); // Check if user already introduced
        loadStatus();
        loadSettings(); // Load bot name for header
        migrateLocalGoals(); // One-time migration of localStorage goals to server
        switchView(getInitialView());
        setInterval(healthCheck, 5000);
    </script>
</body>
</html>"##;
