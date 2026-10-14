use relm4::gtk;
use relm4::gtk::prelude::*;
use relm4::prelude::*;

struct AppModel;

#[derive(Debug)]
enum AppMsg {}

#[relm4::component]
impl SimpleComponent for AppModel {
    type Init = ();
    type Input = AppMsg;
    type Output = ();

    view! {
        #[root]
        gtk::ApplicationWindow {
            set_title: Some("mimo_manager"),
            set_default_size: (900, 600),
            set_decorated: false,

            gtk::Box {
                set_orientation: gtk::Orientation::Vertical,

                gtk::Label {
                    set_label: "mimo_manager — Milestone 1 skeleton",
                    set_vexpand: true,
                },

                gtk::Box {
                    set_orientation: gtk::Orientation::Horizontal,
                    set_spacing: 12,

                    gtk::Label {
                        set_label: "NORMAL",
                    },
                    gtk::Label {
                        set_label: "MIME TYPES",
                    },
                    gtk::Label {
                        set_label: "0 ITEMS",
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
        let model = AppModel;
        let widgets = view_output!();
        ComponentParts { model, widgets }
    }

    fn update(&mut self, message: Self::Input, _sender: ComponentSender<Self>) {
        match message {}
    }
}

fn main() {
    let app = RelmApp::new("dev.mimo_manager.MimoManager");
    app.run::<AppModel>(());
}
