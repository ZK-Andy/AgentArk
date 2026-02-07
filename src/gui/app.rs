//! Main GUI application

use eframe::egui;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::core::Agent;

/// Active view in the GUI
#[derive(Debug, Clone, PartialEq)]
pub enum ActiveView {
    Chat,
    Tasks,
    Memory,
    Actions,
    Safety,
    Proofs,
    Settings,
}

/// Main GUI application state
pub struct CrateAgentApp {
    /// Shared agent instance
    agent: Arc<RwLock<Agent>>,

    /// Tokio runtime for async operations
    runtime: tokio::runtime::Runtime,

    /// Current active view
    active_view: ActiveView,

    /// Chat input
    chat_input: String,

    /// Chat history
    chat_history: Vec<ChatMessage>,

    /// Status message
    status: String,

    /// Pending response receiver
    pending_response: Option<std::sync::mpsc::Receiver<Result<String, String>>>,
}

#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

impl CrateAgentApp {
    pub fn new(agent: Agent) -> Self {
        let runtime = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");

        Self {
            agent: Arc::new(RwLock::new(agent)),
            runtime,
            active_view: ActiveView::Chat,
            chat_input: String::new(),
            chat_history: Vec::new(),
            status: "Ready".to_string(),
            pending_response: None,
        }
    }

    fn render_sidebar(&mut self, ui: &mut egui::Ui) {
        ui.heading("Crate Agent");
        ui.separator();

        ui.vertical(|ui| {
            if ui
                .selectable_label(self.active_view == ActiveView::Chat, "Chat")
                .clicked()
            {
                self.active_view = ActiveView::Chat;
            }
            if ui
                .selectable_label(self.active_view == ActiveView::Tasks, "Tasks")
                .clicked()
            {
                self.active_view = ActiveView::Tasks;
            }
            if ui
                .selectable_label(self.active_view == ActiveView::Memory, "Memory")
                .clicked()
            {
                self.active_view = ActiveView::Memory;
            }
            if ui
                .selectable_label(self.active_view == ActiveView::Actions, "Actions")
                .clicked()
            {
                self.active_view = ActiveView::Actions;
            }
            if ui
                .selectable_label(self.active_view == ActiveView::Safety, "Safety")
                .clicked()
            {
                self.active_view = ActiveView::Safety;
            }
            if ui
                .selectable_label(self.active_view == ActiveView::Proofs, "Proofs")
                .clicked()
            {
                self.active_view = ActiveView::Proofs;
            }
            ui.separator();
            if ui
                .selectable_label(self.active_view == ActiveView::Settings, "Settings")
                .clicked()
            {
                self.active_view = ActiveView::Settings;
            }
        });

        ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
            ui.horizontal(|ui| {
                ui.label(&self.status);
            });
        });
    }

    fn render_chat(&mut self, ui: &mut egui::Ui) {
        ui.heading("Chat");
        ui.separator();

        // Check for pending response
        if let Some(rx) = &self.pending_response {
            if let Ok(result) = rx.try_recv() {
                match result {
                    Ok(response) => {
                        self.chat_history.push(ChatMessage {
                            role: "assistant".to_string(),
                            content: response,
                            timestamp: chrono::Utc::now(),
                        });
                    }
                    Err(e) => {
                        self.chat_history.push(ChatMessage {
                            role: "system".to_string(),
                            content: format!("Error: {}", e),
                            timestamp: chrono::Utc::now(),
                        });
                    }
                }
                self.pending_response = None;
                self.status = "Ready".to_string();
            }
        }

        // Chat history
        let available_height = ui.available_height() - 60.0;
        egui::ScrollArea::vertical()
            .max_height(available_height)
            .stick_to_bottom(true)
            .show(ui, |ui| {
                for msg in &self.chat_history {
                    let (bg_color, align) = if msg.role == "user" {
                        (egui::Color32::from_rgb(60, 60, 80), egui::Align::RIGHT)
                    } else {
                        (egui::Color32::from_rgb(40, 60, 40), egui::Align::LEFT)
                    };

                    ui.with_layout(egui::Layout::top_down(align), |ui| {
                        egui::Frame::none()
                            .fill(bg_color)
                            .rounding(8.0)
                            .inner_margin(10.0)
                            .show(ui, |ui| {
                                ui.label(&msg.content);
                                ui.small(msg.timestamp.format("%H:%M").to_string());
                            });
                    });
                    ui.add_space(5.0);
                }
            });

        // Input area
        ui.separator();
        ui.horizontal(|ui| {
            let input = egui::TextEdit::singleline(&mut self.chat_input)
                .hint_text("Type a message...")
                .desired_width(ui.available_width() - 80.0);

            let response = ui.add(input);

            let send_clicked = ui.button("Send").clicked();
            let enter_pressed =
                response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));

            if (send_clicked || enter_pressed)
                && !self.chat_input.is_empty()
                && self.pending_response.is_none()
            {
                let message = self.chat_input.clone();
                self.chat_input.clear();

                // Add user message to history
                self.chat_history.push(ChatMessage {
                    role: "user".to_string(),
                    content: message.clone(),
                    timestamp: chrono::Utc::now(),
                });

                self.status = "Processing...".to_string();

                // Process message synchronously using block_on
                // This is simpler and avoids Send issues
                let (tx, rx) = std::sync::mpsc::channel();
                self.pending_response = Some(rx);

                let agent = self.agent.clone();
                std::thread::spawn(move || {
                    let rt = tokio::runtime::Runtime::new().unwrap();
                    let result = rt.block_on(async {
                        let mut agent = agent.write().await;
                        agent.process_message(&message, "gui").await
                    });
                    let _ = tx.send(result.map_err(|e| e.to_string()));
                });
            }
        });
    }

    fn render_tasks(&mut self, ui: &mut egui::Ui) {
        ui.heading("Tasks");
        ui.separator();

        let tasks = self.runtime.block_on(async {
            let agent = self.agent.read().await;
            let tasks = agent.tasks.read().await;
            tasks.all().to_vec()
        });

        if tasks.is_empty() {
            ui.label("No tasks");
            return;
        }

        egui::ScrollArea::vertical().show(ui, |ui| {
            for task in tasks {
                egui::Frame::none()
                    .fill(egui::Color32::from_rgb(40, 40, 50))
                    .rounding(5.0)
                    .inner_margin(10.0)
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            let status_emoji = match &task.status {
                                crate::core::TaskStatus::Pending => "O",
                                crate::core::TaskStatus::AwaitingApproval => "!",
                                crate::core::TaskStatus::InProgress => "*",
                                crate::core::TaskStatus::Completed => "+",
                                crate::core::TaskStatus::Failed { .. } => "X",
                                crate::core::TaskStatus::Cancelled => "-",
                            };
                            ui.label(status_emoji);
                            ui.strong(&task.description);
                        });
                        ui.label(format!("Action: {}", task.action));
                    });
                ui.add_space(5.0);
            }
        });
    }

    fn render_memory(&mut self, ui: &mut egui::Ui) {
        ui.heading("Memory");
        ui.separator();

        let count = self.runtime.block_on(async {
            let agent = self.agent.read().await;
            agent.memory.entry_count()
        });

        ui.label(format!("Total memory entries: {}", count));

        ui.separator();
        ui.label("Memory types:");
        ui.horizontal(|ui| {
            ui.label("Episodic");
            ui.label("Semantic");
            ui.label("Procedural");
        });
    }

    fn render_actions(&mut self, ui: &mut egui::Ui) {
        ui.heading("Actions");
        ui.separator();

        let actions = self.runtime.block_on(async {
            let agent = self.agent.read().await;
            agent.runtime.list_actions().await.unwrap_or_default()
        });

        egui::ScrollArea::vertical().show(ui, |ui| {
            for action in actions {
                egui::Frame::none()
                    .fill(egui::Color32::from_rgb(40, 50, 40))
                    .rounding(5.0)
                    .inner_margin(10.0)
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.strong(&action.name);
                            ui.small(format!("v{}", action.version));
                        });
                        ui.label(&action.description);
                    });
                ui.add_space(5.0);
            }
        });
    }

    fn render_safety(&mut self, ui: &mut egui::Ui) {
        ui.heading("Safety Rules");
        ui.separator();

        let rules = self.runtime.block_on(async {
            let agent = self.agent.read().await;
            agent.safety.rules().to_vec()
        });

        egui::ScrollArea::vertical().show(ui, |ui| {
            for rule in rules {
                egui::Frame::none()
                    .fill(egui::Color32::from_rgb(50, 40, 40))
                    .rounding(5.0)
                    .inner_margin(10.0)
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            let verified = if rule.verified { "+" } else { "?" };
                            ui.label(verified);
                            ui.strong(&rule.name);
                        });
                        ui.label(&rule.description);
                    });
                ui.add_space(5.0);
            }
        });
    }

    fn render_proofs(&mut self, ui: &mut egui::Ui) {
        ui.heading("Execution Proofs");
        ui.separator();

        let (proof_count, receipts) = self.runtime.block_on(async {
            let agent = self.agent.read().await;
            let trace = agent.proofs.trace();
            let count = trace.proofs.len();
            let receipts: Vec<_> = trace
                .proofs
                .iter()
                .rev()
                .take(20)
                .map(crate::proofs::ProofReceipt::from)
                .collect();
            (count, receipts)
        });

        ui.label(format!("Total proofs: {}", proof_count));

        ui.separator();

        egui::ScrollArea::vertical().show(ui, |ui| {
            for receipt in receipts {
                egui::Frame::none()
                    .fill(egui::Color32::from_rgb(40, 40, 60))
                    .rounding(5.0)
                    .inner_margin(10.0)
                    .show(ui, |ui| {
                        ui.label(format!("ID: {}", receipt.proof_id));
                        ui.small(format!("Time: {}", receipt.timestamp));
                        ui.small(format!(
                            "Hash: {}...",
                            &receipt.proof_hash.chars().take(16).collect::<String>()
                        ));
                    });
                ui.add_space(5.0);
            }
        });
    }

    fn render_settings(&mut self, ui: &mut egui::Ui) {
        ui.heading("Settings");
        ui.separator();

        let (did, has_telegram) = self.runtime.block_on(async {
            let agent = self.agent.read().await;
            (
                agent.identity.did().to_string(),
                agent.config.telegram.is_some(),
            )
        });

        ui.group(|ui| {
            ui.label("Identity");
            ui.label(format!("DID: {}", did));
        });

        ui.group(|ui| {
            ui.label("LLM Provider");
            ui.label("Configure in config.toml");
        });

        ui.group(|ui| {
            ui.label("Telegram");
            if has_telegram {
                ui.label("Configured");
            } else {
                ui.label("Not configured");
            }
        });
    }
}

impl eframe::App for CrateAgentApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Sidebar
        egui::SidePanel::left("sidebar")
            .min_width(150.0)
            .max_width(200.0)
            .show(ctx, |ui| {
                self.render_sidebar(ui);
            });

        // Main content
        egui::CentralPanel::default().show(ctx, |ui| match self.active_view {
            ActiveView::Chat => self.render_chat(ui),
            ActiveView::Tasks => self.render_tasks(ui),
            ActiveView::Memory => self.render_memory(ui),
            ActiveView::Actions => self.render_actions(ui),
            ActiveView::Safety => self.render_safety(ui),
            ActiveView::Proofs => self.render_proofs(ui),
            ActiveView::Settings => self.render_settings(ui),
        });

        // Request repaint for animations
        if self.pending_response.is_some() {
            ctx.request_repaint();
        }
    }
}
