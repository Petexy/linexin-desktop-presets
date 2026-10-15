//! Native port of the Desktop Presets widget (`b-desktop_presets.py`),
//! installed as `/usr/share/linexin/widgets/b-desktop_presets.so`.
//!
//! GTK only: it switches Plasma and GNOME layouts, so it has no LineXinBar
//! page and the Center does not list it on LineXinBar sessions.

mod process;
mod pyformat;
mod session;
mod ui;

use std::ffi::c_void;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::ptr;

use gtk::glib::translate::{FromGlibPtrNone, IntoGlibPtr};
use gtk::prelude::*;
use linexin_plugin_api as api;

static WIDGET: api::WidgetDescriptorV1 = api::WidgetDescriptorV1::new(
    c"desktop_presets",
    c"Desktop Presets",
    c"/usr/share/icons/customize.svg",
    Some(create),
);
api::export_widget!(WIDGET);

unsafe extern "C" fn create(context: *const api::CreateContextV1) -> *mut c_void {
    let widget = catch_unwind(AssertUnwindSafe(|| {
        // SAFETY: `context` is the pointer the host passed to `create`.
        let context = unsafe { api::CreateContextV1::read(context) }?;
        if let Err(error) = adw::init() {
            eprintln!("Desktop Presets: could not initialize GTK: {error}");
            return None;
        }
        let window = (!context.window.is_null())
            .then(|| {
                // SAFETY: the host passes a live GtkWindow*; a new reference is taken.
                let object = unsafe {
                    gtk::glib::Object::from_glib_none(
                        context.window.cast::<gtk::glib::gobject_ffi::GObject>(),
                    )
                };
                object.downcast::<gtk::Window>().ok()
            })
            .flatten();
        let root = ui::build(context.hide_sidebar, window).upcast::<gtk::Widget>();
        let widget: *mut gtk::ffi::GtkWidget = root.into_glib_ptr();
        Some(widget.cast::<c_void>())
    }));
    match widget {
        Ok(Some(widget)) => widget,
        Ok(None) => ptr::null_mut(),
        Err(_) => {
            eprintln!("Desktop Presets: widget creation panicked");
            ptr::null_mut()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CStr;

    #[test]
    fn descriptor_matches_the_python_widget() {
        assert_eq!(WIDGET.abi_version, api::ABI_VERSION);
        assert_eq!(
            WIDGET.struct_size as usize,
            size_of::<api::WidgetDescriptorV1>()
        );
        let text = |pointer| unsafe { CStr::from_ptr(pointer) }.to_str().unwrap();
        assert_eq!(text(WIDGET.widget_id), ui::DOMAIN);
        assert_eq!(text(WIDGET.name), "Desktop Presets");
        assert_eq!(text(WIDGET.icon), "/usr/share/icons/customize.svg");
        assert!(WIDGET.create.is_some());
        // GTK only: no LineXinBar page.
        assert_eq!(text(WIDGET.lxb_mark), "");
        assert!(WIDGET.lxb_create.is_none() && WIDGET.lxb_page.is_none());
        assert!(WIDGET.lxb_destroy.is_none());
    }

    #[test]
    fn entry_point_rejects_a_missing_host() {
        assert!(unsafe { linexin_widget_v1(ptr::null()) }.is_null());
        assert!(unsafe { create(ptr::null()) }.is_null());
    }
}
