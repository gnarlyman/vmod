use gtk4::prelude::*;
use gtk4::subclass::prelude::*;
use gtk4::glib;

#[derive(Debug, Default)]
pub struct PreferencesDialog {}

#[glib::object_subclass]
impl ObjectSubclass for PreferencesDialog {
    const NAME: &'static str = "PreferencesDialog";
    type Type = super::PreferencesDialog;
    type ParentType = gtk4::Window;
}

impl ObjectImpl for PreferencesDialog {
    fn constructed(&self) {
        self.parent_constructed();

        let obj = self.obj();
        obj.set_title(Some("Preferences"));
        obj.set_modal(true);
        obj.set_default_size(400, 300);

        // Create main vertical box
        let vbox = gtk4::Box::new(gtk4::Orientation::Vertical, 12);
        vbox.set_margin_top(20);
        vbox.set_margin_bottom(20);
        vbox.set_margin_start(20);
        vbox.set_margin_end(20);

        // Add a label with placeholder text
        let label = gtk4::Label::new(Some("Preferences will be implemented in Phase 2"));
        label.set_vexpand(true);
        vbox.append(&label);

        // Add close button
        let button = gtk4::Button::with_label("Close");
        button.connect_clicked(glib::clone!(
            #[weak]
            obj,
            move |_| {
                obj.close();
            }
        ));
        vbox.append(&button);

        obj.set_child(Some(&vbox));
    }
}

impl WidgetImpl for PreferencesDialog {}
impl WindowImpl for PreferencesDialog {}
