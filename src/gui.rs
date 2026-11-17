//! Main dialog (native Win32 controls through `native-windows-gui`).

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use native_windows_gui as nwg;
use nwg::{Event, EventData, MousePressEvent};

use crate::copier::{self, Job, Progress, Summary};
use crate::format;
use crate::i18n::{Lang, Translator};

/// `WS_EX_ACCEPTFILES`: lets a control receive files dropped from Explorer.
const WS_EX_ACCEPTFILES: u32 = 0x0000_0010;
/// Resource id of the icon embedded by `build.rs`.
const ICON_RESOURCE_ID: usize = 1;
const PROGRESS_STEPS: u32 = 1000;

/// Messages sent by the copy thread to the UI thread.
enum Update {
    Progress(Progress),
    Done(usize),
    Failed(String),
    Finished(Result<Summary, String>),
}

struct Shared {
    queue: Mutex<Vec<Update>>,
    cancel: AtomicBool,
}

struct Running {
    shared: Arc<Shared>,
    files: Vec<PathBuf>,
    thread: Option<JoinHandle<()>>,
}

/// What the status bar shows. Kept as data (not text) so it can be
/// re-rendered when the interface language changes.
#[derive(Clone, Copy, Default)]
enum StatusView {
    #[default]
    Ready,
    Starting,
    Progress(Progress),
    Done { copied: usize, elapsed: Duration, rate: f64 },
    Partial { copied: usize, failed: usize },
    Cancelled,
}

/// Width/height of the language selector, and room reserved for it in the status bar.
const LANG_COMBO_WIDTH: i32 = 120;
const STATUS_BAR_HEIGHT: u32 = 26;

#[derive(Default)]
struct App {
    window: nwg::Window,
    icon: nwg::Icon,
    title_font: nwg::Font,
    icon_frame: nwg::ImageFrame,
    title: nwg::Label,
    drop_label: nwg::Label,
    list: nwg::ListBox<String>,
    dest1_label: nwg::Label,
    dest1: nwg::TextInput,
    dest1_btn: nwg::Button,
    dest2_label: nwg::Label,
    dest2: nwg::TextInput,
    dest2_btn: nwg::Button,
    progress: nwg::ProgressBar,
    go: nwg::Button,
    status: nwg::StatusBar,
    lang_combo: nwg::ComboBox<String>,
    notice: nwg::Notice,
    files_dialog: nwg::FileDialog,
    folder_dialog: nwg::FileDialog,

    /// Current translator; changed at run time by the language selector
    /// (the choice is deliberately never persisted).
    tr: RefCell<Option<Translator>>,
    status_view: RefCell<StatusView>,
    /// Files not processed yet; mirrors the content of `list`.
    pending: RefCell<Vec<PathBuf>>,
    running: RefCell<Option<Running>>,
    failures: RefCell<Vec<String>>,
}

/// Opens the main window and runs the message loop until it is closed.
pub fn run(tr: Translator, dest1: Option<PathBuf>, dest2: Option<PathBuf>) {
    nwg::init().expect("failed to initialise the Windows GUI");
    let mut font = nwg::Font::default();
    if nwg::Font::builder().family("Segoe UI").size(16).build(&mut font).is_ok() {
        nwg::Font::set_global_default(Some(font));
    }

    let app = Rc::new(App::build(tr, dest1, dest2));

    let handler_app = Rc::clone(&app);
    let handler = nwg::full_bind_event_handler(&app.window.handle, move |evt, data, handle| {
        handler_app.on_event(evt, data, handle);
    });

    nwg::dispatch_thread_events();
    nwg::unbind_event_handler(&handler);
}

impl App {
    fn t(&self, key: &str) -> String {
        self.t_f(key, &[])
    }

    fn build(tr: Translator, dest1: Option<PathBuf>, dest2: Option<PathBuf>) -> App {
        let mut app = App::default();

        let embed = nwg::EmbedResource::load(None).ok();
        let _ = nwg::Icon::builder()
            .source_embed(embed.as_ref())
            .source_embed_id(ICON_RESOURCE_ID)
            .size(Some((48, 48)))
            .build(&mut app.icon);
        let _ = nwg::Font::builder().family("Segoe UI").size(34).weight(700).build(&mut app.title_font);

        nwg::Window::builder()
            .flags(nwg::WindowFlags::WINDOW | nwg::WindowFlags::VISIBLE)
            .size((574, 430))
            .center(true)
            .title(tr.t("app.title"))
            .icon(Some(&app.icon))
            .build(&mut app.window)
            .expect("window");
        let w = &app.window;

        nwg::ImageFrame::builder()
            .parent(w)
            .position((16, 10))
            .size((48, 48))
            .icon(Some(&app.icon))
            .build(&mut app.icon_frame)
            .expect("icon frame");
        nwg::Label::builder()
            .parent(w)
            .text(tr.t("app.title"))
            .font(Some(&app.title_font))
            .position((74, 14))
            .size((300, 44))
            .build(&mut app.title)
            .expect("title");

        nwg::Label::builder()
            .parent(w)
            .text(tr.t("drop.enabled"))
            .position((16, 68))
            .size((542, 20))
            .build(&mut app.drop_label)
            .expect("drop label");
        nwg::ListBox::<String>::builder()
            .parent(w)
            .flags(nwg::ListBoxFlags::VISIBLE | nwg::ListBoxFlags::NO_SELECT)
            .ex_flags(WS_EX_ACCEPTFILES)
            .position((16, 90))
            .size((542, 150))
            .build(&mut app.list)
            .expect("drop zone");

        let rows = [
            ("dest1.label", dest1.as_ref(), 250, 0),
            ("dest2.label", dest2.as_ref(), 304, 1),
        ];
        for (key, initial, y, which) in rows {
            let (label, input, button) = if which == 0 {
                (&mut app.dest1_label, &mut app.dest1, &mut app.dest1_btn)
            } else {
                (&mut app.dest2_label, &mut app.dest2, &mut app.dest2_btn)
            };
            nwg::Label::builder().parent(w).text(tr.t(key)).position((16, y)).size((542, 20)).build(label).expect("label");
            nwg::TextInput::builder()
                .parent(w)
                .text(&initial.map(|p| p.display().to_string()).unwrap_or_default())
                .position((16, y + 20))
                .size((492, 25))
                .build(input)
                .expect("input");
            nwg::Button::builder()
                .parent(w)
                .text(tr.t("browse"))
                .position((516, y + 19))
                .size((42, 27))
                .build(button)
                .expect("browse button");
        }

        nwg::ProgressBar::builder()
            .parent(w)
            .range(0..PROGRESS_STEPS)
            .position((16, 368))
            .size((420, 22))
            .build(&mut app.progress)
            .expect("progress");
        nwg::Button::builder()
            .parent(w)
            .text(tr.t("go"))
            .position((446, 363))
            .size((112, 32))
            .build(&mut app.go)
            .expect("go button");
        nwg::StatusBar::builder().parent(w).text(tr.t("status.ready")).build(&mut app.status).expect("status bar");
        app.status.set_min_height(STATUS_BAR_HEIGHT);
        nwg::Notice::builder().parent(w).build(&mut app.notice).expect("notice");

        // Language selector, laid over the right end of the status bar
        // (created after it so that it is drawn on top).
        let (client_w, client_h) = w
            .handle
            .hwnd()
            .and_then(|h| crate::platform::client_size(h.cast()))
            .unwrap_or((574, 431));
        let current = Lang::ALL.iter().position(|&l| l == tr.lang());
        nwg::ComboBox::<String>::builder()
            .parent(w)
            .collection(Lang::ALL.iter().map(|l| l.native_name().to_owned()).collect())
            .selected_index(current)
            .position((client_w - LANG_COMBO_WIDTH - 4, client_h - STATUS_BAR_HEIGHT as i32 + 1))
            .size((LANG_COMBO_WIDTH, 160))
            .build(&mut app.lang_combo)
            .expect("language selector");
        if let Some(h) = app.lang_combo.handle.hwnd() {
            crate::platform::raise_child(h.cast());
        }

        nwg::FileDialog::builder()
            .title(tr.t("dialog.files"))
            .action(nwg::FileDialogAction::Open)
            .multiselect(true)
            .build(&mut app.files_dialog)
            .expect("file dialog");
        nwg::FileDialog::builder()
            .title(tr.t("dialog.folder"))
            .action(nwg::FileDialogAction::OpenDirectory)
            .build(&mut app.folder_dialog)
            .expect("folder dialog");

        app.tr = RefCell::new(Some(tr));
        app.refresh_controls();
        app
    }

    /// Re-applies every translatable text with the current translator.
    fn apply_language(&self) {
        self.window.set_text(&self.t("app.title"));
        self.dest1_label.set_text(&self.t("dest1.label"));
        self.dest2_label.set_text(&self.t("dest2.label"));
        self.go.set_text(&self.t("go"));
        self.drop_label.set_text(&self.t("drop.enabled"));
        self.files_dialog.set_title(&self.t("dialog.files"));
        self.folder_dialog.set_title(&self.t("dialog.folder"));
        self.render_status();
    }

    fn on_language_selected(&self) {
        let Some(lang) = self.lang_combo.selection().and_then(|i| Lang::ALL.get(i).copied()) else {
            return;
        };
        if self.tr.borrow().as_ref().is_some_and(|t| t.lang() == lang) {
            return;
        }
        *self.tr.borrow_mut() = Some(Translator::new(lang));
        self.apply_language();
    }

    fn set_status(&self, view: StatusView) {
        *self.status_view.borrow_mut() = view;
        self.render_status();
    }

    fn render_status(&self) {
        let text = match *self.status_view.borrow() {
            StatusView::Ready => self.t("status.ready"),
            StatusView::Starting => "...".to_owned(),
            StatusView::Progress(p) => {
                let eta = p.eta().map(format::duration).unwrap_or_else(|| "--".into());
                self.t_f("status.progress", &[&eta, &self.rate_text(p.rate())])
            }
            StatusView::Done { copied, elapsed, rate } => self.t_f(
                "status.done",
                &[&copied.to_string(), &format::duration(elapsed), &self.rate_text(rate)],
            ),
            StatusView::Partial { copied, failed } => {
                self.t_f("status.partial", &[&copied.to_string(), &failed.to_string()])
            }
            StatusView::Cancelled => self.t("status.cancelled"),
        };
        self.status.set_text(0, &text);
    }

    fn on_event(&self, evt: Event, data: EventData, handle: nwg::ControlHandle) {
        match evt {
            Event::OnWindowClose if handle == self.window.handle => self.on_close(),
            Event::OnButtonClick if handle == self.go.handle => self.start(),
            Event::OnButtonClick if handle == self.dest1_btn.handle => self.browse_folder(&self.dest1),
            Event::OnButtonClick if handle == self.dest2_btn.handle => self.browse_folder(&self.dest2),
            Event::OnTextInput => self.refresh_controls(),
            Event::OnComboxBoxSelection if handle == self.lang_combo.handle => self.on_language_selected(),
            Event::OnNotice => self.on_notice(),
            Event::OnMousePress(MousePressEvent::MousePressLeftUp) if handle == self.list.handle => {
                if self.list.enabled() {
                    self.browse_files();
                }
            }
            Event::OnFileDrop if handle == self.list.handle => {
                let dropped = data.on_file_drop().files().into_iter().map(PathBuf::from);
                self.add_files(dropped);
            }
            _ => {}
        }
    }

    // ---- state -----------------------------------------------------------

    fn destinations_ready(&self) -> bool {
        let (a, b) = (self.dest1.text(), self.dest2.text());
        let (a, b) = (a.trim(), b.trim());
        !a.is_empty() && !b.is_empty() && !crate::same_folder(a.as_ref(), b.as_ref())
    }

    /// Enables/disables controls according to the current state.
    fn refresh_controls(&self) {
        let running = self.running.borrow().is_some();
        let ready = self.destinations_ready();

        // Everything is usable from the start; only "Go" waits for both destinations.
        self.list.set_enabled(!running);
        self.go.set_enabled(ready && !running);
        for edit in [&self.dest1, &self.dest2] {
            edit.set_enabled(!running);
        }
        for button in [&self.dest1_btn, &self.dest2_btn] {
            button.set_enabled(!running);
        }
    }

    fn add_files(&self, paths: impl IntoIterator<Item = PathBuf>) {
        if !self.list.enabled() {
            return;
        }
        {
            let mut pending = self.pending.borrow_mut();
            for path in paths {
                if path.is_file() && !pending.contains(&path) {
                    self.list.push(path.display().to_string());
                    pending.push(path);
                }
            }
        }
        self.refresh_controls();
    }

    fn browse_files(&self) {
        if self.files_dialog.run(Some(&self.window)) {
            if let Ok(items) = self.files_dialog.get_selected_items() {
                self.add_files(items.into_iter().map(PathBuf::from));
            }
        }
    }

    fn browse_folder(&self, target: &nwg::TextInput) {
        if self.folder_dialog.run(Some(&self.window)) {
            if let Ok(folder) = self.folder_dialog.get_selected_item() {
                target.set_text(&folder.to_string_lossy());
                self.refresh_controls();
            }
        }
    }

    // ---- copy ------------------------------------------------------------

    fn start(&self) {
        if self.running.borrow().is_some() || !self.destinations_ready() || self.pending.borrow().is_empty() {
            return;
        }
        let job = Job {
            files: self.pending.borrow().clone(),
            dest1: PathBuf::from(self.dest1.text().trim()),
            dest2: PathBuf::from(self.dest2.text().trim()),
        };
        self.failures.borrow_mut().clear();
        self.progress.set_pos(0);

        let shared = Arc::new(Shared { queue: Mutex::new(Vec::new()), cancel: AtomicBool::new(false) });
        let worker_shared = Arc::clone(&shared);
        let sender = self.notice.sender();
        let tr = self.tr.borrow().clone().expect("translator");
        let files = job.files.clone();

        let thread = std::thread::spawn(move || {
            let push = |update: Update| {
                let mut queue = worker_shared.queue.lock().unwrap();
                // Only the latest progress snapshot matters.
                if let (Update::Progress(_), Some(Update::Progress(_))) = (&update, queue.last()) {
                    queue.pop();
                }
                queue.push(update);
                drop(queue);
                sender.notice();
            };
            let result = copier::run(&job, &worker_shared.cancel, &mut |event| match event {
                copier::Event::Progress(p) => push(Update::Progress(p)),
                copier::Event::FileDone { index } => push(Update::Done(index)),
                copier::Event::FileFailed { error, .. } => push(Update::Failed(tr.copy_error(error))),
            });
            push(Update::Finished(result.map_err(|e| tr.f("error.dest_folder", &[&e.to_string()]))));
        });

        *self.running.borrow_mut() = Some(Running { shared, files, thread: Some(thread) });
        self.set_status(StatusView::Starting);
        self.refresh_controls();
    }

    fn on_notice(&self) {
        let updates = {
            let running = self.running.borrow();
            let Some(running) = running.as_ref() else { return };
            std::mem::take(&mut *running.shared.queue.lock().unwrap())
        };

        let mut last_progress = None;
        let mut finished = None;
        for update in updates {
            match update {
                Update::Progress(p) => last_progress = Some(p),
                Update::Done(index) => self.remove_pending(index),
                Update::Failed(message) => self.failures.borrow_mut().push(message),
                Update::Finished(result) => finished = Some(result),
            }
        }

        if let Some(p) = last_progress {
            self.progress.set_pos((p.fraction() * PROGRESS_STEPS as f64) as u32);
            self.set_status(StatusView::Progress(p));
        }
        if let Some(result) = finished {
            self.finish(result);
        }
    }

    fn remove_pending(&self, job_index: usize) {
        let running = self.running.borrow();
        let Some(path) = running.as_ref().and_then(|r| r.files.get(job_index)) else { return };
        let mut pending = self.pending.borrow_mut();
        if let Some(pos) = pending.iter().position(|p| p == path) {
            pending.remove(pos);
            self.list.remove(pos);
        }
    }

    fn finish(&self, result: Result<Summary, String>) {
        if let Some(mut running) = self.running.borrow_mut().take() {
            if let Some(thread) = running.thread.take() {
                let _ = thread.join();
            }
        }
        let failures = std::mem::take(&mut *self.failures.borrow_mut());

        match result {
            Ok(summary) => {
                self.progress.set_pos(if summary.failed.is_empty() && !summary.cancelled { PROGRESS_STEPS } else { 0 });
                self.set_status(if summary.cancelled {
                    StatusView::Cancelled
                } else if summary.failed.is_empty() {
                    StatusView::Done { copied: summary.copied, elapsed: summary.elapsed, rate: summary.rate() }
                } else {
                    StatusView::Partial { copied: summary.copied, failed: summary.failed.len() }
                });
                if !failures.is_empty() {
                    self.show_error(&self.t_f("error.details", &[&failures.len().to_string(), &failures.join("\n")]));
                }
            }
            Err(message) => {
                self.set_status(StatusView::Ready);
                self.show_error(&message);
            }
        }
        self.refresh_controls();
    }

    fn show_error(&self, message: &str) {
        nwg::modal_error_message(&self.window, &self.t("error.title"), message);
    }

    fn on_close(&self) {
        // Ask the copy thread to stop (it removes any partial file) and wait for it.
        if let Some(mut running) = self.running.borrow_mut().take() {
            running.shared.cancel.store(true, Ordering::Relaxed);
            if let Some(thread) = running.thread.take() {
                let _ = thread.join();
            }
        }
        nwg::stop_thread_dispatch();
    }

    // ---- text helpers ----------------------------------------------------

    fn t_f(&self, key: &str, args: &[&str]) -> String {
        self.tr.borrow().as_ref().expect("translator").f(key, args)
    }

    fn rate_text(&self, bytes_per_sec: f64) -> String {
        let tr = self.tr.borrow();
        let tr = tr.as_ref().expect("translator");
        format!("{} {}", format::megabytes_per_second(bytes_per_sec, tr.decimal_separator()), tr.t("unit.rate"))
    }
}
