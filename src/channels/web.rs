//! Web UI channel assets.
//!
//! The full application UI is served from frontend assets.
//! This module only keeps locked-mode HTML for master-password unlock.

/// Unlock page HTML - shown when master password is required
pub const UNLOCK_PAGE_HTML: &str = r##"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>AgentArk - Unlock</title>
    <style>
        * { margin: 0; padding: 0; box-sizing: border-box; }
        body {
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
            background: linear-gradient(135deg, #0a0a1a 0%, #1a1a2e 50%, #16213e 100%);
            color: #e0e0e0;
            min-height: 100vh;
            display: flex;
            align-items: center;
            justify-content: center;
        }
        .unlock-card {
            background: rgba(255,255,255,0.05);
            border: 1px solid rgba(255,255,255,0.1);
            border-radius: 16px;
            padding: 40px 36px;
            max-width: 400px;
            width: 90%;
            text-align: center;
            backdrop-filter: blur(20px);
        }
        .unlock-card img { width: 64px; height: 64px; margin-bottom: 16px; }
        .unlock-card h1 { font-size: 1.4em; margin-bottom: 8px; color: #fff; }
        .unlock-card p { font-size: 0.85em; color: #999; margin-bottom: 24px; }
        .unlock-card input {
            width: 100%;
            padding: 12px 16px;
            background: rgba(255,255,255,0.08);
            border: 1px solid rgba(255,255,255,0.15);
            border-radius: 8px;
            color: #fff;
            font-size: 0.95em;
            outline: none;
            margin-bottom: 16px;
        }
        .unlock-card input:focus { border-color: #6c5ce7; }
        .unlock-card button {
            width: 100%;
            padding: 12px;
            background: linear-gradient(135deg, #6c5ce7, #a855f7);
            border: none;
            border-radius: 8px;
            color: #fff;
            font-size: 0.95em;
            font-weight: 600;
            cursor: pointer;
        }
        .unlock-card button:hover { opacity: 0.9; }
        .unlock-card button:disabled { opacity: 0.5; cursor: wait; }
        .error { color: #ff6b6b; font-size: 0.82em; margin-top: 12px; }
        .success { color: #51cf66; font-size: 0.82em; margin-top: 12px; }
        .hint {
            font-size: 0.75em; color: #666; margin-top: 20px;
            border-top: 1px solid rgba(255,255,255,0.05); padding-top: 16px;
        }
    </style>
</head>
<body>
    <div class="unlock-card">
        <img src="/logo.svg" alt="AgentArk">
        <h1>AgentArk is Locked</h1>
        <p>Enter your master password to unlock the agent.</p>
        <form id="unlock-form">
            <input type="password" id="password" placeholder="Master password"
                   autofocus autocomplete="current-password">
            <button type="submit" id="unlock-btn">Unlock</button>
            <div id="msg" style="display:none"></div>
        </form>
        <div class="hint">
            Enter your master password to unlock AgentArk.
        </div>
    </div>
    <script>
        document.getElementById('unlock-form').onsubmit = async (e) => {
            e.preventDefault();
            const btn = document.getElementById('unlock-btn');
            const msg = document.getElementById('msg');
            const pw = document.getElementById('password').value;
            if (!pw) return;
            btn.disabled = true;
            btn.textContent = 'Unlocking...';
            msg.style.display = 'none';
            try {
                const res = await fetch('/unlock', {
                    method: 'POST',
                    headers: {'Content-Type': 'application/json'},
                    body: JSON.stringify({password: pw})
                });
                const data = await res.json();
                if (res.ok) {
                    msg.className = 'success';
                    msg.textContent = 'Unlocked! Starting up...';
                    msg.style.display = 'block';
                    setTimeout(() => location.reload(), 4000);
                } else {
                    msg.className = 'error';
                    msg.textContent = data.error || 'Invalid password';
                    msg.style.display = 'block';
                    btn.disabled = false;
                    btn.textContent = 'Unlock';
                    document.getElementById('password').select();
                }
            } catch(err) {
                msg.className = 'error';
                msg.textContent = 'Connection error';
                msg.style.display = 'block';
                btn.disabled = false;
                btn.textContent = 'Unlock';
            }
        };
    </script>
</body>
</html>
"##;
