use gettextrs::*;
use std::thread;

use adw::{
    prelude::{ActionRowExt, MessageDialogExt, PreferencesRowExt},
    ActionRow, Application, PreferencesGroup, ToastOverlay, Window,
};
use gtk::{gio, glib::*, prelude::*, FileDialog};
use gtk::{
    glib::{self},
    Align, ApplicationWindow, Notebook, Orientation, PositionType,
};

mod distrobox_handler;
use distrobox_handler::*;

mod utils;
use utils::{
    get_distro_img, get_supported_terminals_list, get_terminal_and_separator_arg,
    get_user_home_directory, has_distrobox_installed, has_host_access, set_up_localisation,
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
        load_boxes(&main_box, &window, Some(0));
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
    // TRANSLATORS: Button tooltip
    add_btn.set_tooltip_text(Some(&gettext("Create A Distrobox")));

    let win_clone = window.clone();
    add_btn.connect_clicked(move |_btn| create_new_distrobox(&win_clone));

    let assemble_btn = gtk::Button::from_icon_name("document-properties-symbolic");
    if has_host_access() {
        // TRANSLATORS: Button tooltip
        assemble_btn.set_tooltip_text(Some(&gettext("Assemble A Distrobox")));

        assemble_btn.connect_clicked(clone!(@weak window => move |_btn| {
            let ini_filter = gtk::FileFilter::new();

            //TRANSLATORS: File type
            ini_filter.set_name(Some(&gettext("INI-Files")));
            ini_filter.add_mime_type("text/plain".as_ref());
            ini_filter.add_mime_type("application/textedit".as_ref());
            ini_filter.add_mime_type("application/zz-winassoc-ini".as_ref());

            let file_dialog = FileDialog::builder().default_filter(&ini_filter).modal(false).build();
            file_dialog.open(Some(&window), None::<&gio::Cancellable>, clone!(@weak window => move |result| {
                if let Ok(file) = result {
                    let ini_path = file.path().unwrap().into_os_string().into_string();
                    if ini_path.is_ok() {
                        assemble_new_distrobox(&window, ini_path.unwrap());
                    }
                }
            }));
        }));
    }

    let about_btn = gtk::Button::from_icon_name("help-about-symbolic");
    // TRANSLATORS: Button tooltip
    about_btn.set_tooltip_text(Some(&gettext("About BoxBuddy")));

    let win_clone = window.clone();
    about_btn.connect_clicked(move |_btn| show_about_popup(&win_clone));

    let refresh_btn = gtk::Button::from_icon_name("view-refresh-symbolic");
    // TRANSLATORS: Button tooltip - Re-render application
    refresh_btn.set_tooltip_text(Some(&gettext("Reload")));

    let win_clone = window.clone();
    refresh_btn.connect_clicked(move |_btn| {
        delayed_rerender(&win_clone, None);
    });

    let title_lbl = gtk::Label::new(Some("BoxBuddy"));
    title_lbl.add_css_class("header");

    let titlebar = adw::HeaderBar::builder().title_widget(&title_lbl).build();

    titlebar.pack_start(&add_btn);
    if has_host_access() {
        titlebar.pack_start(&assemble_btn);
    }
    titlebar.pack_end(&about_btn);
    titlebar.pack_end(&refresh_btn);

    window.set_titlebar(Some(&titlebar))
}

fn render_not_installed(main_box: &gtk::Box) {
    // TRANSLATORS: Error message
    let not_installed_lbl = gtk::Label::new(Some(&gettext("Distrobox not found!")));
    not_installed_lbl.add_css_class("title-1");

    // TRANSLATORS: Error message
    let not_installed_lbl_two = gtk::Label::new(Some(&gettext(
        "Distrobox could not be found, please ensure it is installed!",
    )));
    not_installed_lbl_two.add_css_class("title-2");

    main_box.append(&not_installed_lbl);
    main_box.append(&not_installed_lbl_two);
}

fn load_boxes(main_box: &gtk::Box, window: &ApplicationWindow, active_page: Option<u32>) {
    let tabs = Notebook::new();
    tabs.set_tab_pos(PositionType::Left);
    tabs.set_hexpand(true);
    tabs.set_vexpand(true);

    let boxes = get_all_distroboxes();

    if boxes.is_empty() {
        render_no_boxes_message(main_box);
        return;
    }

    for (box_num, dbox) in boxes.iter().enumerate() {
        let tab = make_box_tab(dbox, window, box_num as u32);
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

    if active_page.is_some() {
        tabs.set_current_page(active_page);
    }
}

fn make_box_tab(dbox: &DBox, window: &ApplicationWindow, tab_num: u32) -> gtk::Box {
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

    let stop_btn = gtk::Button::from_icon_name("media-playback-stop");
    // TRANSLATORS: Button tooltip
    stop_btn.set_tooltip_text(Some(&gettext("Stop Box")));

    let box_name_clone = dbox.name.clone();
    let win_clone = window.clone();
    stop_btn.connect_clicked(move |_btn| {
        stop_box(box_name_clone.to_string());
        delayed_rerender(&win_clone, Some(tab_num));
    });

    let title_box = gtk::Box::new(Orientation::Horizontal, 10);
    title_box.set_margin_start(10);
    title_box.append(&page_img);
    title_box.append(&page_title);
    title_box.append(&page_status);

    if dbox.is_running {
        title_box.append(&stop_btn);
    }

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
    // TRANSLATORS: Button Label
    open_terminal_row.set_title(&gettext("Open Terminal"));
    open_terminal_row.add_suffix(&open_terminal_button);
    open_terminal_row.set_activatable_widget(Some(&open_terminal_button));

    // upgrade button
    let upgrade_button = gtk::Button::from_icon_name("software-update-available-symbolic");
    upgrade_button.add_css_class("flat");

    let up_bn_clone = box_name.clone();
    upgrade_button.connect_clicked(move |_btn| on_upgrade_clicked(up_bn_clone.clone()));

    let upgrade_row = ActionRow::new();
    // TRANSLATORS: Button Label
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
    // TRANSLATORS: Button Label
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
    //TRANSLATORS: Button Label
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

fn assemble_new_distrobox(window: &ApplicationWindow, ini_file: String) {
    let assemble_box_popup = gtk::Window::new();
    assemble_box_popup.set_transient_for(Some(window));
    assemble_box_popup.set_default_size(700, 350);
    assemble_box_popup.set_modal(true);

    // TRANSLATORS: Popup Window Label
    let title_lbl = gtk::Label::new(Some(&gettext("Create New Distrobox")));
    title_lbl.add_css_class("header");
    let assemble_box_titlebar = adw::HeaderBar::builder().title_widget(&title_lbl).build();
    assemble_box_popup.set_titlebar(Some(&assemble_box_titlebar));

    // TRANSLATORS: Context label of the application doing something
    let assemble_lbl = gtk::Label::new(Some(&gettext("Assembling Distroboxes, please wait...")));

    //Loading spinner
    let loading_spinner = gtk::Spinner::new();
    loading_spinner.start();

    let main_box = gtk::Box::new(Orientation::Vertical, 10);
    main_box.set_margin_start(10);
    main_box.set_margin_end(10);
    main_box.set_margin_top(10);
    main_box.set_margin_bottom(10);

    main_box.append(&assemble_lbl);
    main_box.append(&loading_spinner);

    assemble_box_popup.set_child(Some(&main_box));
    assemble_box_popup.present();

    let (sender, receiver) =
        glib::MainContext::channel::<BoxCreatedMessage>(glib::Priority::DEFAULT);

    thread::spawn(move || {
        assemble_box(ini_file);
        sender.send(BoxCreatedMessage::Success).unwrap();
    });

    let ls_clone = loading_spinner.clone();
    let w_clone = window.clone();
    let popup = assemble_box_popup.clone();
    receiver.attach(None, move |msg| match msg {
        BoxCreatedMessage::Success => {
            ls_clone.stop();

            let num_boxes = get_number_of_boxes();
            delayed_rerender(&w_clone, Some(num_boxes - 1));
            popup.destroy();

            glib::ControlFlow::Continue
        }
    });
}

// callbacks
fn create_new_distrobox(window: &ApplicationWindow) {
    let new_box_popup = gtk::Window::new();
    new_box_popup.set_transient_for(Some(window));
    new_box_popup.set_modal(true);

    // TRANSLATORS: Button Label
    let title_lbl = gtk::Label::new(Some(&gettext("Create New Distrobox")));
    title_lbl.add_css_class("header");

    // TRANSLATORS: Button Label
    let create_btn = gtk::Button::with_label(&gettext("Create"));
    create_btn.add_css_class("suggested-action");

    let info_btn = gtk::Button::from_icon_name("dialog-information-symbolic");
    // TRANSLATORS: Button Label
    info_btn.set_tooltip_text(Some(&gettext("Additional Information")));
    let win_clone = window.clone();
    info_btn.connect_clicked(move |_btn| show_flatpak_dir_access_popup(&win_clone));

    // TRANSLATORS: Button Label
    let cancel_btn = gtk::Button::with_label(&gettext("Cancel"));

    cancel_btn.connect_clicked(move |btn| {
        let win = btn.root().and_downcast::<gtk::Window>().unwrap();
        win.destroy();
    });

    let new_box_titlebar = adw::HeaderBar::builder().title_widget(&title_lbl).build();

    new_box_titlebar.pack_end(&create_btn);
    new_box_titlebar.pack_start(&cancel_btn);

    if !has_host_access() {
        new_box_titlebar.pack_end(&info_btn);
    }

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

    // TRANSLATORS: Entry Label - Name input for new distrobox
    name_entry_row.set_title(&gettext("Name"));

    // custom home
    let choose_home_btn = gtk::Button::from_icon_name("document-open-symbolic");
    let home_select_row = adw::ActionRow::new();
    home_select_row.set_activatable_widget(Some(&choose_home_btn));
    home_select_row.add_suffix(&choose_home_btn);

    //home entry row for manual edit
    let home_entry_row = adw::EntryRow::new();
    home_entry_row.set_hexpand(true);

    //Volume box list will not bee added to main_box is host access == false
    let volume_box_list = gtk::ListBox::new();
    volume_box_list.add_css_class("boxed-list");
    volume_box_list.set_visible(false);

    // TRANSLATORS: Entry Label - Select home directory for new distrobox
    home_entry_row.set_title(&gettext("Home Directory (Leave blank for default)"));
    home_entry_row.set_width_request(600);
    home_select_row.add_prefix(&home_entry_row);
    let home_entry_row_future_clone = home_entry_row.clone();

    choose_home_btn.connect_clicked(clone!(@weak window => move |_btn| {
        let home_clone = home_entry_row.clone();
        let file_dialog = FileDialog::builder().modal(false).build();
        file_dialog.select_folder(Some(&window), None::<&gio::Cancellable>, clone!(@weak window => move |result| {
            if let Ok(file) = result {
                let home_path = file.path().unwrap().into_os_string().into_string().unwrap();
                home_clone.set_text(&home_path);
            }
        }));
    }));

    // Image
    let available_images = get_available_images_with_distro_name();
    let avail_images_as_ref: Vec<&str> = available_images.iter().map(|s| s as &str).collect();
    let imgs_strlist = gtk::StringList::new(avail_images_as_ref.as_slice());

    let exp = gtk::PropertyExpression::new(
        gtk::StringObject::static_type(),
        None::<gtk::Expression>,
        "string",
    );

    let image_select = gtk::DropDown::new(Some(imgs_strlist), Some(exp));
    image_select.set_enable_search(true);
    image_select.set_search_match_mode(gtk::StringFilterMatchMode::Substring);

    let image_select_row = adw::ActionRow::new();
    // TRANSLATORS - Label for Dropdown where the user selects the container image to create
    image_select_row.set_title(&gettext("Image"));
    image_select_row.set_activatable_widget(Some(&image_select));
    image_select_row.add_suffix(&image_select);

    // init
    let init_row = adw::SwitchRow::new();
    // TRANSLATORS - Label for Toggle when creating box to add systemd support
    init_row.set_title(&gettext("Use init system"));
    init_row.set_active(false);

    let loading_spinner = gtk::Spinner::new();

    let home_row = home_entry_row_future_clone.clone();
    let ne_row = name_entry_row.clone();
    let is_row = image_select_row.clone();
    let in_row = init_row.clone();
    let loading_spinner_clone = loading_spinner.clone();
    let win_clone = window.clone();
    let volume_box_list_clone = volume_box_list.clone();
    create_btn.connect_clicked(move |btn| {
        loading_spinner_clone.start();
        let mut name = ne_row.text().to_string();
        let mut home_path = home_row.text().to_string();
        let use_init = in_row.is_active();
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

        let mut volumes: Vec<String> = vec![];
        if volume_box_list_clone.is_visible() {
            while let Some(row) = volume_box_list_clone.last_child() {
                let entry_row = row
                    .first_child()
                    .unwrap()
                    .first_child()
                    .unwrap()
                    .first_child()
                    .unwrap()
                    .downcast::<adw::EntryRow>()
                    .unwrap();

                let mut volume_arg = String::new();
                volume_arg.push_str(entry_row.title().as_str());
                volume_arg.push_str(":");
                volume_arg.push_str(entry_row.text().as_str());

                volumes.push(volume_arg);
                volume_box_list_clone.remove(&row);
            }
        }

        name = name.replace(' ', "-");
        home_path = home_path.replace(' ', "\\ "); //Escape spaces
        image = image.split(" - ").last().unwrap().to_string();
        image = image.replace(" ✦ ", "");

        let name_clone = name.clone();

        let (sender, receiver) =
            glib::MainContext::channel::<BoxCreatedMessage>(glib::Priority::DEFAULT);

        thread::spawn(move || {
            create_box(name, image, home_path, use_init, volumes);
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

                let num_boxes = get_number_of_boxes();
                delayed_rerender(&w_clone, Some(num_boxes - 1));

                open_terminal_in_box(name_clone.clone());

                glib::ControlFlow::Continue
            }
        });
    });

    boxed_list.append(&name_entry_row);
    boxed_list.append(&image_select_row);
    boxed_list.append(&init_row);

    if has_host_access() {
        boxed_list.append(&home_select_row);
    }

    main_box.append(&boxed_list);

    //Volumes
    if has_host_access() {
        let volume_add_btn = gtk::Button::from_icon_name("list-add-symbolic");
        volume_add_btn.set_css_classes(&["flat"]);

        let volume_box_list_clone = volume_box_list.clone();
        volume_add_btn.connect_clicked(clone!(@weak window, @weak volume_box_list_clone => move |_btn| {
            let file_dialog = FileDialog::builder().modal(false).build();
            file_dialog.select_folder(Some(&window), None::<&gio::Cancellable>, clone!(@weak window, @weak volume_box_list_clone => move |result| {
                if let Ok(file) = result {
                    let volume_path = file.path().unwrap().into_os_string().into_string().unwrap();

                    if volume_path.starts_with(get_user_home_directory().as_str()) {
                        show_volume_is_in_user_home_popup(&window);
                    } else {
                        let volume_remove_btn = gtk::Button::from_icon_name("list-remove-symbolic");
                        volume_remove_btn.set_css_classes(&["flat"]);
                        volume_remove_btn.set_margin_top(10);
                        volume_remove_btn.set_margin_bottom(10);
                        let volume_action_row = adw::ActionRow::new();
                        volume_action_row.add_suffix(&volume_remove_btn);
                        volume_action_row.set_selectable(false);

                        let volume_entry_row = adw::EntryRow::new();
                        let mut volume_path_title = String::new();
                        volume_path_title.push_str(volume_path.clone().as_str());
                        volume_entry_row.set_title(&volume_path_title);
                        volume_entry_row.set_hexpand(true);
                        volume_entry_row.set_width_request(600);
                        volume_entry_row.set_text(&volume_path);

                        let volume_action_row_clone = volume_action_row.clone();
                        let volume_box_list_button_clone = volume_box_list_clone.clone();
                        volume_remove_btn.connect_clicked(move |_btn| {
                            volume_box_list_button_clone.remove(&volume_action_row_clone);
                            if volume_box_list_button_clone.last_child() == None {
                                volume_box_list_button_clone.set_visible(false);
                            }
                        });

                        volume_action_row.add_prefix(&volume_entry_row);
                        volume_box_list_clone.append(&volume_action_row);
                        volume_box_list_clone.set_visible(true);
                    }
                }
            }));
        }));

        let volume_preference_group = adw::PreferencesGroup::builder()
            .title(&gettext("Volumes:"))
            .description(&gettext(
                "Additional directories the new box should be able to access",
            ))
            .header_suffix(&volume_add_btn)
            .build();

        main_box.append(&volume_preference_group);
        main_box.append(&volume_box_list);
    }

    main_box.append(&loading_spinner);

    new_box_popup.set_child(Some(&main_box));
    new_box_popup.present();
}

fn show_about_popup(window: &ApplicationWindow) {
    let d = adw::AboutWindow::new();
    d.set_transient_for(Some(window));
    d.set_application_name("BoxBuddy");
    d.set_version("1.2.1");
    d.set_developer_name("Dvlv");
    d.set_license_type(gtk::License::MitX11);
    d.set_comments(
        "A Graphical Manager for your Distroboxes.
    \nBoxBuddy is not partnered with or endorsed by any linux distributions or companies.
    \nTrademarks, service marks, and logos are the property of their respective owners.",
    );
    d.set_website("https://github.com/Dvlv/BoxBuddyRS");
    d.set_issue_url("https://github.com/Dvlv/BoxBuddyRS/issues");
    d.set_support_url("https://dvlv.github.io/BoxBuddyRS");
    d.set_developers(&["Dvlv", "VortexAcherontic"]);
    d.set_application_icon("io.github.dvlv.boxbuddyrs");
    d.set_translator_credits("Vovkiv - RU and UK\nalbanobattistella - IT\nVortexAcherontic - DE");
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

    // TRANSLATORS: Window Title - shows list of installed applications in distrobox
    apps_popup.set_title(Some(&gettext("Installed Applications")));

    let main_box = gtk::Box::new(Orientation::Vertical, 10);
    main_box.set_margin_start(10);
    main_box.set_margin_end(10);
    main_box.set_margin_top(10);
    main_box.set_margin_bottom(10);

    let loading_spinner = gtk::Spinner::new();

    // TRANSLATORS: Loading Message
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
                    //TRANSLATORS: Error Message
                    let no_apps_lbl = gtk::Label::new(Some(&gettext("No Applications Installed")));
                    no_apps_lbl.add_css_class("title-2");
                    main_box.append(&no_apps_lbl);
                } else {
                    //TRANSLATORS: Window Title
                    loading_lbl.set_text(&gettext("Available Applications"));

                    let boxed_list = gtk::ListBox::new();
                    boxed_list.add_css_class("boxed-list");

                    for app in apps {
                        let row = adw::ActionRow::new();
                        row.set_title(&markup_escape_text(&app.name.to_string()));

                        let img = gtk::Image::from_icon_name(&app.icon);

                        //TRANSLATORS: Button Label
                        let run_btn = gtk::Button::with_label(&gettext("Run"));
                        run_btn.add_css_class("pill");
                        run_btn.set_width_request(100);
                        let box_name_clone = box_name.clone();
                        let app_clone = app.clone();
                        run_btn.connect_clicked(move |_btn| {
                            run_app_in_box(&app_clone, box_name_clone.clone());
                        });

                        row.add_prefix(&img);
                        row.add_suffix(&run_btn);
                        row.add_suffix(&gtk::Separator::new(gtk::Orientation::Horizontal));

                        if app.is_on_host {
                            let remove_from_menu_btn =
                                //TRANSLATORS: Button Label
                                gtk::Button::with_label(&gettext("Remove From Menu"));
                            remove_from_menu_btn.add_css_class("pill");
                            remove_from_menu_btn.set_width_request(200);

                            let box_name_clone = box_name.clone();
                            let loading_lbl_clone = loading_lbl.clone();
                            let app_clone = app.clone();
                            remove_from_menu_btn.connect_clicked(move |_btn| {
                                remove_app_from_menu(
                                    &app_clone,
                                    box_name_clone.clone(),
                                    &loading_lbl_clone.clone(),
                                );
                            });
                            row.add_suffix(&remove_from_menu_btn);
                        } else {
                            //TRANSLATORS: Button Label
                            let add_menu_btn = gtk::Button::with_label(&gettext("Add To Menu"));
                            add_menu_btn.add_css_class("pill");
                            add_menu_btn.set_width_request(200);

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
                            row.add_suffix(&add_menu_btn);
                        }

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
    //TRANSLATORS: Success Message
    success_lbl.set_text(&gettext("App Exported!"));
}

fn remove_app_from_menu(app: &DBoxApp, box_name: String, success_lbl: &gtk::Label) {
    let _ = remove_app_from_host(app.name.to_string(), box_name);
    //TRANSLATORS: Success Message
    success_lbl.set_text(&gettext("App Removed!"));
}

fn run_app_in_box(app: &DBoxApp, box_name: String) {
    run_command_in_box(app.exec_name.to_string(), box_name);
}

fn on_delete_clicked(window: &ApplicationWindow, box_name: String) {
    //TRANSLATORS: Confirmation Dialogue - has name of Distrobox appended to the end, please leave a space
    let are_you_sure_pre = &gettext("Are you sure you want to delete ");
    let d = adw::MessageDialog::new(
        Some(window),
        //TRANSLATORS: Confirmation Dialogue
        Some(&gettext("Really Delete?")),
        Some(&format!("{are_you_sure_pre} {box_name}?")),
    );
    d.set_transient_for(Some(window));
    //TRANSLATORS: Button Label
    d.add_response("cancel", &gettext("Cancel"));
    //TRANSLATORS: Button Label
    d.add_response("delete", &gettext("Delete"));
    d.set_default_response(Some("cancel"));
    d.set_close_response("cancel");
    d.set_response_appearance("delete", adw::ResponseAppearance::Destructive);

    let win_clone = window.clone();

    d.connect_response(None, move |d, res| {
        if res == "delete" {
            delete_box(box_name.clone());
            d.destroy();

            //TRANSLATORS: Success Text
            let toast = adw::Toast::new(&gettext("Box Deleted!"));
            if let Some(child) = win_clone.clone().child() {
                let toast_area = child.downcast::<ToastOverlay>();
                toast_area.unwrap().add_toast(toast);
            }

            delayed_rerender(&win_clone, None);
        }
    });

    d.present()
}

fn delayed_rerender(window: &ApplicationWindow, active_page: Option<u32>) {
    let main_box = window.child().unwrap().first_child().unwrap();
    let main_box_as_box = main_box.downcast::<gtk::Box>().unwrap();

    load_boxes(&main_box_as_box, window, active_page);
}

fn show_no_supported_terminal_popup(window: &ApplicationWindow) {
    let supported_terminals = get_supported_terminals_list();

    //TRANSLATORS: Error Message
    let supported_terminals_pre = &gettext("Please install one of the supported terminals:");
    let supported_terminals_body = format!("{supported_terminals_pre}\n\n{supported_terminals}");
    let d = adw::MessageDialog::new(
        Some(window),
        //TRANSLATORS: Error Message
        Some(&gettext("No supported terminal found")),
        Some(&supported_terminals_body),
    );
    d.set_transient_for(Some(window));
    //TRANSLATORS: Button Label
    d.add_response("ok", &gettext("Ok"));
    d.set_default_response(Some("ok"));
    d.set_close_response("ok");

    d.present();
}

fn render_no_boxes_message(main_box: &gtk::Box) {
    while let Some(child) = main_box.first_child() {
        main_box.remove(&child);
    }

    //TRANSLATORS: Error Message
    let no_boxes_msg = gtk::Label::new(Some(&gettext("No Boxes")));
    //TRANSLATORS: Instructions
    let no_boxes_msg_2 = gtk::Label::new(Some(&gettext(
        "Click the + at the top-left to create your first box!",
    )));

    no_boxes_msg.add_css_class("title-1");
    no_boxes_msg_2.add_css_class("title-2");

    main_box.append(&no_boxes_msg);
    main_box.append(&no_boxes_msg_2);
}

fn show_flatpak_dir_access_popup(window: &ApplicationWindow) {
    //TRANSLATORS: Error / Info Message
    let message_body = gettext("You appear to be using a Flatpak of BoxBuddy without filesystem access. If you wish to set a Custom Home Directory you will need to grant filesystem access. Please see the <a href='https://dvlv.github.io/BoxBuddyRS/tips'>documentation for details.</a>");
    let d = adw::MessageDialog::new(
        Some(window),
        //TRANSLATORS: Popup Heading
        Some(&gettext("Sandboxed Flatpak Detected")),
        Some(&message_body),
    );
    d.set_body_use_markup(true);
    d.set_transient_for(Some(window));
    //TRANSLATORS: Button Label
    d.add_response("ok", &gettext("Ok"));
    d.set_default_response(Some("ok"));
    d.set_close_response("ok");

    d.present();
}

fn show_volume_is_in_user_home_popup(window: &ApplicationWindow) {
    //TRANSLATORS: Error / Info Message
    let message_body = gettext("The volume path is in your user directory. Distrobox will already have access to it. This is also true if you've set a different home directory for this box.");
    let d = adw::MessageDialog::new(
        Some(window),
        //TRANSLATORS: Popup Heading
        Some(&gettext("Volume is already accessible")),
        Some(&message_body),
    );
    d.set_body_use_markup(true);
    d.set_transient_for(Some(window));
    //TRANSLATORS: Button Label
    d.add_response("ok", &gettext("Ok"));
    d.set_default_response(Some("ok"));
    d.set_close_response("ok");

    d.present();
}
