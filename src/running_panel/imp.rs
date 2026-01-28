use gtk4::prelude::*;
use gtk4::subclass::prelude::*;
use gtk4::{
    glib, gio, Box, Button, Entry, FileDialog, Label, Orientation, ScrolledWindow, Separator,
    Stack, StackSwitcher, TextView,
};
use std::cell::RefCell;
use std::fs;
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};

use serde::{Deserialize, Serialize};

/// Configuration struct for persistence
#[derive(Serialize, Deserialize, Default)]
struct RunningConfig {
    launcher_path: String,
    env_vars: String,
}

impl RunningConfig {
    fn config_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("vmod")
            .join("running_config.json")
    }

    fn load() -> Self {
        let path = Self::config_path();
        if let Ok(content) = std::fs::read_to_string(&path) {
            serde_json::from_str(&content).unwrap_or_default()
        } else {
            Self::default()
        }
    }

    fn save(&self) {
        let path = Self::config_path();
        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(content) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(&path, content);
        }
    }
}

pub struct RunningPanel {
    // State
    pub launcher_path: RefCell<Option<PathBuf>>,
    pub env_vars: RefCell<String>,
    pub child_process: RefCell<Option<Child>>,

    // UI widgets (stored for later access)
    pub exe_entry: RefCell<Option<Entry>>,
    pub env_entry: RefCell<Option<Entry>>,
    pub status_label: RefCell<Option<Label>>,
    pub run_button: RefCell<Option<Button>>,
    pub stop_button: RefCell<Option<Button>>,
    pub output_view: RefCell<Option<TextView>>,

    // Tabbed log viewing
    pub stack: RefCell<Option<Stack>>,
    pub player_log_view: RefCell<Option<TextView>>,
    pub vmod_log_view: RefCell<Option<TextView>>,

    // For output polling
    pub output_source_id: RefCell<Option<glib::SourceId>>,
}

impl Default for RunningPanel {
    fn default() -> Self {
        Self {
            launcher_path: RefCell::new(None),
            env_vars: RefCell::new("TERM=xterm".to_string()),
            child_process: RefCell::new(None),
            exe_entry: RefCell::new(None),
            env_entry: RefCell::new(None),
            status_label: RefCell::new(None),
            run_button: RefCell::new(None),
            stop_button: RefCell::new(None),
            output_view: RefCell::new(None),
            stack: RefCell::new(None),
            player_log_view: RefCell::new(None),
            vmod_log_view: RefCell::new(None),
            output_source_id: RefCell::new(None),
        }
    }
}

#[glib::object_subclass]
impl ObjectSubclass for RunningPanel {
    const NAME: &'static str = "RunningPanel";
    type Type = super::RunningPanel;
    type ParentType = Box;
}

impl ObjectImpl for RunningPanel {
    fn constructed(&self) {
        self.parent_constructed();

        let obj = self.obj();
        obj.set_orientation(Orientation::Vertical);
        obj.set_spacing(6);
        obj.set_margin_start(12);
        obj.set_margin_end(12);
        obj.set_margin_top(6);
        obj.set_margin_bottom(6);
        obj.set_width_request(300);

        // Load saved configuration
        let config = RunningConfig::load();

        // Header
        let header = Label::new(Some("Running DFU"));
        header.add_css_class("heading");
        header.set_halign(gtk4::Align::Start);
        obj.append(&header);

        // Executable path section
        let exe_label = Label::new(Some("Executable:"));
        exe_label.set_halign(gtk4::Align::Start);
        exe_label.add_css_class("dim-label");
        obj.append(&exe_label);

        let exe_box = Box::new(Orientation::Horizontal, 6);

        let exe_entry = Entry::new();
        exe_entry.set_hexpand(true);
        exe_entry.set_placeholder_text(Some("Path to DaggerfallUnity executable"));
        exe_entry.set_text(&config.launcher_path);
        exe_box.append(&exe_entry);

        let browse_button = Button::with_label("Browse...");
        exe_box.append(&browse_button);

        obj.append(&exe_box);

        // Environment variables section
        let env_label = Label::new(Some("Environment Variables:"));
        env_label.set_halign(gtk4::Align::Start);
        env_label.add_css_class("dim-label");
        env_label.set_margin_top(6);
        obj.append(&env_label);

        let env_entry = Entry::new();
        env_entry.set_placeholder_text(Some("TERM=xterm FOO=bar"));
        if config.env_vars.is_empty() {
            env_entry.set_text("TERM=xterm");
        } else {
            env_entry.set_text(&config.env_vars);
        }
        obj.append(&env_entry);

        // Status label
        let status_label = Label::new(Some("Status: Not Running"));
        status_label.set_halign(gtk4::Align::Start);
        status_label.set_margin_top(6);
        obj.append(&status_label);

        // Button box
        let button_box = Box::new(Orientation::Horizontal, 6);
        button_box.set_margin_top(6);

        let run_button = Button::with_label("Run");
        run_button.add_css_class("suggested-action");
        button_box.append(&run_button);

        let stop_button = Button::with_label("Stop");
        stop_button.add_css_class("destructive-action");
        stop_button.set_visible(false);
        button_box.append(&stop_button);

        obj.append(&button_box);

        // Separator
        let separator = Separator::new(Orientation::Horizontal);
        separator.set_margin_top(6);
        separator.set_margin_bottom(6);
        obj.append(&separator);

        // Tab header with refresh button
        let tab_header = Box::new(Orientation::Horizontal, 6);

        let stack_switcher = StackSwitcher::new();
        stack_switcher.set_halign(gtk4::Align::Start);
        stack_switcher.set_hexpand(true);
        tab_header.append(&stack_switcher);

        let refresh_button = Button::from_icon_name("view-refresh-symbolic");
        refresh_button.set_tooltip_text(Some("Refresh log"));
        tab_header.append(&refresh_button);

        obj.append(&tab_header);

        let stack = Stack::new();
        stack.set_vexpand(true);
        stack.set_transition_type(gtk4::StackTransitionType::Crossfade);
        stack.set_transition_duration(150);

        // Output tab (process stdout/stderr)
        let output_scroll = ScrolledWindow::new();
        output_scroll.set_vexpand(true);
        output_scroll.set_min_content_height(200);

        let output_view = TextView::new();
        output_view.set_editable(false);
        output_view.set_cursor_visible(false);
        output_view.set_monospace(true);
        output_view.set_wrap_mode(gtk4::WrapMode::WordChar);
        output_scroll.set_child(Some(&output_view));

        stack.add_titled(&output_scroll, Some("output"), "Output");

        // Player.log tab (Unity game log)
        let player_log_scroll = ScrolledWindow::new();
        player_log_scroll.set_vexpand(true);
        player_log_scroll.set_min_content_height(200);

        let player_log_view = TextView::new();
        player_log_view.set_editable(false);
        player_log_view.set_cursor_visible(false);
        player_log_view.set_monospace(true);
        player_log_view.set_wrap_mode(gtk4::WrapMode::WordChar);
        player_log_scroll.set_child(Some(&player_log_view));

        stack.add_titled(&player_log_scroll, Some("player_log"), "Player.log");

        // vmod.log tab (application log)
        let vmod_log_scroll = ScrolledWindow::new();
        vmod_log_scroll.set_vexpand(true);
        vmod_log_scroll.set_min_content_height(200);

        let vmod_log_view = TextView::new();
        vmod_log_view.set_editable(false);
        vmod_log_view.set_cursor_visible(false);
        vmod_log_view.set_monospace(true);
        vmod_log_view.set_wrap_mode(gtk4::WrapMode::WordChar);
        vmod_log_scroll.set_child(Some(&vmod_log_view));

        stack.add_titled(&vmod_log_scroll, Some("vmod_log"), "vmod.log");

        // Link stack switcher to stack
        stack_switcher.set_stack(Some(&stack));

        obj.append(&stack);

        // Store widget references
        self.exe_entry.replace(Some(exe_entry.clone()));
        self.env_entry.replace(Some(env_entry.clone()));
        self.status_label.replace(Some(status_label.clone()));
        self.run_button.replace(Some(run_button.clone()));
        self.stop_button.replace(Some(stop_button.clone()));
        self.output_view.replace(Some(output_view.clone()));
        self.stack.replace(Some(stack.clone()));
        self.player_log_view.replace(Some(player_log_view));
        self.vmod_log_view.replace(Some(vmod_log_view));

        // Initialize launcher_path from config if present
        if !config.launcher_path.is_empty() {
            self.launcher_path.replace(Some(PathBuf::from(&config.launcher_path)));
        }
        self.env_vars.replace(if config.env_vars.is_empty() {
            "TERM=xterm".to_string()
        } else {
            config.env_vars
        });

        // Connect browse button
        self.connect_browse_button(&browse_button, &exe_entry);

        // Connect exe_entry change for auto-save
        self.connect_exe_entry_changed(&exe_entry, &env_entry);

        // Connect env_entry change for auto-save
        self.connect_env_entry_changed(&exe_entry, &env_entry);

        // Connect run button
        self.connect_run_button(&run_button, &stop_button, &exe_entry, &env_entry, &status_label, &output_view);

        // Connect stop button
        self.connect_stop_button(&run_button, &stop_button, &status_label);

        // Connect refresh button
        self.connect_refresh_button(&refresh_button, &stack);

        // Load initial log content
        self.refresh_log_tab("player_log");
        self.refresh_log_tab("vmod_log");
    }
}

impl WidgetImpl for RunningPanel {}
impl BoxImpl for RunningPanel {}

impl RunningPanel {
    /// Set launcher path programmatically
    pub fn set_launcher(&self, path: PathBuf) {
        if let Some(entry) = self.exe_entry.borrow().as_ref() {
            entry.set_text(&path.display().to_string());
        }
        self.launcher_path.replace(Some(path));
    }

    /// Connect browse button to FileDialog
    fn connect_browse_button(&self, browse_button: &Button, exe_entry: &Entry) {
        let obj = self.obj();
        let exe_entry_clone = exe_entry.clone();
        let obj_weak = obj.downgrade();

        browse_button.connect_clicked(move |btn| {
            let obj = match obj_weak.upgrade() {
                Some(o) => o,
                None => return,
            };

            let file_dialog = FileDialog::new();
            file_dialog.set_title("Select DFU Executable");

            // Get parent window
            let root = btn.root();
            let window = root.and_then(|r| r.downcast::<gtk4::Window>().ok());

            let exe_entry_clone2 = exe_entry_clone.clone();
            let obj_clone = obj.clone();

            file_dialog.open(
                window.as_ref(),
                None::<&gio::Cancellable>,
                move |result| {
                    if let Ok(file) = result {
                        if let Some(path) = file.path() {
                            exe_entry_clone2.set_text(&path.display().to_string());
                            obj_clone.imp().launcher_path.replace(Some(path));
                            // Save config is triggered by exe_entry changed signal
                        }
                    }
                },
            );
        });
    }

    /// Connect exe_entry changed signal for auto-save
    fn connect_exe_entry_changed(&self, exe_entry: &Entry, env_entry: &Entry) {
        let obj = self.obj();
        let env_entry_clone = env_entry.clone();
        let obj_weak = obj.downgrade();

        exe_entry.connect_changed(move |entry| {
            let obj = match obj_weak.upgrade() {
                Some(o) => o,
                None => return,
            };

            let path_text = entry.text().to_string();
            if path_text.is_empty() {
                obj.imp().launcher_path.replace(None);
            } else {
                obj.imp().launcher_path.replace(Some(PathBuf::from(&path_text)));
            }

            // Save config
            let config = RunningConfig {
                launcher_path: path_text,
                env_vars: env_entry_clone.text().to_string(),
            };
            config.save();
        });
    }

    /// Connect env_entry changed signal for auto-save
    fn connect_env_entry_changed(&self, exe_entry: &Entry, env_entry: &Entry) {
        let obj = self.obj();
        let exe_entry_clone = exe_entry.clone();
        let obj_weak = obj.downgrade();

        env_entry.connect_changed(move |entry| {
            let obj = match obj_weak.upgrade() {
                Some(o) => o,
                None => return,
            };

            let env_text = entry.text().to_string();
            obj.imp().env_vars.replace(env_text.clone());

            // Save config
            let config = RunningConfig {
                launcher_path: exe_entry_clone.text().to_string(),
                env_vars: env_text,
            };
            config.save();
        });
    }

    /// Parse environment variables from string
    fn parse_env_vars(input: &str) -> Vec<(String, String)> {
        input
            .split_whitespace()
            .filter_map(|pair| {
                let mut parts = pair.splitn(2, '=');
                match (parts.next(), parts.next()) {
                    (Some(key), Some(value)) => Some((key.to_string(), value.to_string())),
                    _ => None,
                }
            })
            .collect()
    }

    /// Connect run button click handler
    fn connect_run_button(
        &self,
        run_button: &Button,
        stop_button: &Button,
        exe_entry: &Entry,
        env_entry: &Entry,
        status_label: &Label,
        output_view: &TextView,
    ) {
        let obj = self.obj();
        let obj_weak = obj.downgrade();
        let stop_button_clone = stop_button.clone();
        let exe_entry_clone = exe_entry.clone();
        let env_entry_clone = env_entry.clone();
        let status_label_clone = status_label.clone();
        let output_view_clone = output_view.clone();
        let run_button_clone = run_button.clone();

        run_button.connect_clicked(move |_| {
            let obj = match obj_weak.upgrade() {
                Some(o) => o,
                None => return,
            };

            let launcher_path = exe_entry_clone.text().to_string();
            if launcher_path.is_empty() {
                status_label_clone.set_text("Status: No executable selected");
                return;
            }

            let launcher_path = PathBuf::from(&launcher_path);
            if !launcher_path.exists() {
                status_label_clone.set_text("Status: Executable not found");
                return;
            }

            // Clear output
            let buffer = output_view_clone.buffer();
            buffer.set_text("");

            // Parse env vars
            let env_vars = Self::parse_env_vars(&env_entry_clone.text());

            // Build command
            let mut cmd = Command::new(&launcher_path);

            // Set working directory to executable's parent
            if let Some(parent) = launcher_path.parent() {
                cmd.current_dir(parent);
            }

            cmd.stdout(Stdio::piped());
            cmd.stderr(Stdio::piped());

            // Apply environment variables
            for (key, value) in env_vars {
                cmd.env(key, value);
            }

            // Spawn process
            match cmd.spawn() {
                Ok(mut child) => {
                    let pid = child.id();
                    status_label_clone.set_text(&format!("Status: Running (PID: {})", pid));

                    // Update button visibility
                    run_button_clone.set_visible(false);
                    stop_button_clone.set_visible(true);

                    // Take stdout and stderr for reading
                    let stdout = child.stdout.take();
                    let stderr = child.stderr.take();

                    // Store child process
                    obj.imp().child_process.replace(Some(child));

                    // Create channel for thread communication
                    let (sender, receiver) = mpsc::channel::<String>();
                    let receiver = Rc::new(RefCell::new(receiver));

                    // Flag to signal threads to stop
                    let running = Arc::new(AtomicBool::new(true));

                    // Spawn thread for stdout reading
                    if let Some(stdout) = stdout {
                        let sender_clone = sender.clone();
                        let running_clone = running.clone();
                        std::thread::spawn(move || {
                            let reader = BufReader::new(stdout);
                            for line in reader.lines() {
                                if !running_clone.load(Ordering::Relaxed) {
                                    break;
                                }
                                if let Ok(line) = line {
                                    if sender_clone.send(format!("{}\n", line)).is_err() {
                                        break;
                                    }
                                }
                            }
                        });
                    }

                    // Spawn thread for stderr reading
                    if let Some(stderr) = stderr {
                        let running_clone = running.clone();
                        std::thread::spawn(move || {
                            let reader = BufReader::new(stderr);
                            for line in reader.lines() {
                                if !running_clone.load(Ordering::Relaxed) {
                                    break;
                                }
                                if let Ok(line) = line {
                                    if sender.send(format!("[stderr] {}\n", line)).is_err() {
                                        break;
                                    }
                                }
                            }
                        });
                    }

                    // Set up polling to check output and process status
                    let output_view_for_poll = output_view_clone.clone();
                    let status_label_for_poll = status_label_clone.clone();
                    let run_button_for_poll = run_button_clone.clone();
                    let stop_button_for_poll = stop_button_clone.clone();
                    let obj_weak_for_poll = obj.downgrade();

                    let source_id = glib::timeout_add_local(
                        std::time::Duration::from_millis(50),
                        move || {
                            let obj = match obj_weak_for_poll.upgrade() {
                                Some(o) => o,
                                None => {
                                    running.store(false, Ordering::Relaxed);
                                    return glib::ControlFlow::Break;
                                }
                            };

                            // Non-blocking receive of output from threads
                            let buffer = output_view_for_poll.buffer();
                            let receiver_ref = receiver.borrow();
                            let mut got_output = false;
                            while let Ok(text) = receiver_ref.try_recv() {
                                let mut end = buffer.end_iter();
                                buffer.insert(&mut end, &text);
                                got_output = true;
                            }

                            // Auto-scroll to bottom if we got output
                            if got_output {
                                let end = buffer.end_iter();
                                let mark = buffer.create_mark(None, &end, false);
                                output_view_for_poll.scroll_to_mark(&mark, 0.0, false, 0.0, 0.0);
                            }

                            // Check if process has exited
                            let mut child_ref = obj.imp().child_process.borrow_mut();
                            if let Some(ref mut child) = *child_ref {
                                match child.try_wait() {
                                    Ok(Some(status)) => {
                                        // Process exited - drain remaining output
                                        drop(receiver_ref);
                                        let receiver_ref = receiver.borrow();
                                        while let Ok(text) = receiver_ref.try_recv() {
                                            let mut end = buffer.end_iter();
                                            buffer.insert(&mut end, &text);
                                        }

                                        running.store(false, Ordering::Relaxed);
                                        status_label_for_poll.set_text(&format!(
                                            "Status: Exited ({})",
                                            status
                                        ));
                                        run_button_for_poll.set_visible(true);
                                        stop_button_for_poll.set_visible(false);
                                        drop(child_ref);
                                        obj.imp().child_process.replace(None);
                                        obj.imp().output_source_id.replace(None);
                                        return glib::ControlFlow::Break;
                                    }
                                    Ok(None) => {
                                        // Still running
                                    }
                                    Err(e) => {
                                        status_label_for_poll
                                            .set_text(&format!("Status: Error checking process: {}", e));
                                    }
                                }
                            } else {
                                // No child process
                                running.store(false, Ordering::Relaxed);
                                return glib::ControlFlow::Break;
                            }

                            glib::ControlFlow::Continue
                        },
                    );

                    obj.imp().output_source_id.replace(Some(source_id));
                }
                Err(e) => {
                    status_label_clone.set_text(&format!("Status: Failed to start: {}", e));
                }
            }
        });
    }

    /// Connect stop button click handler
    fn connect_stop_button(
        &self,
        run_button: &Button,
        stop_button: &Button,
        status_label: &Label,
    ) {
        let obj = self.obj();
        let obj_weak = obj.downgrade();
        let run_button_clone = run_button.clone();
        let stop_button_clone = stop_button.clone();
        let status_label_clone = status_label.clone();

        stop_button.connect_clicked(move |_| {
            let obj = match obj_weak.upgrade() {
                Some(o) => o,
                None => return,
            };

            // Cancel output polling
            if let Some(source_id) = obj.imp().output_source_id.borrow_mut().take() {
                source_id.remove();
            }

            // Take the child process (drop borrow before using obj again)
            let child_opt = obj.imp().child_process.borrow_mut().take();

            // Kill the process
            if let Some(mut child) = child_opt {
                // Use kill() which sends SIGKILL on Unix
                let _ = child.kill();
                let _ = child.wait();

                status_label_clone.set_text("Status: Stopped");
                run_button_clone.set_visible(true);
                stop_button_clone.set_visible(false);
            }
        });
    }

    /// Connect refresh button click handler
    fn connect_refresh_button(&self, refresh_button: &Button, stack: &Stack) {
        let obj = self.obj();
        let obj_weak = obj.downgrade();
        let stack_clone = stack.clone();

        refresh_button.connect_clicked(move |_| {
            let obj = match obj_weak.upgrade() {
                Some(o) => o,
                None => return,
            };

            if let Some(visible_name) = stack_clone.visible_child_name() {
                obj.imp().refresh_log_tab(&visible_name);
            }
        });
    }

    /// Refresh the content of a log tab
    fn refresh_log_tab(&self, tab_name: &str) {
        const MAX_BYTES: u64 = 100 * 1024; // Last 100KB

        match tab_name {
            "player_log" => {
                let path = dirs::config_dir()
                    .unwrap_or_else(|| PathBuf::from("."))
                    .join("unity3d")
                    .join("Daggerfall Workshop")
                    .join("Daggerfall Unity")
                    .join("Player.log");

                if let Some(view) = self.player_log_view.borrow().as_ref() {
                    let content = Self::read_log_tail(&path, MAX_BYTES);
                    view.buffer().set_text(&content);
                    Self::scroll_to_bottom(view);
                }
            }
            "vmod_log" => {
                let path = dirs::config_dir()
                    .unwrap_or_else(|| PathBuf::from("."))
                    .join("vmod")
                    .join("vmod.log");

                if let Some(view) = self.vmod_log_view.borrow().as_ref() {
                    let content = Self::read_log_tail(&path, MAX_BYTES);
                    view.buffer().set_text(&content);
                    Self::scroll_to_bottom(view);
                }
            }
            _ => {}
        }
    }

    /// Read the last N bytes of a log file
    fn read_log_tail(path: &PathBuf, max_bytes: u64) -> String {
        if !path.exists() {
            return format!("[File not found: {}]", path.display());
        }

        let mut file = match fs::File::open(path) {
            Ok(f) => f,
            Err(e) => return format!("[Error opening file: {}]", e),
        };

        let metadata = match file.metadata() {
            Ok(m) => m,
            Err(e) => return format!("[Error reading file: {}]", e),
        };

        let file_size = metadata.len();
        let start_pos = if file_size > max_bytes {
            file_size - max_bytes
        } else {
            0
        };

        if let Err(e) = file.seek(SeekFrom::Start(start_pos)) {
            return format!("[Error seeking file: {}]", e);
        }

        let mut content = String::new();
        if let Err(e) = file.read_to_string(&mut content) {
            return format!("[Error reading file: {}]", e);
        }

        // If we started mid-file, skip to first newline
        if start_pos > 0 {
            if let Some(newline_pos) = content.find('\n') {
                content = content[newline_pos + 1..].to_string();
            }
        }

        content
    }

    /// Scroll a TextView to the bottom
    fn scroll_to_bottom(view: &TextView) {
        let buffer = view.buffer();
        let end = buffer.end_iter();
        let mark = buffer.create_mark(None, &end, false);
        view.scroll_to_mark(&mark, 0.0, false, 0.0, 0.0);
    }
}
