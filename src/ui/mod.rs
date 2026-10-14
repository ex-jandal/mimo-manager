//! GTK4 + Relm4 user interface.

use relm4::gtk::prelude::*;
use relm4::prelude::*;

struct AppModel {
    items: usize,
}

#[derive(Debug)]
enum AppMsg {}

#[relm4::component]
impl SimpleComponent for AppModel {
    type Init = ();
    type Input = AppMsg;
    type Output = ();

    view! {
        gtk::ApplicationWindow {
            set_title: Some("mimo_manager"),
            set_default_size: (900, 600),
            set_decorated: false,

            gtk::Box {
                set_orientation: gtk::Orientation::Vertical,

                gtk::Label {
                    set_hexpand: true,
                    set_vexpand: true,
                    set_label: "No items",
                },

                gtk::Box {
                    add_css_class: "modeline",
                    gtk::Label { set_label: "NORMAL" },
                    gtk::Label { set_label: "MIME TYPES" },
                    #[watch]
                    set_tooltip_text: Some(&format!("{} items", model.items)),
                    gtk::Label {
                        #[watch]
                        set_label: &format!("{} ITEMS", model.items),
                    },
                },
            },
        }
    }

    fn init(
        _init: Self::Init,
        root: Self::Root,
        _sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let model = AppModel { items: 0 };
        let widgets = view_output!();
        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, _sender: ComponentSender<Self>) {
        match msg {}
    }
}

/// Run the application.
pub fn run() {
    let app = RelmApp::new("dev.mimo_manager.MimoManager");
    app.run::<AppModel>(());
}
