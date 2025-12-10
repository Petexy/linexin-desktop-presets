#!/usr/bin/env python3

import gi
import subprocess
import threading
import gettext
import locale
import os
import time

gi.require_version("Gtk", "4.0")
gi.require_version("Adw", "1")
from gi.repository import Gtk, Adw, GLib

# --- Localization Setup ---
APP_NAME = "linexin-desktop-presets"
LOCALE_DIR = os.path.abspath("/usr/share/locale")

# Set up the locale environment
try:
    locale.setlocale(locale.LC_ALL, '')
    locale.bindtextdomain(APP_NAME, LOCALE_DIR)
    gettext.bindtextdomain(APP_NAME, LOCALE_DIR)
    gettext.textdomain(APP_NAME)
except locale.Error as e:
    print(f"Warning: Could not set locale: {e}")
_ = gettext.gettext
# --------------------------

def is_plasma_session():
    """Check if the current desktop environment is KDE Plasma"""
    # Check common environment variables for Plasma
    desktop_session = os.environ.get('DESKTOP_SESSION', '').lower()
    xdg_current_desktop = os.environ.get('XDG_CURRENT_DESKTOP', '').lower()
    kde_session = os.environ.get('KDE_SESSION_VERSION', '')
    
    # Check if any indicator suggests Plasma
    plasma_indicators = ['plasma', 'kde']
    
    if kde_session:  # KDE_SESSION_VERSION is set
        return True
    
    if any(indicator in desktop_session for indicator in plasma_indicators):
        return True
    
    if any(indicator in xdg_current_desktop for indicator in plasma_indicators):
        return True
    
    return False

def is_gnome_session():
    """Check if the current desktop environment is GNOME"""
    desktop_session = os.environ.get('DESKTOP_SESSION', '').lower()
    xdg_current_desktop = os.environ.get('XDG_CURRENT_DESKTOP', '').lower()
    gdm_session = os.environ.get('GDMSESSION', '').lower()
    
    # Check if any indicator suggests GNOME
    gnome_indicators = ['gnome', 'ubuntu', 'pop']  # ubuntu and pop use GNOME
    
    if any(indicator in desktop_session for indicator in gnome_indicators):
        return True
    
    if any(indicator in xdg_current_desktop for indicator in gnome_indicators):
        return True
    
    if any(indicator in gdm_session for indicator in gnome_indicators):
        return True
    
    return False

class DesktopPresetsWidget(Gtk.Box):
    def __init__(self, hide_sidebar=False, window=None):
        super().__init__(orientation=Gtk.Orientation.VERTICAL, spacing=12)
        
        # Required: Widget display name
        self.widgetname = "Desktop Presets"
        
        # Optional: Widget icon
        self.widgeticon = "/usr/share/icons/customize.svg"
        
        # Widget content
        self.set_margin_top(12)
        self.set_margin_bottom(50)
        self.set_margin_start(12)
        self.set_margin_end(12)
        
        # Store window and sidebar settings
        self.window = window
        self.hide_sidebar = hide_sidebar
        
        # Check desktop environment and setup accordingly
        if is_plasma_session():
            self.setup_plasma_interface()
        elif is_gnome_session():
            self.setup_gnome_interface()
        else:
            # Fallback to GNOME interface for unknown DEs
            self.setup_gnome_interface()
        
        # Handle window resizing for single widget mode
        if self.hide_sidebar:
            GLib.idle_add(self.resize_window_deferred)
    
    def setup_gnome_interface(self):
        """Setup the full GNOME desktop presets interface"""
        # Initialize state variables
        self.install_started = False
        self.error_message = None
        self.current_product = None
        self.page_names = ["Linexin", "Winexin", "Ubunexin", "Pure GNOME"]
        
        # Setup paths
        home_dir = os.path.expanduser('~')
        self.script_base_path = os.path.join(home_dir, ".local/share/linexin/linexin-desktop/")
        
        self.monitor_script_path = os.path.join(self.script_base_path, "update-monitor.sh")
        self.script_paths = {
            "Linexin": os.path.join(self.script_base_path, "default.sh"),
            "Winexin": os.path.join(self.script_base_path, "windowish.sh"),
            "Ubunexin": os.path.join(self.script_base_path, "ubunexin.sh"),
            "Pure GNOME": os.path.join(self.script_base_path, "gnome.sh"),
        }

        # Setup UI
        self.setup_title()
        self.setup_carousel()
        self.setup_controls()

        # Initialize UI state
        self.update_ui_for_page_change()
    
    def setup_plasma_interface(self):
        """Setup the Plasma desktop presets interface"""
        # Initialize state variables for Plasma
        self.install_started = False
        self.error_message = None
        self.current_product = None
        self.page_names = ["Kinexin", "10ish", "11ish", "2worlds", "Plasmexin"]
        
        # Setup paths for Plasma scripts
        home_dir = os.path.expanduser('~')
        self.script_base_path = os.path.join(home_dir, ".local/share/linexin/linexin-desktop/plasma/")
        
        self.monitor_script_path = None
        self.script_paths = {
            "Kinexin": os.path.join(self.script_base_path, "style1.sh"),
            "10ish": os.path.join(self.script_base_path, "style2.sh"),
            "11ish": os.path.join(self.script_base_path, "style3.sh"),
            "2worlds": os.path.join(self.script_base_path, "style4.sh"),
            "Plasmexin": os.path.join(self.script_base_path, "style5.sh"),
        }

        # Setup UI
        self.setup_title()
        self.setup_carousel()
        self.setup_controls()

        # Initialize UI state
        self.update_ui_for_page_change()
    
    def resize_window_deferred(self):
        """Called after widget initialization to resize window safely"""
        if self.window:
            try:
                self.window.set_default_size(1200, 850)
                print("Window default size set to 1400x800")
            except Exception as e:
                print(f"Failed to resize window: {e}")
        return False

    def setup_title(self):
        """Setup the title section"""
        title = Gtk.Label(label=_("Select Your Design"))
        title.add_css_class("title-2")
        title.set_margin_bottom(6)
        self.append(title)
    
    def setup_carousel(self):
        """Setup the carousel with desktop previews"""
        # Navigation container
        nav_box = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=12)
        nav_box.set_halign(Gtk.Align.CENTER)
        nav_box.set_vexpand(True)
        
        # Previous button
        self.btn_prev = Gtk.Button.new_from_icon_name("go-previous-symbolic")
        self.btn_prev.set_valign(Gtk.Align.CENTER)
        self.btn_prev.connect("clicked", self.on_prev_clicked)
        nav_box.append(self.btn_prev)
        
        # Stack container for pages
        self.content_stack = Gtk.Stack()
        self.content_stack.set_transition_type(Gtk.StackTransitionType.SLIDE_LEFT_RIGHT)
        self.content_stack.set_hexpand(True)
        self.content_stack.set_vexpand(True)
        nav_box.append(self.content_stack)
        
        # Next button
        self.btn_next = Gtk.Button.new_from_icon_name("go-next-symbolic")
        self.btn_next.set_valign(Gtk.Align.CENTER)
        self.btn_next.connect("clicked", self.on_next_clicked)
        nav_box.append(self.btn_next)
        
        # Setup pages with images
        image_paths = []
        if is_plasma_session():
            # Plasma image paths
            image_paths = [
                os.path.join(self.script_base_path, "style1.png"),
                os.path.join(self.script_base_path, "style2.png"),
                os.path.join(self.script_base_path, "style3.png"),
                os.path.join(self.script_base_path, "style4.png"),
                os.path.join(self.script_base_path, "style5.png")
            ]
        else:
            # GNOME image paths
            image_paths = [
                os.path.join(self.script_base_path, "default.png"),
                os.path.join(self.script_base_path, "windowish.png"),
                os.path.join(self.script_base_path, "ubunexin.png"),
                os.path.join(self.script_base_path, "gnome.png")
            ]
        
        for i, page_name in enumerate(self.page_names):
            image_path = image_paths[i]
            
            # Debug output
            print(f"Looking for image: {image_path}")
            print(f"Image exists: {os.path.exists(image_path)}")
            
            # Page container
            page_container = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=8)
            page_container.set_valign(Gtk.Align.CENTER)
            page_container.set_halign(Gtk.Align.CENTER)
            
            # Preview image or placeholder
            if os.path.exists(image_path):
                print(f"Loading image: {image_path}")
                picture = Gtk.Picture()
                picture.set_keep_aspect_ratio(True)
                picture.set_can_shrink(True)
                picture.set_content_fit(Gtk.ContentFit.SCALE_DOWN)
                picture.set_size_request(300, 200)
                picture.set_filename(image_path)
                page_container.append(picture)
            else:
                print(f"Image not found, showing placeholder for: {image_path}")
                # Fallback: use a simple placeholder box
                fallback = Gtk.Box()
                fallback.set_size_request(300, 200)
                fallback.add_css_class("card")
                
                # Add a label to show it's a placeholder
                placeholder_label = Gtk.Label(label="Preview not available")
                placeholder_label.add_css_class("dim-label")
                fallback.append(placeholder_label)
                fallback.set_halign(Gtk.Align.CENTER)
                fallback.set_valign(Gtk.Align.CENTER)
                
                page_container.append(fallback)
            
            # Style name label
            label = Gtk.Label(label=page_name)
            label.add_css_class("title-3")
            page_container.append(label)
            
            self.content_stack.add_named(page_container, page_name)
        
        # Set initial page
        self.content_stack.set_visible_child_name(self.page_names[0])
        self.content_stack.connect("notify::visible-child", self.update_ui_for_page_change)
        
        self.append(nav_box)
    
    def setup_controls(self):
        """Setup control buttons"""
        controls_box = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=6)
        controls_box.set_halign(Gtk.Align.CENTER)
        controls_box.set_margin_top(6)
        
        self.btn_apply = Gtk.Button(label=_("Apply Style"))
        self.btn_apply.add_css_class("suggested-action")
        self.btn_apply.add_css_class("buttons_all")
        self.btn_apply.connect("clicked", self.on_apply_clicked)
        
        controls_box.append(self.btn_apply)
        self.append(controls_box)
    
    def on_prev_clicked(self, button):
        """Handle previous button click"""
        current_name = self.content_stack.get_visible_child_name()
        if not current_name or current_name not in self.page_names:
            return
        current_index = self.page_names.index(current_name)
        if current_index > 0:
            self.content_stack.set_visible_child_name(self.page_names[current_index - 1])
    
    def on_next_clicked(self, button):
        """Handle next button click"""
        current_name = self.content_stack.get_visible_child_name()
        if not current_name or current_name not in self.page_names:
            return
        current_index = self.page_names.index(current_name)
        if current_index < len(self.page_names) - 1:
            self.content_stack.set_visible_child_name(self.page_names[current_index + 1])
    
    def update_ui_for_page_change(self, *args):
        """Update navigation button states"""
        current_name = self.content_stack.get_visible_child_name()
        if current_name not in self.page_names:
            return
            
        current_index = self.page_names.index(current_name)
        
        if not self.install_started:
            self.btn_prev.set_sensitive(current_index > 0)
            self.btn_next.set_sensitive(current_index < len(self.page_names) - 1)
            self.btn_prev.set_opacity(1.0 if current_index > 0 else 0.3)
            self.btn_next.set_opacity(1.0 if current_index < len(self.page_names) - 1 else 0.3)
    
    def on_apply_clicked(self, button):
        """Handle apply button click"""
        selected_desktop = self.content_stack.get_visible_child_name()
        script_to_run = self.script_paths.get(selected_desktop)
        
        # Validate scripts
        if not script_to_run or not os.path.exists(script_to_run) or not os.access(script_to_run, os.X_OK):
            self.show_error_dialog(_("Error"), _("Main script not found or is not executable:\n{}").format(script_to_run))
            return
        
        # Only validate monitor script if it's needed (GNOME)
        if self.monitor_script_path and (not os.path.exists(self.monitor_script_path) or not os.access(self.monitor_script_path, os.X_OK)):
            self.show_error_dialog(_("Error"), _("Monitor script not found or is not executable:\n{}").format(self.monitor_script_path))
            return
        
        # Begin installation
        self.begin_install(script_to_run, selected_desktop)
    
    def begin_install(self, script_path, product_name):
        """Start the installation process"""
        self.error_message = None
        self.install_started = True
        
        # Disable UI elements
        self.btn_prev.set_sensitive(False)
        self.btn_next.set_sensitive(False)
        self.btn_prev.set_opacity(0.3)
        self.btn_next.set_opacity(0.3)
        self.btn_apply.set_sensitive(False)
        
        self.btn_apply.set_label(_("Updating display..."))
        self.current_product = product_name
        
        # Skip monitor script for Plasma (when monitor_script_path is None)
        if self.monitor_script_path is None:
            self.btn_apply.set_label(_("Applying settings..."))
            threading.Thread(target=self.task_run_main_script, args=(script_path,), daemon=True).start()
        else:
            self.btn_apply.set_label(_("Updating display..."))
            threading.Thread(target=self.task_run_monitor_script, args=(script_path,), daemon=True).start()
    
    def task_run_monitor_script(self, main_script_path_for_next_step):
        """Execute the monitor update script"""
        try:
            command = ['/bin/bash', self.monitor_script_path]
            process = subprocess.Popen(command, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
            stdout, stderr = process.communicate()
            
            if process.returncode != 0:
                self.error_message = _("Monitor script failed with code {0}.\n\nDetails: {1}").format(process.returncode, stderr.strip())
        
        except Exception as e:
            self.error_message = _("An error occurred during monitor update: {}").format(str(e))
        
        GLib.idle_add(self.on_monitor_script_done, main_script_path_for_next_step)
    
    def on_monitor_script_done(self, main_script_path):
        """Handle monitor script completion"""
        if self.error_message:
            self.finalize_ui()
        else:
            self.btn_apply.set_label(_("Applying settings..."))
            threading.Thread(target=self.task_run_main_script, args=(main_script_path,), daemon=True).start()
        
        return GLib.SOURCE_REMOVE
    
    def task_run_main_script(self, script_path):
        """Execute the main desktop preset script twice with delay"""
        try:
            # First run
            time.sleep(1)
            command = ['/bin/bash', script_path]
            process = subprocess.Popen(command, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
            stdout, stderr = process.communicate()
            
            if process.returncode != 0:
                self.error_message = _("Main script (first run) failed with code {0}.\n\nDetails: {1}").format(process.returncode, stderr.strip())
                GLib.idle_add(self.finalize_ui)
                return
            
            # Update UI for second run
            GLib.idle_add(lambda: self.btn_apply.set_label(_("Applying settings...")))
            
            # Second run with delay
            time.sleep(2)
            process = subprocess.Popen(command, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
            stdout, stderr = process.communicate()
            
            if process.returncode != 0:
                self.error_message = _("Main script (second run) failed with code {0}.\n\nDetails: {1}").format(process.returncode, stderr.strip())
        
        except Exception as e:
            self.error_message = _("An unexpected error occurred while running the main script: {}").format(str(e))
        
        GLib.idle_add(self.finalize_ui)
    
    def finalize_ui(self):
        """Restore UI after operation completion"""
        self.install_started = False
        
        self.btn_apply.set_sensitive(True)
        self.btn_apply.set_label(_("Apply Style"))
        
        self.update_ui_for_page_change()
        
        if self.error_message:
            dialog_heading = _("Operation Failed")
            dialog_body = _("Could not apply style for {}.\n\nError: {}").format(self.current_product, self.error_message)
            self.show_error_dialog(dialog_heading, dialog_body)
        else:
            # Always show logout dialog after successful style application
            self.show_logout_dialog()
        
        return GLib.SOURCE_REMOVE
    
    def show_error_dialog(self, heading, body):
        """Show error dialog"""
        # Get the toplevel window to use as parent
        parent_window = self.get_root()
        
        dialog = Adw.MessageDialog(
            heading=heading,
            body=body,
            transient_for=parent_window,
            modal=True
        )
        dialog.add_response("ok", _("OK"))
        dialog.set_default_response("ok")
        dialog.set_close_response("ok")
        dialog.present()

    def show_logout_dialog(self):
        """Show logout prompt dialog"""
        parent_window = self.get_root()
        
        # Use the template format that matches the dictionary
        body_template = "To complete the change to {}, you need to log out and log back in again."
        # Format it with the actual product name
        body = body_template.format(self.current_product)
        
        dialog = Adw.MessageDialog(
            heading="Restart Required",
            body=body,
            transient_for=parent_window,
            modal=True
        )
        dialog.add_response("later", "Later")
        dialog.add_response("logout", "Log Out Now")
        dialog.set_response_appearance("logout", Adw.ResponseAppearance.SUGGESTED)
        dialog.set_default_response("later")
        dialog.set_close_response("later")
        
        dialog.connect("response", self.on_logout_dialog_response)
        dialog.present()

    def on_logout_dialog_response(self, dialog, response):
        """Handle logout dialog response"""
        if response == "logout":
            # Give a brief moment for any pending operations to complete
            def delayed_logout():
                time.sleep(0.5)
                # Execute logout command based on desktop environment
                if is_plasma_session():
                    # KDE Plasma logout command - try multiple methods
                    try:
                        # Method 1: Using qdbus6 (for Plasma 6)
                        subprocess.run(['qdbus6', 'org.kde.Shutdown', '/Shutdown', 'logout'], check=False)
                    except FileNotFoundError:
                        try:
                            # Method 2: Using qdbus (for Plasma 5)
                            subprocess.run(['qdbus', 'org.kde.ksmserver', '/KSMServer', 'logout', '0', '0', '0'], check=False)
                        except FileNotFoundError:
                            # Method 3: Using loginctl
                            subprocess.run(['loginctl', 'terminate-user', os.environ.get('USER', '')], check=False)
                else:
                    # GNOME logout command
                    subprocess.Popen(['gnome-session-quit', '--logout', '--no-prompt'])
            
            threading.Thread(target=delayed_logout, daemon=True).start()