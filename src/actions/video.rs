//! Video generation using Remotion
//!
//! Creates videos from LLM-generated React component code.
//! The Remotion template (with pre-installed node_modules) lives at
//! /app/services/remotion-template in Docker. At render time we:
//!   1. Create a temp directory
//!   2. Symlink node_modules from the template
//!   3. Write the LLM-generated component as Main.tsx
//!   4. Run `npx remotion render` to produce an MP4
//!   5. Copy the output to data/outputs/{id}/video.mp4
//!   6. Delete the temp directory

use anyhow::{Context, Result};
use std::path::Path;
use uuid::Uuid;

const MIN_DURATION_SECONDS: u64 = 1;
const MAX_DURATION_SECONDS: u64 = 120;
const MIN_FPS: u64 = 12;
const MAX_FPS: u64 = 60;
const MIN_WIDTH: u64 = 256;
const MAX_WIDTH: u64 = 3840;
const MIN_HEIGHT: u64 = 256;
const MAX_HEIGHT: u64 = 2160;
const MAX_PIXELS: u64 = 8_294_400; // 4K UHD
const MAX_FRAMES: u64 = 7_200; // 2m @ 60fps

/// Locate the Remotion template directory.
/// In Docker: /app/services/remotion-template
/// In dev: {exe_dir}/services/remotion-template or ./services/remotion-template
fn find_template_dir() -> Result<std::path::PathBuf> {
    // Explicit override
    if let Ok(custom) = std::env::var("REMOTION_TEMPLATE_DIR") {
        let custom = std::path::PathBuf::from(custom.trim());
        if custom.join("package.json").exists() {
            return Ok(custom);
        }
    }

    // Docker path (most common)
    let docker_path = std::path::PathBuf::from("/app/services/remotion-template");
    if docker_path.join("package.json").exists() {
        return Ok(docker_path);
    }

    // Dev: relative to current exe
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            let dev_path = parent.join("services/remotion-template");
            if dev_path.join("package.json").exists() {
                return Ok(dev_path);
            }
        }
    }

    // Dev: relative to cwd
    let cwd_path = std::path::PathBuf::from("services/remotion-template");
    if cwd_path.join("package.json").exists() {
        return Ok(cwd_path);
    }

    anyhow::bail!("Remotion template not found. Ensure services/remotion-template exists with node_modules installed.")
}

fn clamp_or_error(value: u64, min: u64, max: u64, field: &str) -> Result<u64> {
    if value < min || value > max {
        anyhow::bail!(
            "Invalid '{}': {} (allowed: {}..={})",
            field,
            value,
            min,
            max
        );
    }
    Ok(value)
}

fn sanitize_output_filename(raw: Option<&str>) -> String {
    let fallback = "video.mp4".to_string();
    let src = raw.unwrap_or("video.mp4").trim();
    if src.is_empty() {
        return fallback;
    }

    let base = std::path::Path::new(src)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("video.mp4");

    let mut clean: String = base
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') {
                c
            } else {
                '_'
            }
        })
        .collect();

    while clean.contains("..") {
        clean = clean.replace("..", ".");
    }
    clean = clean.trim_matches('.').trim_matches('_').to_string();
    if clean.is_empty() {
        return fallback;
    }

    if clean.len() > 120 {
        clean.truncate(120);
    }

    let ext = clean
        .rsplit('.')
        .next()
        .map(|s| s.to_ascii_lowercase())
        .unwrap_or_default();
    if !matches!(ext.as_str(), "mp4" | "webm") {
        clean.push_str(".mp4");
    }
    clean
}

/// Generate a video from a React component using Remotion.
///
/// Arguments (JSON):
/// - `component`: React TSX code (required) — the video content
/// - `duration_seconds`: Video duration in seconds (default: 10)
/// - `width`: Video width in pixels (default: 1920)
/// - `height`: Video height in pixels (default: 1080)
/// - `fps`: Frames per second (default: 30)
/// - `filename`: Output filename (default: video.mp4)
///
/// Returns the download URL path for the rendered video.
pub async fn video_generate(
    _config_dir: &Path,
    data_dir: &Path,
    arguments: &serde_json::Value,
) -> Result<String> {
    let component = arguments["component"].as_str().ok_or_else(|| {
        anyhow::anyhow!("Missing 'component' — provide React TSX code for the video")
    })?;

    let raw_duration_seconds = arguments
        .get("duration_seconds")
        .and_then(|v| v.as_u64())
        .unwrap_or(10);
    let raw_width = arguments
        .get("width")
        .and_then(|v| v.as_u64())
        .unwrap_or(1920);
    let raw_height = arguments
        .get("height")
        .and_then(|v| v.as_u64())
        .unwrap_or(1080);
    let raw_fps = arguments.get("fps").and_then(|v| v.as_u64()).unwrap_or(30);
    let filename = sanitize_output_filename(arguments.get("filename").and_then(|v| v.as_str()));

    let duration_seconds = clamp_or_error(
        raw_duration_seconds,
        MIN_DURATION_SECONDS,
        MAX_DURATION_SECONDS,
        "duration_seconds",
    )?;
    let width = clamp_or_error(raw_width, MIN_WIDTH, MAX_WIDTH, "width")?;
    let height = clamp_or_error(raw_height, MIN_HEIGHT, MAX_HEIGHT, "height")?;
    let fps = clamp_or_error(raw_fps, MIN_FPS, MAX_FPS, "fps")?;

    if width.saturating_mul(height) > MAX_PIXELS {
        anyhow::bail!(
            "Invalid resolution: {}x{} exceeds maximum pixel budget ({})",
            width,
            height,
            MAX_PIXELS
        );
    }
    let duration_in_frames = duration_seconds * fps;
    if duration_in_frames > MAX_FRAMES {
        anyhow::bail!(
            "Invalid duration/fps: {} frames exceeds maximum ({})",
            duration_in_frames,
            MAX_FRAMES
        );
    }

    let template_dir = find_template_dir()?;
    tracing::info!(
        "Video generate: {}s @ {}x{} {}fps ({} frames)",
        duration_seconds,
        width,
        height,
        fps,
        duration_in_frames
    );

    // Create temp working directory
    let render_id = Uuid::new_v4().to_string();
    let temp_dir = std::env::temp_dir().join(format!("remotion-{}", render_id));
    tokio::fs::create_dir_all(&temp_dir).await?;

    // Ensure cleanup on all exit paths
    let cleanup_result = async {
        // Copy template config files to temp dir
        for file in &["package.json", "tsconfig.json", "remotion.config.ts"] {
            let src = template_dir.join(file);
            if src.exists() {
                tokio::fs::copy(&src, temp_dir.join(file)).await
                    .with_context(|| format!("Failed to copy {}", file))?;
            }
        }

        // Create src directory
        let src_dir = temp_dir.join("src");
        tokio::fs::create_dir_all(&src_dir).await?;

        // Symlink node_modules from template (avoids copying ~200MB)
        let nm_src = template_dir.join("node_modules");
        let nm_dst = temp_dir.join("node_modules");
        if nm_src.exists() {
            #[cfg(unix)]
            tokio::fs::symlink(&nm_src, &nm_dst).await
                .with_context(|| "Failed to symlink node_modules")?;
            #[cfg(windows)]
            std::os::windows::fs::symlink_dir(&nm_src, &nm_dst)
                .with_context(|| "Failed to symlink node_modules")?;
        } else {
            anyhow::bail!("Remotion node_modules not found. Run 'npm install' in the remotion-template directory.");
        }

        // Write the LLM-generated component as Main.tsx
        tokio::fs::write(src_dir.join("Main.tsx"), component).await?;

        // Write index.tsx with correct dimensions and duration
        let index_tsx = format!(
            r#"import {{ Composition }} from "remotion";
import {{ Main }} from "./Main";

export const RemotionRoot: React.FC = () => {{
  return (
    <Composition
      id="main"
      component={{Main}}
      width={{{width}}}
      height={{{height}}}
      fps={{{fps}}}
      durationInFrames={{{duration_in_frames}}}
      defaultProps={{{{}}}}
    />
  );
}};
"#,
            width = width,
            height = height,
            fps = fps,
            duration_in_frames = duration_in_frames,
        );
        tokio::fs::write(src_dir.join("index.tsx"), index_tsx).await?;

        // Output path — use the existing outputs system: data/outputs/{id}/{filename}
        let output_id = Uuid::new_v4().to_string();
        let output_dir = data_dir.join("outputs").join(&output_id);
        tokio::fs::create_dir_all(&output_dir).await?;
        let output_path = output_dir.join(&filename);

        // Run Remotion render
        let temp_dir_str = temp_dir.to_string_lossy().replace('\\', "/");
        let output_str = output_path.to_string_lossy().replace('\\', "/");

        tracing::info!("Remotion render starting in {}", temp_dir_str);

        let mut render_cmd = tokio::process::Command::new("npx");
        render_cmd
            .args(["remotion", "render", "src/index.tsx", "main", &output_str])
            .current_dir(&temp_dir)
            .env("PUPPETEER_SKIP_DOWNLOAD", "true")
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());

        // Keep GL backend configurable, with sane Linux default.
        if let Ok(gl) = std::env::var("REMOTION_GL") {
            let gl = gl.trim();
            if !gl.is_empty() {
                render_cmd.arg(format!("--gl={}", gl));
            }
        } else if cfg!(target_os = "linux") {
            render_cmd.arg("--gl=angle-egl");
        }

        let render_output = render_cmd.output();

        // Scale timeout with render workload.
        let timeout_secs = (duration_seconds.saturating_mul(20)).clamp(120, 1800);
        let output = tokio::time::timeout(
            std::time::Duration::from_secs(timeout_secs),
            render_output,
        )
        .await
        .map_err(|_| anyhow::anyhow!("Video render timed out after {} seconds", timeout_secs))?
        .with_context(|| "Failed to run Remotion render")?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        if !output.status.success() {
            tracing::error!("Remotion render failed:\nstdout: {}\nstderr: {}", stdout, stderr);
            anyhow::bail!("Video render failed: {}", stderr.chars().take(500).collect::<String>());
        }

        tracing::info!("Remotion render complete: {}", output_str);

        // Verify output exists
        if !output_path.exists() {
            anyhow::bail!("Render completed but output file not found at {}", output_str);
        }

        let file_size = tokio::fs::metadata(&output_path).await?.len();
        let encoded_name = urlencoding::encode(&filename).into_owned();
        let download_url = format!("/api/outputs/{}/{}", output_id, encoded_name);

        Ok::<String, anyhow::Error>(serde_json::json!({
            "status": "success",
            "url": download_url,
            "filename": filename,
            "output_id": output_id,
            "duration_seconds": duration_seconds,
            "resolution": format!("{}x{}", width, height),
            "fps": fps,
            "file_size_bytes": file_size,
        }).to_string())
    }.await;

    // Always clean up temp directory
    if let Err(e) = tokio::fs::remove_dir_all(&temp_dir).await {
        tracing::warn!("Failed to clean up temp dir {}: {}", temp_dir.display(), e);
    } else {
        tracing::info!("Cleaned up temp dir: {}", temp_dir.display());
    }

    cleanup_result
}
