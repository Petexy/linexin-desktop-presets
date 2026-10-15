//! Development harness: loads the built widget library through the C ABI with a
//! host whose translation services are the Center's own (dictionaries,
//! `{}` patterns and `translate_dialog` from linexin-localization), shows it in
//! an `AdwApplicationWindow`, drives it like a user and renders PNGs.
//!
//! ```sh
//! cargo build --lib --examples   # --lib: examples alone leave target/debug/*.so stale
//! target/debug/examples/harness --lang pl_PL --clicks nn --output out.png
//! ```
//! Run it only inside a headless compositor with a scratch HOME and fake
//! commands (examples/compare.sh does that): `--apply` runs the preset scripts
//! of `$HOME/.local/share/linexin/linexin-desktop` and `--respond logout` the
//! logout commands. `examples/python_reference.py` takes the same options.
//!
//! Options: `--clicks SEQ` presses previous/next (`p`/`n`) before `--output`;
//! `--apply` then presses Apply, `--running-output` renders 0.6 s later,
//! `--second-output` 0.6 s after the button's label next changes,
//! `--dialog-output`/`--after-output` render the dialog that follows and the
//! window behind it, `--respond ID` answers that dialog. `--destroy-during-apply`
//! drops the widget right after Apply. Every run removes the widget at the end
//! and reports whether it was finalized.

use std::cell::RefCell;
use std::ffi::{CStr, CString, c_char, c_int, c_void};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use adw::prelude::*;
use gtk::glib::translate::{FromGlibPtrBorrow, FromGlibPtrFull};
use gtk::{gio, glib, graphene};
use linexin_localization::{Localization, widgets};
use linexin_plugin_api::{
    ABI_VERSION, CreateContextV1, ENTRY_SYMBOL, EntryV1, Header, HostApiV1, WidgetDescriptorV1,
};

const MANIFEST_DIR: &str = env!("CARGO_MANIFEST_DIR");

struct Options {
    library: PathBuf,
    localization: PathBuf,
    /// The Center's stylesheet; its own copy (`CENTER_STYLE_CSS`) without one.
    css: Option<PathBuf>,
    language: CString,
    dark: bool,
    width: i32,
    height: i32,
    hide_sidebar: bool,
    clicks: String,
    output: Option<PathBuf>,
    apply: bool,
    running_output: Option<PathBuf>,
    second_output: Option<PathBuf>,
    dialog_output: Option<PathBuf>,
    after_output: Option<PathBuf>,
    respond: Option<String>,
    destroy_during_apply: bool,
}

static OPTIONS: OnceLock<Options> = OnceLock::new();

fn options() -> &'static Options {
    OPTIONS
        .get()
        .expect("options are set before the host is used")
}

fn parse_options() -> Result<Options, String> {
    let manifest = Path::new(MANIFEST_DIR);
    let mut options = Options {
        library: beside_the_examples("liblinexin_desktop_presets.so"),
        localization: manifest.join("data/usr/share/linexin/widgets/localization"),
        css: None,
        language: c"en_US".into(),
        dark: false,
        width: 900,
        height: 750,
        hide_sidebar: false,
        clicks: String::new(),
        output: None,
        apply: false,
        running_output: None,
        second_output: None,
        dialog_output: None,
        after_output: None,
        respond: None,
        destroy_during_apply: false,
    };
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        let mut value = || args.next().ok_or(format!("{arg} needs a value"));
        match arg.as_str() {
            "--library" => options.library = value()?.into(),
            "--localization" => options.localization = value()?.into(),
            "--css" => options.css = Some(value()?.into()),
            "--lang" => options.language = CString::new(value()?).map_err(|e| e.to_string())?,
            "--dark" => options.dark = true,
            "--width" => options.width = value()?.parse().map_err(|_| "bad --width")?,
            "--height" => options.height = value()?.parse().map_err(|_| "bad --height")?,
            "--hide-sidebar" => options.hide_sidebar = true,
            "--clicks" => options.clicks = value()?,
            "--output" => options.output = Some(value()?.into()),
            "--apply" => options.apply = true,
            "--running-output" => options.running_output = Some(value()?.into()),
            "--second-output" => options.second_output = Some(value()?.into()),
            "--dialog-output" => options.dialog_output = Some(value()?.into()),
            "--after-output" => options.after_output = Some(value()?.into()),
            "--respond" => options.respond = Some(value()?),
            "--destroy-during-apply" => options.destroy_during_apply = true,
            _ => return Err(format!("unknown argument {arg}")),
        }
    }
    Ok(options)
}

/// Install the Center's catalog for the chosen language as the process-wide one.
fn install_localization() {
    let options = options();
    let localization = Localization::load(
        std::slice::from_ref(&options.localization),
        &options.language.to_string_lossy(),
        None,
    );
    for diagnostic in localization.diagnostics() {
        eprintln!("localization: {diagnostic}");
    }
    linexin_localization::install(localization);
}

fn localization() -> &'static Localization {
    linexin_localization::global()
}

unsafe extern "C" fn language() -> *const c_char {
    options().language.as_ptr()
}

unsafe extern "C" fn translate(domain: *const c_char, text: *const c_char) -> *mut c_char {
    if domain.is_null() || text.is_null() {
        return std::ptr::null_mut();
    }
    let (domain, text) = unsafe { (CStr::from_ptr(domain), CStr::from_ptr(text)) };
    let translated = localization().translate(&text.to_string_lossy(), &domain.to_string_lossy());
    CString::new(translated).map_or(std::ptr::null_mut(), CString::into_raw)
}

unsafe extern "C" fn free_string(string: *mut c_char) {
    if !string.is_null() {
        drop(unsafe { CString::from_raw(string) });
    }
}

/// The Center's `translate_widget` (original `translate_dialog`).
unsafe extern "C" fn translate_widget(domain: *const c_char, widget: *mut c_void) {
    if domain.is_null() || widget.is_null() {
        return;
    }
    let domain = unsafe { CStr::from_ptr(domain) }.to_string_lossy();
    let widget = unsafe { gtk::Widget::from_glib_borrow(widget.cast::<gtk::ffi::GtkWidget>()) };
    widgets::translate_dialog(&widget, &domain);
}

unsafe extern "C" fn no_op() {}
unsafe extern "C" fn validate_password(_: *const c_char) -> bool {
    false
}
unsafe extern "C" fn set_password(_: *const c_char) {}
unsafe extern "C" fn has_password() -> bool {
    false
}
unsafe extern "C" fn wrapper_path() -> *const c_char {
    c"/bin/false".as_ptr()
}
unsafe extern "C" fn run_privileged(_: *const *const c_char) -> c_int {
    -1
}

unsafe extern "C" fn askpass_path() -> *const c_char {
    c"/bin/false".as_ptr()
}

unsafe extern "C" fn not_compact(_: *mut c_void) -> bool {
    false
}

static HOST: HostApiV1 = HostApiV1 {
    abi_version: ABI_VERSION,
    struct_size: size_of::<HostApiV1>() as u32,
    language,
    translate,
    free_string,
    translate_widget,
    lock_ui: no_op,
    unlock_ui: no_op,
    sudo_validate_password: validate_password,
    sudo_set_password: set_password,
    sudo_has_password: has_password,
    sudo_forget_password: no_op,
    sudo_wrapper_path: wrapper_path,
    sudo_start_session: no_op,
    sudo_stop_session: no_op,
    sudo_run_privileged: run_privileged,
    refresh_widgets: no_op,
    sudo_askpass_path: askpass_path,
    window_compact_mode: not_compact,
};

fn load_descriptor(path: &Path) -> Result<&'static WidgetDescriptorV1, String> {
    // SAFETY: a trusted development build of the widget.
    let library = unsafe { libloading::Library::new(path) }.map_err(|e| e.to_string())?;
    // Like the Center, never unload: GTK keeps pointers into the library.
    let library = Box::leak(Box::new(library));
    let entry = unsafe { library.get::<EntryV1>(ENTRY_SYMBOL) }.map_err(|e| e.to_string())?;
    let descriptor = unsafe { entry(&HOST) };
    if !unsafe { Header::is_compatible(descriptor.cast(), size_of::<WidgetDescriptorV1>()) } {
        return Err("incompatible descriptor".into());
    }
    Ok(unsafe { &*descriptor })
}

/// Descendants of `widget` (itself included) of type `T`, depth first.
fn find_all<T: IsA<gtk::Widget>>(widget: &gtk::Widget, found: &mut Vec<T>) {
    if let Ok(widget) = widget.clone().downcast::<T>() {
        found.push(widget);
    }
    let mut child = widget.first_child();
    while let Some(current) = child {
        find_all(&current, found);
        child = current.next_sibling();
    }
}

fn render(widget: &impl IsA<gtk::Widget>, path: &Path) -> Result<(), String> {
    let widget = widget.as_ref();
    let (width, height) = (widget.width(), widget.height());
    let snapshot = gtk::Snapshot::new();
    gtk::WidgetPaintable::new(Some(widget)).snapshot(&snapshot, width.into(), height.into());
    let node = snapshot.to_node().ok_or("nothing was drawn")?;
    let renderer = widget
        .native()
        .and_then(|n| n.renderer())
        .ok_or("no renderer")?;
    let viewport = graphene::Rect::new(0.0, 0.0, width as f32, height as f32);
    let texture = renderer.render_texture(&node, Some(&viewport));
    texture.save_to_png(path).map_err(|e| e.to_string())?;
    println!("rendered {width}x{height} to {}", path.display());
    Ok(())
}

/// Widgets the scenarios read and press, found like a user would.
struct Controls {
    stack: gtk::Stack,
    previous: gtk::Button,
    next: gtk::Button,
    apply: gtk::Button,
}

impl Controls {
    fn find(widget: &gtk::Widget) -> Result<Self, String> {
        let mut stacks = Vec::new();
        find_all::<gtk::Stack>(widget, &mut stacks);
        let mut buttons = Vec::new();
        find_all::<gtk::Button>(widget, &mut buttons);
        match (stacks.as_slice(), buttons.as_slice()) {
            ([stack], [previous, next, apply]) => Ok(Self {
                stack: stack.clone(),
                previous: previous.clone(),
                next: next.clone(),
                apply: apply.clone(),
            }),
            _ => Err(format!(
                "unexpected tree: {} stacks, {} buttons",
                stacks.len(),
                buttons.len()
            )),
        }
    }

    fn state(&self) -> String {
        let button = |button: &gtk::Button| {
            format!(
                "sensitive={} opacity={:.1}",
                button.is_sensitive(),
                button.opacity()
            )
        };
        format!(
            "page={:?} previous[{}] next[{}] apply[{:?} {}]",
            self.stack.visible_child_name().unwrap_or_default(),
            button(&self.previous),
            button(&self.next),
            self.apply.label().unwrap_or_default(),
            button(&self.apply)
        )
    }
}

#[allow(deprecated)] // the widget's Adw.MessageDialog
fn message_dialogs() -> Vec<adw::MessageDialog> {
    gtk::Window::list_toplevels()
        .into_iter()
        .filter_map(|window| window.downcast::<adw::MessageDialog>().ok())
        .filter(|dialog| dialog.is_visible())
        .collect()
}

#[allow(deprecated)]
fn describe_dialog(dialog: &adw::MessageDialog) -> String {
    let mut responses = Vec::new();
    for id in ["ok", "later", "logout"] {
        if dialog.has_response(id) {
            responses.push(format!(
                "{id}={:?}/{:?}",
                dialog.response_label(id),
                dialog.response_appearance(id)
            ));
        }
    }
    format!(
        "dialog heading={:?} body={:?} responses=[{}] default={:?} close={:?} modal={} transient={}",
        dialog.heading().unwrap_or_default(),
        dialog.body(),
        responses.join(", "),
        dialog.default_response().unwrap_or_default(),
        dialog.close_response(),
        dialog.is_modal(),
        dialog.transient_for().is_some()
    )
}

async fn sleep(duration: Duration) {
    glib::timeout_future(duration).await;
}

/// Wait until `condition` holds (checked every 20 ms) or `limit` passes.
async fn wait_until(limit: Duration, mut condition: impl FnMut() -> bool) -> bool {
    let started = Instant::now();
    while !condition() {
        if started.elapsed() > limit {
            return false;
        }
        sleep(Duration::from_millis(20)).await;
    }
    true
}

/// Time for the stack's slide and button state transitions to finish.
const SETTLE: Duration = Duration::from_millis(1000);

async fn scenario(window: adw::ApplicationWindow, widget: gtk::Widget) -> Result<(), String> {
    let options = options();
    sleep(SETTLE).await;
    let controls = Controls::find(&widget)?;
    for click in options.clicks.chars() {
        match click {
            'n' => controls.next.emit_clicked(),
            'p' => controls.previous.emit_clicked(),
            _ => return Err(format!("unknown click {click:?}")),
        }
        sleep(Duration::from_millis(400)).await;
    }
    sleep(SETTLE).await;
    println!("state: {}", controls.state());
    if options.hide_sidebar {
        println!("window default size: {:?}", window.default_size());
    }
    if let Some(path) = &options.output {
        render(&window, path)?;
    }
    if !options.apply {
        return finish(&window, widget).await;
    }

    let dialogs_before = message_dialogs().len();
    controls.apply.emit_clicked();
    let first_label = controls.apply.label().unwrap_or_default();
    println!("after apply: {}", controls.state());
    if options.destroy_during_apply {
        let weak = widget.downgrade();
        let weak_apply = controls.apply.downgrade();
        window.set_content(None::<&gtk::Widget>);
        drop((controls, widget));
        sleep(Duration::from_millis(300)).await;
        println!(
            "widget finalized during apply: {}",
            weak.upgrade().is_none()
        );
        // Outlive the worker threads (monitor + 1 s + run + 2 s + run).
        sleep(Duration::from_secs(7)).await;
        let dialogs = message_dialogs();
        println!(
            "dialogs after the scripts finished: {}",
            dialogs.len() - dialogs_before
        );
        for dialog in &dialogs[dialogs_before..] {
            println!("{}", describe_dialog(dialog));
            dialog.destroy();
        }
        sleep(Duration::from_millis(300)).await;
        println!(
            "widget finalized (state) after the flow: {}",
            weak_apply.upgrade().is_none()
        );
        return Ok(());
    }
    if let Some(path) = &options.running_output {
        sleep(Duration::from_millis(600)).await;
        println!("running: {}", controls.state());
        render(&window, path)?;
    }
    let dialog_shown = || message_dialogs().len() > dialogs_before;
    let changed = wait_until(Duration::from_secs(20), || {
        controls.apply.label().unwrap_or_default() != first_label || dialog_shown()
    })
    .await;
    if changed && !dialog_shown() {
        println!("label changed: {}", controls.state());
        if let Some(path) = &options.second_output {
            sleep(Duration::from_millis(600)).await;
            render(&window, path)?;
        }
    }
    if !wait_until(Duration::from_secs(30), dialog_shown).await {
        println!("no dialog: {}", controls.state());
        return finish(&window, widget).await;
    }
    sleep(SETTLE).await;
    let dialogs = message_dialogs();
    let dialog = dialogs.last().ok_or("dialog vanished")?;
    println!("{}", describe_dialog(dialog));
    println!("after dialog: {}", controls.state());
    if let Some(path) = &options.dialog_output {
        render(dialog, path)?;
    }
    if let Some(path) = &options.after_output {
        render(&window, path)?;
    }
    if let Some(response) = &options.respond {
        #[allow(deprecated)]
        dialog.response(response);
        // The logout runs 0.5 s later on a thread.
        sleep(Duration::from_secs(2)).await;
        println!(
            "dialogs after responding: {}",
            message_dialogs().len() - dialogs_before
        );
    }
    finish(&window, widget).await
}

async fn finish(window: &adw::ApplicationWindow, widget: gtk::Widget) -> Result<(), String> {
    for dialog in message_dialogs() {
        dialog.destroy();
    }
    let weak = widget.downgrade();
    window.set_content(None::<&gtk::Widget>);
    drop(widget);
    // GTK drops its last internal references on the next frames.
    sleep(Duration::from_millis(300)).await;
    println!(
        "widget finalized after removal: {}",
        weak.upgrade().is_none()
    );
    Ok(())
}

fn activate(app: &adw::Application) -> Result<(), String> {
    let options = options();
    adw::StyleManager::default().set_color_scheme(if options.dark {
        adw::ColorScheme::ForceDark
    } else {
        adw::ColorScheme::ForceLight
    });
    // The Center's application style sheet (.buttons_all, .dim-label, ...).
    let css = gtk::CssProvider::new();
    css.load_from_string(&match &options.css {
        Some(path) => {
            std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?
        }
        None => linexin_plugin_api::CENTER_STYLE_CSS.to_owned(),
    });
    let display = gtk::gdk::Display::default().ok_or("no display")?;
    gtk::style_context_add_provider_for_display(
        &display,
        &css,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );

    let descriptor = load_descriptor(&options.library)?;
    let text = |pointer| {
        unsafe { CStr::from_ptr(pointer) }
            .to_string_lossy()
            .into_owned()
    };
    let (id, name) = (text(descriptor.widget_id), text(descriptor.name));
    println!(
        "widget_id={id} name={name:?} sidebar={:?} icon={} lxb_page={}",
        localization().translate(&name, &id),
        text(descriptor.icon),
        descriptor.lxb_create.is_some()
    );

    let window = adw::ApplicationWindow::new(app);
    window.set_default_size(options.width, options.height);
    let context = CreateContextV1 {
        abi_version: ABI_VERSION,
        struct_size: size_of::<CreateContextV1>() as u32,
        host: &HOST,
        window: window.upcast_ref::<gtk::Window>().as_ptr().cast(),
        hide_sidebar: options.hide_sidebar,
        voice_autostart: false,
        conversation_id: std::ptr::null(),
    };
    let create = descriptor.create.ok_or("descriptor without create")?;
    let pointer = unsafe { create(&context) };
    if pointer.is_null() {
        return Err("create returned null".into());
    }
    let widget = unsafe { gtk::Widget::from_glib_full(pointer.cast::<gtk::ffi::GtkWidget>()) };
    window.set_content(Some(&widget));
    window.present();

    let app = app.clone();
    glib::MainContext::default().spawn_local(async move {
        if let Err(error) = scenario(window, widget).await {
            eprintln!("harness: {error}");
            FAILED.with(|failed| failed.replace(true));
        }
        app.quit();
    });
    Ok(())
}

thread_local! {
    static FAILED: RefCell<bool> = const { RefCell::new(false) };
}

fn main() -> ExitCode {
    // The stylesheet, for the Python reference to load the same one.
    if std::env::args().nth(1).as_deref() == Some("--print-css") {
        print!("{}", linexin_plugin_api::CENTER_STYLE_CSS);
        return ExitCode::SUCCESS;
    }
    match parse_options() {
        Ok(options) => {
            let _ = OPTIONS.set(options);
            install_localization();
        }
        Err(error) => {
            eprintln!("harness: {error}");
            return ExitCode::FAILURE;
        }
    }
    let app = adw::Application::builder()
        .flags(gio::ApplicationFlags::NON_UNIQUE)
        .build();
    app.connect_activate(|app| {
        if let Err(error) = activate(app) {
            eprintln!("harness: {error}");
            FAILED.with(|failed| failed.replace(true));
            app.quit();
        }
    });
    let status = app.run_with_args::<&str>(&[]);
    if status == glib::ExitCode::SUCCESS && !FAILED.with(|failed| *failed.borrow()) {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

/// `name` where cargo built this harness's library: `<target>/<profile>/`,
/// beside the `examples/` holding the harness (wherever CARGO_TARGET_DIR put it).
fn beside_the_examples(name: &str) -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|harness| Some(harness.parent()?.parent()?.join(name)))
        .unwrap_or_else(|| PathBuf::from(name))
}
