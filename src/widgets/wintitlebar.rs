use super::addeventsourcedlg::Msg as AddEventSourceDialogMsg;
use super::addeventsourcedlg::{
    AddEventSourceDialog, AddEventSourceDialogParams, EventSourceEditModel,
};
use gtk::prelude::*;
use relm::{init, Component, Widget};
use relm_derive::{widget, Msg};
use std::collections::{HashMap, HashSet};

#[derive(Msg)]
pub enum Msg {
    ScreenChanged,
    MainWindowStackReady(gtk::Stack),
    NewEventSourceClick,
    AddConfig(&'static str, String, HashMap<&'static str, String>),
    EventSourceNamesChanged(HashSet<String>),
}

pub struct Model {
    relm: relm::Relm<WinTitleBar>,
    displaying_event_sources: bool,
    main_window_stack: Option<gtk::Stack>,
    existing_source_names: HashSet<String>,
}

#[widget]
impl Widget for WinTitleBar {
    fn init_view(&mut self) {
        self.new_event_source_btn
            .get_style_context()
            .add_class("suggested-action");
    }

    fn model(relm: &relm::Relm<Self>, existing_source_names: HashSet<String>) -> Model {
        Model {
            relm: relm.clone(),
            displaying_event_sources: false,
            main_window_stack: None,
            existing_source_names,
        }
    }

    pub fn prepare_addedit_eventsource_dlg(
        main_win: &gtk::Window,
        existing_source_names: &HashSet<String>,
        edit_model: Option<EventSourceEditModel>,
    ) -> (gtk::Dialog, Component<AddEventSourceDialog>) {
        let dialog = gtk::DialogBuilder::new()
            .use_header_bar(1)
            .default_width(400)
            .default_height(250)
            .title(if edit_model.is_some() {
                "Edit event source"
            } else {
                "Add event source"
            })
            .transient_for(main_win)
            .build();
        let header_bar = dialog
            .get_header_bar()
            .unwrap()
            .dynamic_cast::<gtk::HeaderBar>()
            .unwrap();
        // i'm not using the 'official' dialog buttons,
        // because i've had problems with relm events
        // not propagating when using those. worked
        // fine when i started using my own buttons.
        let btn = gtk::Button::new_with_label("Next");
        btn.get_style_context().add_class("suggested-action");
        header_bar.pack_end(&btn);
        btn.show();
        let dialog_contents = init::<AddEventSourceDialog>(AddEventSourceDialogParams {
            existing_source_names: existing_source_names.clone(),
            next_btn: btn.clone(),
            dialog: dialog.clone(),
            edit_model,
        })
        .expect("error initializing the add event source modal");
        dialog
            .get_content_area()
            .pack_start(dialog_contents.widget(), true, true, 0);

        dialog.add_button("Cancel", gtk::ResponseType::Cancel);
        (dialog, dialog_contents)
    }

    fn run_event_source_addedit_dlg(&self) {
        let main_win = self
            .model
            .main_window_stack
            .as_ref()
            .unwrap()
            .get_toplevel()
            .and_then(|w| w.dynamic_cast::<gtk::Window>().ok());
        let (dialog, dialog_contents) = Self::prepare_addedit_eventsource_dlg(
            &main_win.unwrap(),
            &self.model.existing_source_names,
            None,
        );
        relm::connect!(dialog_contents@AddEventSourceDialogMsg::AddConfig(ref providername, ref name, ref cfg),
                               self.model.relm, Msg::AddConfig(providername, name.clone(), cfg.clone()));
        let resp = dialog.run();
        match resp {
            gtk::ResponseType::Cancel | gtk::ResponseType::DeleteEvent => dialog.destroy(),
            _ => {}
        }
    }

    fn update(&mut self, event: Msg) {
        match event {
            Msg::MainWindowStackReady(stack) => {
                self.model.main_window_stack = Some(stack.clone());
                self.main_window_stack_switcher
                    .set_stack(self.model.main_window_stack.as_ref());
                relm::connect!(
                    self.model.relm,
                    &stack,
                    connect_property_visible_child_name_notify(_),
                    Msg::ScreenChanged
                );
            }
            Msg::ScreenChanged => {
                self.model.displaying_event_sources = self
                    .model
                    .main_window_stack
                    .as_ref()
                    .unwrap()
                    .get_visible_child_name()
                    .as_ref()
                    .map(|s| s.as_str())
                    == Some("event-sources");
                self.header_bar.set_subtitle(
                    Some("Event Sources").filter(|_| self.model.displaying_event_sources),
                );
                self.new_event_source_btn
                    .set_visible(self.model.displaying_event_sources);
            }
            Msg::NewEventSourceClick => {
                self.run_event_source_addedit_dlg();
            }
            Msg::EventSourceNamesChanged(src) => {
                self.model.existing_source_names = src;
            }
            Msg::AddConfig(_, _, _) => {
                // this is meant for win... we emit here, not interested by it ourselves
            }
        }
    }

    view! {
        #[name="header_bar"]
        gtk::HeaderBar {
            #[name="new_event_source_btn"]
            gtk::Button {
                label: "New",
                visible:false,
                clicked() => Msg::NewEventSourceClick,
            },
            show_close_button: true,
            title: Some("Cigale"),
            #[name="main_window_stack_switcher"]
            gtk::StackSwitcher {
                child: {
                    pack_type: gtk::PackType::End
                }
            }
        }
    }
}
