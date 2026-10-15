//! Widget tree and behavior of the original `DesktopPresetsWidget`.
//!
//! Method names follow the Python class. Blocking work runs on worker threads
//! like the original's `threading.Thread`s; their results come back through
//! channels to a task on the GTK main context (the original's
//! `GLib.idle_add`). Workers never touch GTK objects.

use std::cell::{Cell, RefCell};
use std::ffi::{OsStr, OsString};
use std::io;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::rc::Rc;
use std::time::Duration;

use adw::prelude::*;
use futures_channel::{mpsc, oneshot};
use futures_util::StreamExt;
use gtk::glib;

use crate::session::{self, display};
use crate::{process, pyformat};

/// Translation domain: `localization/<lang>/desktop_presets_dictionary.py`
/// (the key the Python Center picked for `b-desktop_presets.py`).
pub const DOMAIN: &str = "desktop_presets";

/// The original's injected `_()`.
fn tr(text: &str) -> String {
    linexin_plugin_api::tr(DOMAIN, text)
}

/// The original's injected `translate_dialog(dialog)`.
fn translate_dialog(dialog: &impl IsA<gtk::Widget>) {
    if let Some(host) = linexin_plugin_api::host() {
        // SAFETY: GTK main thread; `dialog` is a live widget.
        unsafe { host.translate_widget(DOMAIN, dialog.as_ref().as_ptr().cast()) };
    }
}

/// Run a GTK callback without letting a panic unwind into C.
pub fn guard(what: &str, callback: impl FnOnce()) {
    if catch_unwind(AssertUnwindSafe(callback)).is_err() {
        eprintln!("Desktop Presets: {what} panicked");
    }
}

/// Where the original lets a Python exception escape (a GTK callback or a
/// thread): PyGObject/threading print it and the rest of that step is skipped.
fn report_exception(context: &str, message: &str) {
    eprintln!("Desktop Presets: exception in {context}: {message}");
}

/// Start a worker thread like `threading.Thread(..., daemon=True).start()`.
/// When that fails, `work` (and the channel ends it owns) is dropped.
fn start_thread(name: &str, work: impl FnOnce() + Send + 'static) {
    let what = name.to_owned();
    let started = std::thread::Builder::new()
        .name(name.to_owned())
        .spawn(move || {
            if catch_unwind(AssertUnwindSafe(work)).is_err() {
                eprintln!("Desktop Presets: thread {what} panicked");
            }
        });
    if let Err(error) = started {
        report_exception(name, &format!("can't start new thread: {error}"));
    }
}

/// The data of `setup_plasma_interface()` / `setup_gnome_interface()`.
struct Interface {
    page_names: &'static [&'static str],
    script_base_path: OsString,
    monitor_script_path: Option<OsString>,
    script_paths: Vec<(&'static str, OsString)>,
}

impl Interface {
    fn gnome() -> Self {
        let home_dir = session::home_dir();
        let script_base_path = session::join(&home_dir, ".local/share/linexin/linexin-desktop/");
        let monitor_script_path = Some(session::join(&script_base_path, "update-monitor.sh"));
        let scripts = [
            ("Linexin", "default.sh"),
            ("Winexin", "windowish.sh"),
            ("Ubunexin", "ubunexin.sh"),
            ("Pure GNOME", "gnome.sh"),
        ];
        Self::new(
            &["Linexin", "Winexin", "Ubunexin", "Pure GNOME"],
            script_base_path,
            monitor_script_path,
            &scripts,
        )
    }

    fn plasma() -> Self {
        let home_dir = session::home_dir();
        let script_base_path =
            session::join(&home_dir, ".local/share/linexin/linexin-desktop/plasma/");
        let scripts = [
            ("Kinexin", "style1.sh"),
            ("10ish", "style2.sh"),
            ("11ish", "style3.sh"),
            ("2worlds", "style4.sh"),
            ("Plasmexin", "style5.sh"),
        ];
        Self::new(
            &["Kinexin", "10ish", "11ish", "2worlds", "Plasmexin"],
            script_base_path,
            None,
            &scripts,
        )
    }

    fn new(
        page_names: &'static [&'static str],
        script_base_path: OsString,
        monitor_script_path: Option<OsString>,
        scripts: &[(&'static str, &str)],
    ) -> Self {
        let script_paths = scripts
            .iter()
            .map(|(name, file)| (*name, session::join(&script_base_path, file)))
            .collect();
        Self {
            page_names,
            script_base_path,
            monitor_script_path,
            script_paths,
        }
    }

    /// `self.script_paths.get(name)`.
    fn script_path(&self, name: &str) -> Option<&OsString> {
        self.script_paths
            .iter()
            .find(|(page, _)| *page == name)
            .map(|(_, path)| path)
    }

    /// `setup_carousel()`'s previews; like the original it asks
    /// `is_plasma_session()` again.
    fn image_paths(&self) -> Vec<OsString> {
        let files: &[&str] = if session::is_plasma_session() {
            &[
                "style1.png",
                "style2.png",
                "style3.png",
                "style4.png",
                "style5.png",
            ]
        } else {
            &["default.png", "windowish.png", "ubunexin.png", "gnome.png"]
        };
        files
            .iter()
            .map(|file| session::join(&self.script_base_path, file))
            .collect()
    }
}

/// The worker an apply flow waits for.
enum Step {
    /// `task_run_monitor_script`, then the main script given here.
    Monitor(oneshot::Receiver<Option<String>>, OsString),
    Main(mpsc::UnboundedReceiver<MainScript>),
}

/// Start `task_run_monitor_script` on its thread.
fn start_monitor_script(monitor_script_path: OsString) -> oneshot::Receiver<Option<String>> {
    let (sender, receiver) = oneshot::channel();
    start_thread("task_run_monitor_script", move || {
        if let Some(error_message) = task_run_monitor_script(&monitor_script_path) {
            let _ = sender.send(error_message);
        }
    });
    receiver
}

/// Start `task_run_main_script` on its thread.
fn start_main_script(script_path: OsString) -> mpsc::UnboundedReceiver<MainScript> {
    let (sender, receiver) = mpsc::unbounded();
    start_thread("task_run_main_script", move || {
        task_run_main_script(&script_path, &sender)
    });
    receiver
}

/// Messages of `task_run_main_script` to the GTK thread.
enum MainScript {
    /// First run succeeded: the original's `idle_add(lambda: set_label(...))`.
    FirstRunSucceeded,
    /// `self.error_message` and `idle_add(self.finalize_ui)`.
    Finished(Option<String>),
}

struct Presets {
    root: glib::WeakRef<gtk::Box>,
    content_stack: gtk::Stack,
    btn_prev: gtk::Button,
    btn_next: gtk::Button,
    btn_apply: gtk::Button,
    interface: Interface,
    install_started: Cell<bool>,
    error_message: RefCell<Option<String>>,
    current_product: RefCell<Option<String>>,
}

/// Build the widget. `window` is the Center's window, resized to 1200×850 on
/// idle in single-widget mode like the original.
pub fn build(hide_sidebar: bool, window: Option<gtk::Window>) -> gtk::Box {
    let root = gtk::Box::new(gtk::Orientation::Vertical, 12);
    root.set_margin_top(12);
    root.set_margin_bottom(50);
    root.set_margin_start(12);
    root.set_margin_end(12);

    // The original's if/elif/else: anything but Plasma gets the GNOME presets.
    #[allow(clippy::if_same_then_else)]
    let interface = if session::is_plasma_session() {
        Interface::plasma()
    } else if session::is_gnome_session() {
        Interface::gnome()
    } else {
        Interface::gnome()
    };

    // setup_title()
    let title = gtk::Label::builder()
        .label(tr("Select Your Design"))
        .build();
    title.add_css_class("title-2");
    title.set_margin_bottom(6);
    root.append(&title);

    // setup_carousel()
    let nav_box = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    nav_box.set_halign(gtk::Align::Center);
    nav_box.set_vexpand(true);
    let btn_prev = gtk::Button::from_icon_name("go-previous-symbolic");
    btn_prev.set_valign(gtk::Align::Center);
    nav_box.append(&btn_prev);
    let content_stack = gtk::Stack::new();
    content_stack.set_transition_type(gtk::StackTransitionType::SlideLeftRight);
    content_stack.set_hexpand(true);
    content_stack.set_vexpand(true);
    nav_box.append(&content_stack);
    let btn_next = gtk::Button::from_icon_name("go-next-symbolic");
    btn_next.set_valign(gtk::Align::Center);
    nav_box.append(&btn_next);
    let image_paths = interface.image_paths();
    for (index, page_name) in interface.page_names.iter().enumerate() {
        let Some(image_path) = image_paths.get(index) else {
            break;
        };
        content_stack.add_named(&preview_page(page_name, image_path), Some(page_name));
    }
    content_stack.set_visible_child_name(interface.page_names[0]);
    root.append(&nav_box);

    // setup_controls()
    let controls_box = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    controls_box.set_halign(gtk::Align::Center);
    controls_box.set_margin_top(6);
    let btn_apply = gtk::Button::builder().label(tr("Apply Style")).build();
    btn_apply.add_css_class("suggested-action");
    btn_apply.add_css_class("buttons_all");
    controls_box.append(&btn_apply);
    root.append(&controls_box);

    let presets = Rc::new(Presets {
        root: root.downgrade(),
        content_stack,
        btn_prev,
        btn_next,
        btn_apply,
        interface,
        install_started: Cell::new(false),
        error_message: RefCell::new(None),
        current_product: RefCell::new(None),
    });
    // Handlers hold weak references: the state owns the children, the root
    // owns the state (below), nothing owns the root.
    let weak = Rc::downgrade(&presets);
    presets.btn_prev.connect_clicked({
        let weak = weak.clone();
        move |_| {
            guard("previous", || {
                if let Some(presets) = weak.upgrade() {
                    presets.on_prev_clicked();
                }
            })
        }
    });
    presets.btn_next.connect_clicked({
        let weak = weak.clone();
        move |_| {
            guard("next", || {
                if let Some(presets) = weak.upgrade() {
                    presets.on_next_clicked();
                }
            })
        }
    });
    presets.content_stack.connect_visible_child_notify({
        let weak = weak.clone();
        move |_| {
            guard("page change", || {
                if let Some(presets) = weak.upgrade() {
                    presets.update_ui_for_page_change();
                }
            })
        }
    });
    presets.btn_apply.connect_clicked(move |_| {
        guard("apply", || {
            if let Some(presets) = weak.upgrade() {
                presets.on_apply_clicked();
            }
        })
    });
    presets.update_ui_for_page_change();

    let resize = Rc::new(RefCell::new(None::<glib::SourceId>));
    if hide_sidebar {
        let window = window.map(|window| window.downgrade());
        let pending = resize.clone();
        let source = glib::idle_add_local_once(move || {
            pending.take();
            guard("window resize", || resize_window_deferred(window.as_ref()));
        });
        resize.replace(Some(source));
    }

    // The pending resize goes with the widget. A running apply flow keeps
    // the state (never the root) until its scripts are done, like the
    // original's threads and idle callbacks kept the Python object.
    let state = RefCell::new(Some(presets));
    root.connect_destroy(move |_| {
        guard("cleanup", || {
            if let Some(source) = resize.take() {
                source.remove();
            }
            state.take();
        })
    });
    root
}

/// One carousel page: the preview (or its placeholder) above the name.
fn preview_page(page_name: &str, image_path: &OsStr) -> gtk::Box {
    println!("Looking for image: {}", display(image_path));
    let image_exists = session::exists(image_path);
    println!(
        "Image exists: {}",
        if image_exists { "True" } else { "False" }
    );
    let page_container = gtk::Box::new(gtk::Orientation::Vertical, 8);
    page_container.set_valign(gtk::Align::Center);
    page_container.set_halign(gtk::Align::Center);
    if session::exists(image_path) {
        println!("Loading image: {}", display(image_path));
        let picture = gtk::Picture::new();
        #[allow(deprecated)] // as in the original; content-fit overrides it
        picture.set_keep_aspect_ratio(true);
        picture.set_can_shrink(true);
        picture.set_content_fit(gtk::ContentFit::ScaleDown);
        picture.set_size_request(300, 200);
        picture.set_filename(Some(image_path));
        page_container.append(&picture);
    } else {
        println!(
            "Image not found, showing placeholder for: {}",
            display(image_path)
        );
        let fallback = gtk::Box::builder().build();
        fallback.set_size_request(300, 200);
        fallback.add_css_class("card");
        let placeholder_label = gtk::Label::builder()
            .label(tr("Preview not available"))
            .build();
        placeholder_label.add_css_class("dim-label");
        fallback.append(&placeholder_label);
        fallback.set_halign(gtk::Align::Center);
        fallback.set_valign(gtk::Align::Center);
        page_container.append(&fallback);
    }
    let label = gtk::Label::builder().label(page_name).build();
    label.add_css_class("title-3");
    page_container.append(&label);
    page_container
}

/// `resize_window_deferred()`.
fn resize_window_deferred(window: Option<&glib::WeakRef<gtk::Window>>) {
    if let Some(window) = window.and_then(|window| window.upgrade()) {
        window.set_default_size(1200, 850);
        println!("Window default size set to 1200x850");
    }
}

impl Presets {
    /// Index of the visible page in `page_names`.
    fn current_index(&self) -> Option<usize> {
        let current_name = self.content_stack.visible_child_name()?;
        self.interface
            .page_names
            .iter()
            .position(|name| *name == current_name.as_str())
    }

    fn on_prev_clicked(&self) {
        if let Some(current_index) = self.current_index()
            && current_index > 0
        {
            self.content_stack
                .set_visible_child_name(self.interface.page_names[current_index - 1]);
        }
    }

    fn on_next_clicked(&self) {
        if let Some(current_index) = self.current_index()
            && current_index < self.interface.page_names.len() - 1
        {
            self.content_stack
                .set_visible_child_name(self.interface.page_names[current_index + 1]);
        }
    }

    fn update_ui_for_page_change(&self) {
        let Some(current_index) = self.current_index() else {
            return;
        };
        if !self.install_started.get() {
            let last = self.interface.page_names.len() - 1;
            self.btn_prev.set_sensitive(current_index > 0);
            self.btn_next.set_sensitive(current_index < last);
            self.btn_prev
                .set_opacity(if current_index > 0 { 1.0 } else { 0.3 });
            self.btn_next
                .set_opacity(if current_index < last { 1.0 } else { 0.3 });
        }
    }

    fn on_apply_clicked(self: &Rc<Self>) {
        let selected_desktop = self
            .content_stack
            .visible_child_name()
            .map(|name| name.to_string());
        let script_to_run = selected_desktop
            .as_deref()
            .and_then(|name| self.interface.script_path(name))
            .cloned();
        let script_ok = script_to_run
            .as_deref()
            .is_some_and(|script| session::exists(script) && session::is_executable(script));
        if !script_ok {
            let shown = script_to_run.as_deref().map_or("None".into(), display);
            self.show_error(
                &tr("Error"),
                "Main script not found or is not executable:\n{}",
                &shown,
            );
            return;
        }
        if let Some(monitor) = &self.interface.monitor_script_path
            && !monitor.is_empty()
            && !(session::exists(monitor) && session::is_executable(monitor))
        {
            self.show_error(
                &tr("Error"),
                "Monitor script not found or is not executable:\n{}",
                &display(monitor),
            );
            return;
        }
        if let (Some(script), Some(product)) = (script_to_run, selected_desktop) {
            self.begin_install(script, product);
        }
    }

    /// `show_error_dialog(heading, _(template).format(value))`.
    fn show_error(&self, heading: &str, template: &str, value: &str) {
        match pyformat::format(&tr(template), &[value]) {
            Ok(body) => self.show_error_dialog(heading, &body),
            Err(error) => report_exception("on_apply_clicked", &error),
        }
    }

    fn begin_install(self: &Rc<Self>, script_path: OsString, product_name: String) {
        self.error_message.replace(None);
        self.install_started.set(true);
        self.btn_prev.set_sensitive(false);
        self.btn_next.set_sensitive(false);
        self.btn_prev.set_opacity(0.3);
        self.btn_next.set_opacity(0.3);
        self.btn_apply.set_sensitive(false);
        self.btn_apply.set_label(&tr("Updating display..."));
        self.current_product.replace(Some(product_name));
        // The first worker starts here, in the click handler, like the original.
        let first = match self.interface.monitor_script_path.clone() {
            None => {
                self.btn_apply.set_label(&tr("Applying settings..."));
                Step::Main(start_main_script(script_path))
            }
            Some(monitor_script_path) => {
                self.btn_apply.set_label(&tr("Updating display..."));
                Step::Monitor(start_monitor_script(monitor_script_path), script_path)
            }
        };
        let presets = Rc::clone(self);
        let flow = async move {
            let mut receiver = match first {
                Step::Main(receiver) => receiver,
                Step::Monitor(receiver, script_path) => {
                    // No report (the thread could not start, or died like a
                    // Python exception escaping it): the UI stays busy, as in
                    // the original.
                    let Ok(error_message) = receiver.await else {
                        return;
                    };
                    let mut proceed = false;
                    guard("monitor script done", || {
                        proceed = presets.on_monitor_script_done(error_message);
                    });
                    if !proceed {
                        return;
                    }
                    start_main_script(script_path)
                }
            };
            // Ends when the worker is done (or could not start).
            while let Some(message) = receiver.next().await {
                guard("main script progress", || match message {
                    MainScript::FirstRunSucceeded => {
                        presets.btn_apply.set_label(&tr("Applying settings..."));
                    }
                    MainScript::Finished(error_message) => {
                        presets.error_message.replace(error_message);
                        presets.finalize_ui();
                    }
                });
            }
        };
        glib::MainContext::default().spawn_local_with_priority(glib::Priority::DEFAULT_IDLE, flow);
    }

    /// Returns whether the main script runs next.
    fn on_monitor_script_done(&self, error_message: Option<String>) -> bool {
        self.error_message.replace(error_message);
        if self.has_error() {
            self.finalize_ui();
            false
        } else {
            self.btn_apply.set_label(&tr("Applying settings..."));
            true
        }
    }

    /// `if self.error_message:` (an empty message counts as none).
    fn has_error(&self) -> bool {
        self.error_message
            .borrow()
            .as_deref()
            .is_some_and(|message| !message.is_empty())
    }

    /// `str(self.current_product)`.
    fn product(&self) -> String {
        self.current_product
            .borrow()
            .clone()
            .unwrap_or_else(|| "None".into())
    }

    fn finalize_ui(&self) {
        self.install_started.set(false);
        self.btn_apply.set_sensitive(true);
        self.btn_apply.set_label(&tr("Apply Style"));
        self.update_ui_for_page_change();
        if self.has_error() {
            let dialog_heading = tr("Operation Failed");
            let error_message = self.error_message.borrow().clone().unwrap_or_default();
            match pyformat::format(
                &tr("Could not apply style for {}.\n\nError: {}"),
                &[&self.product(), &error_message],
            ) {
                Ok(dialog_body) => self.show_error_dialog(&dialog_heading, &dialog_body),
                Err(error) => report_exception("finalize_ui", &error),
            }
        } else {
            self.show_logout_dialog();
        }
    }

    /// `Adw.MessageDialog(heading=, body=, transient_for=self.get_root(), modal=True)`.
    #[allow(deprecated)] // Adw.MessageDialog, as in the original.
    fn message_dialog(&self, heading: &str, body: &str) -> adw::MessageDialog {
        let parent_window = self
            .root
            .upgrade()
            .and_then(|root| root.root())
            .and_then(|root| root.downcast::<gtk::Window>().ok());
        let builder = adw::MessageDialog::builder().heading(heading).body(body);
        let builder = match &parent_window {
            Some(parent_window) => builder.transient_for(parent_window),
            None => builder,
        };
        builder.modal(true).build()
    }

    #[allow(deprecated)] // Adw.MessageDialog, as in the original.
    fn show_error_dialog(&self, heading: &str, body: &str) {
        let dialog = self.message_dialog(heading, body);
        dialog.add_response("ok", &tr("OK"));
        dialog.set_default_response(Some("ok"));
        dialog.set_close_response("ok");
        translate_dialog(&dialog);
        dialog.present();
    }

    #[allow(deprecated)] // Adw.MessageDialog, as in the original.
    fn show_logout_dialog(&self) {
        let body_template =
            tr("To complete the change to {}, you need to log out and log back in again.");
        let body = match pyformat::format(&body_template, &[&self.product()]) {
            Ok(body) => body,
            Err(error) => return report_exception("show_logout_dialog", &error),
        };
        let dialog = self.message_dialog(&tr("Restart Required"), &body);
        dialog.add_response("later", &tr("Later"));
        dialog.add_response("logout", &tr("Log Out Now"));
        dialog.set_response_appearance("logout", adw::ResponseAppearance::Suggested);
        dialog.set_default_response(Some("later"));
        dialog.set_close_response("later");
        dialog.connect_response(None, |_, response| {
            guard("logout response", || on_logout_dialog_response(response))
        });
        translate_dialog(&dialog);
        dialog.present();
    }
}

/// `task_run_monitor_script()` on its worker thread: `Some(error_message)`,
/// or `None` where the original's thread died before reporting back.
fn task_run_monitor_script(monitor_script_path: &OsStr) -> Option<Option<String>> {
    let attempt = || -> Result<Option<String>, String> {
        let command = [OsStr::new("/bin/bash"), monitor_script_path];
        let process = process::communicate(&command)?;
        if process.returncode != 0 {
            let message = pyformat::format(
                &tr("Monitor script failed with code {0}.\n\nDetails: {1}"),
                &[
                    &process.returncode.to_string(),
                    process::strip(&process.stderr),
                ],
            )?;
            return Ok(Some(message));
        }
        Ok(None)
    };
    match attempt() {
        Ok(error_message) => Some(error_message),
        Err(exception) => match pyformat::format(
            &tr("An error occurred during monitor update: {}"),
            &[&exception],
        ) {
            Ok(message) => Some(Some(message)),
            Err(error) => {
                report_exception("task_run_monitor_script", &error);
                None
            }
        },
    }
}

/// `task_run_main_script()` on its worker thread: the script twice, one and
/// two seconds apart.
fn task_run_main_script(script_path: &OsStr, sender: &mpsc::UnboundedSender<MainScript>) {
    // A closed channel means the widget is gone; the scripts still run, as in
    // the original.
    let send = |message| {
        let _ = sender.unbounded_send(message);
    };
    // Ok(None): returned early after reporting the first run's failure.
    let attempt = || -> Result<Option<Option<String>>, String> {
        std::thread::sleep(Duration::from_secs(1));
        let command = [OsStr::new("/bin/bash"), script_path];
        let process = process::communicate(&command)?;
        if process.returncode != 0 {
            let message = pyformat::format(
                &tr("Main script (first run) failed with code {0}.\n\nDetails: {1}"),
                &[
                    &process.returncode.to_string(),
                    process::strip(&process.stderr),
                ],
            )?;
            send(MainScript::Finished(Some(message)));
            return Ok(None);
        }
        send(MainScript::FirstRunSucceeded);
        std::thread::sleep(Duration::from_secs(2));
        let process = process::communicate(&command)?;
        if process.returncode != 0 {
            let message = pyformat::format(
                &tr("Main script (second run) failed with code {0}.\n\nDetails: {1}"),
                &[
                    &process.returncode.to_string(),
                    process::strip(&process.stderr),
                ],
            )?;
            return Ok(Some(Some(message)));
        }
        Ok(Some(None))
    };
    let error_message = match attempt() {
        Ok(None) => return,
        Ok(Some(error_message)) => error_message,
        Err(exception) => match pyformat::format(
            &tr("An unexpected error occurred while running the main script: {}"),
            &[&exception],
        ) {
            Ok(message) => Some(message),
            Err(error) => return report_exception("task_run_main_script", &error),
        },
    };
    send(MainScript::Finished(error_message));
}

/// `on_logout_dialog_response()`.
fn on_logout_dialog_response(response: &str) {
    if response == "logout" {
        start_thread("delayed_logout", delayed_logout);
    }
}

fn delayed_logout() {
    std::thread::sleep(Duration::from_millis(500));
    if session::is_plasma_session() {
        let Err(error) = process::run(&["qdbus6", "org.kde.Shutdown", "/Shutdown", "logout"])
        else {
            return;
        };
        if error.kind() != io::ErrorKind::NotFound {
            return report_exception("delayed_logout", &error.to_string());
        }
        let Err(error) = process::run(&[
            "qdbus",
            "org.kde.ksmserver",
            "/KSMServer",
            "logout",
            "0",
            "0",
            "0",
        ]) else {
            return;
        };
        if error.kind() != io::ErrorKind::NotFound {
            return report_exception("delayed_logout", &error.to_string());
        }
        let user = std::env::var_os("USER").unwrap_or_default();
        let command = [OsStr::new("loginctl"), OsStr::new("terminate-user"), &user];
        if let Err(error) = process::run(&command) {
            report_exception("delayed_logout", &error.to_string());
        }
    } else {
        match process::spawn(&["gnome-session-quit", "--logout", "--no-prompt"]) {
            // Not waited for by the original either; reaped here.
            Ok(mut child) => {
                let _ = child.wait();
            }
            Err(error) => report_exception("delayed_logout", &error.to_string()),
        }
    }
}
