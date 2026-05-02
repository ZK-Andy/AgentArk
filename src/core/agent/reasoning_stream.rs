//! Streaming bridge for the inbound classifier and advisory intent planner
//! LLM calls.
//!
//! These short pre-answer model calls used to run silently while the user
//! stared at "Waiting for first tokens..." in the right-hand pane. To keep
//! the UI alive without coupling the planner/classifier to chat-channel
//! plumbing, we forward their token-level deltas through the existing
//! `StreamEvent` channel as first-class `ReasoningDelta` events tagged with a
//! structural phase ("classifier" | "planner").
//!
//! The frontend distinguishes reasoning preview from the assistant content
//! stream by event kind, never by surface form.
//!
//! Two helpers:
//! - `stream_reasoning_progress`: emit a single delta directly. Used at the
//!   start/end boundaries of a non-streaming call.
//! - `spawn_reasoning_proxy`: returns a child `Sender<StreamEvent>` you can
//!   pass to a streaming LLM helper. Each `Token` arriving on the child
//!   channel is rewritten as a reasoning delta on the parent channel; other
//!   event kinds are dropped (the parent never wanted them — the tokens are
//!   internal).

use tokio::sync::mpsc::Sender;

use super::StreamEvent;

const REASONING_PROXY_BUFFER: usize = 256;

/// Emit a single structural reasoning delta on `token_tx`. `done=true`
/// indicates the phase has finished (used as the closing event for the
/// classifier and planner phases).
pub async fn stream_reasoning_progress(
    token_tx: &Sender<StreamEvent>,
    phase: &str,
    content_delta: &str,
    done: bool,
) {
    let _ = token_tx
        .send(StreamEvent::ReasoningDelta {
            phase: phase.to_string(),
            content_delta: content_delta.to_string(),
            done,
        })
        .await;
}

/// Spawn a forwarding task that converts raw `StreamEvent::Token` events on
/// the returned child channel into structured reasoning deltas on
/// `parent_tx`. The caller passes the child sender to a streaming LLM
/// helper; tokens are forwarded with `done=false`. When the child channel
/// closes (LLM call done), the proxy is dropped — emit a final `done=true`
/// from the call site once the response is parsed.
pub fn spawn_reasoning_proxy(
    parent_tx: Sender<StreamEvent>,
    phase: &'static str,
) -> Sender<StreamEvent> {
    let (child_tx, mut child_rx) =
        tokio::sync::mpsc::channel::<StreamEvent>(REASONING_PROXY_BUFFER);
    crate::spawn_logged!(
        "src/core/agent/reasoning_stream.rs:spawn_reasoning_proxy",
        async move {
            while let Some(event) = child_rx.recv().await {
                if let StreamEvent::Token(text) = event {
                    if !text.is_empty() {
                        stream_reasoning_progress(&parent_tx, phase, &text, false).await;
                    }
                }
            }
        }
    );
    child_tx
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn stream_reasoning_progress_emits_structured_event() {
        let (tx, mut rx) = tokio::sync::mpsc::channel::<StreamEvent>(8);
        stream_reasoning_progress(&tx, "planner", "thinking about goals", false).await;
        let event = rx.recv().await.expect("event delivered");
        match event {
            StreamEvent::ReasoningDelta {
                phase,
                content_delta,
                done,
            } => {
                assert_eq!(phase, "planner");
                assert_eq!(content_delta, "thinking about goals");
                assert!(!done);
            }
            other => panic!("expected ReasoningDelta, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn stream_reasoning_progress_marks_done_boundary() {
        let (tx, mut rx) = tokio::sync::mpsc::channel::<StreamEvent>(8);
        stream_reasoning_progress(&tx, "classifier", "", true).await;
        let event = rx.recv().await.expect("event delivered");
        let StreamEvent::ReasoningDelta { phase, done, .. } = event else {
            panic!("expected ReasoningDelta");
        };
        assert_eq!(phase, "classifier");
        assert!(done);
    }

    #[tokio::test]
    async fn proxy_forwards_token_deltas_with_phase_tag() {
        let (parent_tx, mut parent_rx) = tokio::sync::mpsc::channel::<StreamEvent>(16);
        let child_tx = spawn_reasoning_proxy(parent_tx, "planner");
        child_tx
            .send(StreamEvent::Token("hello ".to_string()))
            .await
            .unwrap();
        child_tx
            .send(StreamEvent::Token("world".to_string()))
            .await
            .unwrap();
        drop(child_tx);

        let mut received = Vec::new();
        while let Some(event) = parent_rx.recv().await {
            if let StreamEvent::ReasoningDelta {
                phase,
                content_delta,
                ..
            } = event
            {
                assert_eq!(phase, "planner");
                received.push(content_delta);
            }
            if received.len() == 2 {
                break;
            }
        }
        assert_eq!(received, vec!["hello ".to_string(), "world".to_string()]);
    }
}
