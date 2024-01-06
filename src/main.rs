use gettextrs::*;
use std::thread;

use adw::{
    prelude::{ActionRowExt, MessageDialogExt, PreferencesRowExt},
    ActionRow, Application, ToastOverlay,
};
use gtk::prelude::*;
use gtk::{
    glib::{self},
    Align, ApplicationWindow, Notebook, Orientation, PositionType,
};

mod distrobox_handler;
use distrobox_handler::*;

mod utils;
use utils::{
    get_distro_img, get_supported_terminals_list, get_terminal_and_separator_arg,
    has_distrobox_installed, set_up_localisation,
};

const APP_ID: &str = "io.github.dvlv.boxbuddyrs";

enum AppsFetchMessage {
    AppsFetched(Vec<DBoxApp>),
}

enum BoxCreatedMessage {
    Success,
}

fn main() -> glib::ExitCode {
    set_up_localisation();

    // Create a new application
    let app = Application::builder().application_id(APP_ID).build();

    // Connect to "activate" signal of `app`
    app.connect_activate(build_ui);

    // Run the application
    app.run()
}

fn build_ui(app: &Application) {
    // Create a window and set the title
    let window = ApplicationWindow::builder()
        .application(app)
        .title("BoxBuddy")
        .build();

    window.set_default_size(800, 450);

    make_titlebar(&window);

    let toast_overlay = ToastOverlay::new();
    let main_box = gtk::Box::new(Orientation::Vertical, 10);
    main_box.set_orientation(Orientation::Vertical);
    main_box.set_hexpand(true);
    main_box.set_vexpand(true);
    main_box.set_margin_top(10);
    main_box.set_margin_bottom(10);
    main_box.set_margin_start(10);
    main_box.set_margin_end(10);

    toast_overlay.set_child(Some(&main_box));
    window.set_child(Some(&toast_overlay));

    if has_distrobox_installed() {
        load_boxes(&main_box, &window);
    } else {
        render_not_installed(&main_box);
    }

    window.present();

    let (term, _) = get_terminal_and_separator_arg();
    if term.is_empty() {
        show_no_supported_terminal_popup(&window);
    }
}

fn make_titlebar(window: &ApplicationWindow) {
    let add_btn = gtk::Button::from_icon_name("list-add-symbolic");
    add_btn.set_tooltip_text(Some(&gettext("Create A Distrobox")));

    let win_clone = window.clone();
    add_btn.connect_clicked(move |_btn| create_new_distrobox(&win_clone));

    let about_btn = gtk::Button::from_icon_name("help-about-symbolic");
    about_btn.set_tooltip_text(Some(&gettext("About BoxBuddy")));

    let win_clone = window.clone();
    about_btn.connect_clicked(move |_btn| show_about_popup(&win_clone));

    let title_lbl = gtk::Label::new(Some("BoxBuddy"));
    title_lbl.add_css_class("header");

    let titlebar = adw::HeaderBar::builder().title_widget(&title_lbl).build();

    titlebar.pack_start(&add_btn);
    titlebar.pack_end(&about_btn);

    window.set_titlebar(Some(&titlebar))
}

fn render_not_installed(main_box: &gtk::Box) {
    let not_installed_lbl = gtk::Label::new(Some(&gettext("Distrobox not found!")));
    not_installed_lbl.add_css_class("title-1");

    let not_installed_lbl_two = gtk::Label::new(Some(&gettext(
        "Distrobox could not be found, please ensure it is installed!",
    )));
    not_installed_lbl_two.add_css_class("title-2");

    main_box.append(&not_installed_lbl);
    main_box.append(&not_installed_lbl_two);
}

fn load_boxes(main_box: &gtk::Box, window: &ApplicationWindow) {
    let tabs = Notebook::new();
    tabs.set_tab_pos(PositionType::Left);
    tabs.set_hexpand(true);
    tabs.set_vexpand(true);

    let boxes = get_all_distroboxes();

    if boxes.is_empty() {
        render_no_boxes_message(&main_box);
        return;
    }

    for dbox in boxes.iter() {
        let tab = make_box_tab(dbox, window);
        // TODO shouldnt this be in make_box_tab
        tab.set_hexpand(true);
        tab.set_vexpand(true);

        let tab_title = gtk::Box::new(Orientation::Horizontal, 5);
        let tab_title_lbl = gtk::Label::new(Some(&dbox.name));
        let tab_title_img = gtk::Label::new(None);
        tab_title_img.set_markup(&get_distro_img(&dbox.distro));

        tab_title.append(&tab_title_img);
        tab_title.append(&tab_title_lbl);

        tabs.append_page(&tab, Some(&tab_title));
    }

    while let Some(child) = main_box.first_child() {
        main_box.remove(&child);
    }

    main_box.append(&tabs);
}

fn make_box_tab(dbox: &DBox, window: &ApplicationWindow) -> gtk::Box {
    let box_name = dbox.name.clone();

    let tab_box = gtk::Box::new(Orientation::Vertical, 15);
    tab_box.set_hexpand(true);

    tab_box.set_margin_top(10);
    tab_box.set_margin_bottom(10);
    tab_box.set_margin_start(10);
    tab_box.set_margin_end(10);

    //title
    let page_img = gtk::Label::new(None);
    page_img.set_markup(&get_distro_img(&dbox.distro));
    let page_title = gtk::Label::new(Some(&dbox.name));
    page_title.add_css_class("title-1");

    let page_status = gtk::Label::new(Some(&dbox.status));
    page_status.set_halign(Align::End);
    page_status.set_hexpand(true);

    let title_box = gtk::Box::new(Orientation::Horizontal, 10);
    title_box.set_margin_start(10);
    title_box.append(&page_img);
    title_box.append(&page_title);
    title_box.append(&page_status);

    // list view
    let boxed_list = gtk::ListBox::new();
    boxed_list.add_css_class("boxed-list");

    // terminal button
    let open_terminal_button = gtk::Button::from_icon_name("utilities-terminal-symbolic");
    open_terminal_button.add_css_class("flat");

    let term_bn_clone = box_name.clone();
    open_terminal_button
        .connect_clicked(move |_btn| on_open_terminal_clicked(term_bn_clone.clone()));

    let open_terminal_row = ActionRow::new();
    open_terminal_row.set_title(&gettext("Open Terminal"));
    open_terminal_row.add_suffix(&open_terminal_button);
    open_terminal_row.set_activatable_widget(Some(&open_terminal_button));

    // upgrade button
    let upgrade_button = gtk::Button::from_icon_name("software-update-available-symbolic");
    upgrade_button.add_css_class("flat");

    let up_bn_clone = box_name.clone();
    upgrade_button.connect_clicked(move |_btn| on_upgrade_clicked(up_bn_clone.clone()));

    let upgrade_row = ActionRow::new();
    upgrade_row.set_title(&gettext("Upgrade Box"));
    upgrade_row.add_suffix(&upgrade_button);
    upgrade_row.set_activatable_widget(Some(&upgrade_button));

    // show applications button
    let show_applications_button = gtk::Button::from_icon_name("application-x-executable-symbolic");
    show_applications_button.add_css_class("flat");

    let show_bn_clone = box_name.clone();
    let win_clone = window.clone();
    show_applications_button.connect_clicked(move |_btn| {
        on_show_applications_clicked(&win_clone, show_bn_clone.clone())
    });

    let show_applications_row = ActionRow::new();
    show_applications_row.set_title(&gettext("View Applications"));
    show_applications_row.add_suffix(&show_applications_button);
    show_applications_row.set_activatable_widget(Some(&show_applications_button));

    // Delete Button
    let delete_button = gtk::Button::from_icon_name("user-trash-symbolic");
    delete_button.add_css_class("flat");

    let del_bn_clone = box_name.clone();
    let win_clone = window.clone();
    delete_button.connect_clicked(move |_btn| on_delete_clicked(&win_clone, del_bn_clone.clone()));

    let delete_row = ActionRow::new();
    delete_row.set_title(&gettext("Delete Box"));
    delete_row.add_suffix(&delete_button);
    delete_row.set_activatable_widget(Some(&delete_button));

    // put all into list
    boxed_list.append(&open_terminal_row);
    boxed_list.append(&upgrade_row);
    boxed_list.append(&show_applications_row);
    boxed_list.append(&delete_row);

    tab_box.append(&title_box);
    tab_box.append(&gtk::Separator::new(Orientation::Horizontal));
    tab_box.append(&boxed_list);

    tab_box
}

// callbacks
fn create_new_distrobox(window: &ApplicationWindow) {
    let new_box_popup = gtk::Window::new();
    new_box_popup.set_transient_for(Some(window));
    new_box_popup.set_default_size(700, 350);
    new_box_popup.set_modal(true);

    let title_lbl = gtk::Label::new(Some(&gettext("Create New Distrobox")));
    title_lbl.add_css_class("header");

    let create_btn = gtk::Button::with_label(&gettext("Create"));
    create_btn.add_css_class("suggested-action");

    let cancel_btn = gtk::Button::with_label(&gettext("Cancel"));

    cancel_btn.connect_clicked(move |btn| {
        let win = btn.root().and_downcast::<gtk::Window>().unwrap();
        win.destroy();
    });

    let new_box_titlebar = adw::HeaderBar::builder().title_widget(&title_lbl).build();

    new_box_titlebar.pack_end(&create_btn);
    new_box_titlebar.pack_start(&cancel_btn);

    new_box_popup.set_titlebar(Some(&new_box_titlebar));

    let main_box = gtk::Box::new(Orientation::Vertical, 10);
    main_box.set_margin_start(10);
    main_box.set_margin_end(10);
    main_box.set_margin_top(10);
    main_box.set_margin_bottom(10);

    let boxed_list = gtk::ListBox::new();
    boxed_list.add_css_class("boxed-list");

    // name input
    let name_entry_row = adw::EntryRow::new();
    name_entry_row.set_hexpand(true);
    name_entry_row.set_title(&gettext("Name"));

    // Image
    let available_images = get_available_images_with_distro_name();
    let avail_images_as_ref: Vec<&str> = available_images.iter().map(|s| s as &str).collect();
    let imgs_strlist = gtk::StringList::new(avail_images_as_ref.as_slice());

    let exp = gtk::Expression::NONE;

    let image_select = gtk::DropDown::new(Some(imgs_strlist), exp);

    let image_select_row = adw::ActionRow::new();
    image_select_row.set_title(&gettext("Image"));
    image_select_row.set_activatable_widget(Some(&image_select));
    image_select_row.add_suffix(&image_select);

    let loading_spinner = gtk::Spinner::new();

    let ne_row = name_entry_row.clone();
    let is_row = image_select_row.clone();
    let loading_spinner_clone = loading_spinner.clone();
    let win_clone = window.clone();
    create_btn.connect_clicked(move |btn| {
        loading_spinner_clone.start();
        let mut name = ne_row.text().to_string();
        let mut image = is_row
            .activatable_widget()
            .and_downcast::<gtk::DropDown>()
            .unwrap()
            .selected_item()
            .unwrap()
            .downcast::<gtk::StringObject>()
            .unwrap()
            .string()
            .to_string();

        if name.is_empty() || image.is_empty() {
            return;
        }

        name = name.replace(' ', "-");
        image = image.split(' ').last().unwrap().to_string();

        let name_clone = name.clone();

        let (sender, receiver) =
            glib::MainContext::channel::<BoxCreatedMessage>(glib::Priority::DEFAULT);

        thread::spawn(move || {
            create_box(name, image);
            sender.send(BoxCreatedMessage::Success).unwrap();
        });

        let b_clone = btn.clone();
        let ls_clone = loading_spinner_clone.clone();
        let w_clone = win_clone.clone();
        receiver.attach(None, move |msg| match msg {
            BoxCreatedMessage::Success => {
                ls_clone.stop();

                let win = b_clone.root().and_downcast::<gtk::Window>().unwrap();
                win.destroy();
                delayed_rerender(&w_clone);

                open_terminal_in_box(name_clone.clone());

                glib::ControlFlow::Continue
            }
        });
    });

    boxed_list.append(&name_entry_row);
    boxed_list.append(&image_select_row);

    main_box.append(&boxed_list);
    main_box.append(&loading_spinner);

    new_box_popup.set_child(Some(&main_box));
    new_box_popup.present();
}

fn show_about_popup(window: &ApplicationWindow) {
    let d = adw::AboutWindow::new();
    d.set_transient_for(Some(window));
    d.set_application_name("BoxBuddy");
    d.set_version("1.0.6");
    d.set_developer_name("Dvlv");
    d.set_license_type(gtk::License::MitX11);
    d.set_comments(
        "A Graphical Manager for your Distroboxes.
    \nBoxBuddy is not partnered with or endorsed by any linux distributions or companies.
    \nTrademarks, service marks, and logos are the property of their respective owners.",
    );
    d.set_website("https://github.com/Dvlv/BoxBuddyRS");
    d.add_credit_section(Some("Contributors"), &["Dvlv"]);
    d.set_developers(&["Dvlv"]);
    d.set_application_icon("io.github.dvlv.boxbuddyrs");
    d.present();
}

fn on_open_terminal_clicked(box_name: String) {
    open_terminal_in_box(box_name);
}

fn on_upgrade_clicked(box_name: String) {
    upgrade_box(box_name)
}

fn on_show_applications_clicked(window: &ApplicationWindow, box_name: String) {
    let apps_popup = gtk::Window::new();
    apps_popup.set_transient_for(Some(window));
    apps_popup.set_default_size(700, 350);
    apps_popup.set_modal(true);
    apps_popup.set_title(Some(&gettext("Installed Applications")));

    let main_box = gtk::Box::new(Orientation::Vertical, 10);
    main_box.set_margin_start(10);
    main_box.set_margin_end(10);
    main_box.set_margin_top(10);
    main_box.set_margin_bottom(10);

    let loading_spinner = gtk::Spinner::new();
    let loading_lbl = gtk::Label::new(Some(&gettext("Loading...")));
    loading_lbl.add_css_class("title-2");

    main_box.append(&loading_lbl);
    main_box.append(&loading_spinner);

    apps_popup.set_child(Some(&main_box));
    loading_spinner.start();
    apps_popup.present();
    apps_popup.queue_draw();

    let (sender, receiver) =
        glib::MainContext::channel::<AppsFetchMessage>(glib::Priority::DEFAULT);
    let box_name_clone = box_name.clone();

    // fetch these in background thread so we can render the window with loading message
    // Massive thanks to https://coaxion.net/blog/2019/02/mpsc-channel-api-for-painless-usage-of-threads-with-gtk-in-rust/
    thread::spawn(move || {
        let apps = get_apps_in_box(box_name_clone.clone());

        sender.send(AppsFetchMessage::AppsFetched(apps)).unwrap();
    });

    receiver.attach(None, move |msg| {
        match msg {
            AppsFetchMessage::AppsFetched(apps) => {
                loading_spinner.stop();

                if apps.is_empty() {
                    let no_apps_lbl = gtk::Label::new(Some(&gettext("No Applications Installed")));
                    no_apps_lbl.add_css_class("title-2");
                    main_box.append(&no_apps_lbl);
                } else {
                    loading_lbl.set_text(&gettext("Available Applications"));

                    let boxed_list = gtk::ListBox::new();
                    boxed_list.add_css_class("boxed-list");

                    for app in apps {
                        let row = adw::ActionRow::new();
                        row.set_title(&app.name);

                        let img = gtk::Image::from_icon_name(&app.icon);

                        let add_menu_btn = gtk::Button::with_label(&gettext("Add To Menu"));
                        add_menu_btn.add_css_class("pill");

                        let box_name_clone = box_name.clone();
                        let loading_lbl_clone = loading_lbl.clone();
                        let app_clone = app.clone();
                        add_menu_btn.connect_clicked(move |_btn| {
                            add_app_to_menu(
                                &app_clone,
                                box_name_clone.clone(),
                                &loading_lbl_clone.clone(),
                            );
                        });

                        let run_btn = gtk::Button::with_label(&gettext("Run"));
                        run_btn.add_css_class("pill");
                        // todo connect
                        let box_name_clone = box_name.clone();
                        let app_clone = app.clone();
                        run_btn.connect_clicked(move |_btn| {
                            run_app_in_box(&app_clone, box_name_clone.clone());
                        });

                        row.add_prefix(&img);
                        row.add_suffix(&run_btn);
                        row.add_suffix(&gtk::Separator::new(gtk::Orientation::Horizontal));
                        row.add_suffix(&add_menu_btn);

                        boxed_list.append(&row);

                        main_box.append(&boxed_list);
                    }
                }
            }
        }

        glib::ControlFlow::Continue
    });
}

fn add_app_to_menu(app: &DBoxApp, box_name: String, success_lbl: &gtk::Label) {
    let _ = export_app_from_box(app.name.to_string(), box_name);
    success_lbl.set_text(&gettext("App Exported!"));
}

fn run_app_in_box(app: &DBoxApp, box_name: String) {
    run_command_in_box(app.exec_name.to_string(), box_name);
}

fn on_delete_clicked(window: &ApplicationWindow, box_name: String) {
    let are_you_sure_pre = &gettext("Are you sure you want to delete ");
    let d = adw::MessageDialog::new(
        Some(window),
        Some(&gettext("Really Delete?")),
        Some(&format!("{are_you_sure_pre} {box_name}?")),
    );
    d.set_transient_for(Some(window));
    d.add_response("cancel", &gettext("Cancel"));
    d.add_response("delete", &gettext("Delete"));
    d.set_default_response(Some("cancel"));
    d.set_close_response("cancel");
    d.set_response_appearance("delete", adw::ResponseAppearance::Destructive);

    let win_clone = window.clone();

    d.connect_response(None, move |d, res| {
        if res == "delete" {
            delete_box(box_name.clone());
            d.destroy();

            let toast = adw::Toast::new(&gettext("Box Deleted!"));
            if let Some(child) = win_clone.clone().child() {
                let toast_area = child.downcast::<ToastOverlay>();
                toast_area.unwrap().add_toast(toast);
            }

            delayed_rerender(&win_clone);
        }
    });

    d.present()
}

fn delayed_rerender(window: &ApplicationWindow) {
    let main_box = window.child().unwrap().first_child().unwrap();
    let main_box_as_box = main_box.downcast::<gtk::Box>().unwrap();

    load_boxes(&main_box_as_box, window);
}

fn show_no_supported_terminal_popup(window: &ApplicationWindow) {
    let supported_terminals = get_supported_terminals_list();
    let supported_terminals_pre = &gettext("Please install one of the supported terminals:");
    let supported_terminals_body = format!("{supported_terminals_pre}\n\n{supported_terminals}");
    let d = adw::MessageDialog::new(
        Some(window),
        Some(&gettext("No supported terminal found")),
        Some(&supported_terminals_body),
    );
    d.set_transient_for(Some(window));
    d.add_response("ok", &gettext("Ok"));
    d.set_default_response(Some("ok"));
    d.set_close_response("ok");

    d.present();
}

fn render_no_boxes_message(main_box: &gtk::Box) {
    while let Some(child) = main_box.first_child() {
        main_box.remove(&child);
    }

    let no_boxes_msg = gtk::Label::new(Some(&gettext("No Boxes")));
    let no_boxes_msg_2 = gtk::Label::new(Some(&gettext(
        "Click the + at the top-left to create your first box!",
    )));

    no_boxes_msg.add_css_class("title-1");
    no_boxes_msg_2.add_css_class("title-2");

    main_box.append(&no_boxes_msg);
    main_box.append(&no_boxes_msg_2);
}
