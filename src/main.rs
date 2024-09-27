use gettextrs::gettext;
use std::thread;

use adw::{
    prelude::{ActionRowExt, MessageDialogExt, PreferencesRowExt},
    ActionRow, Application, StyleManager, ToastOverlay,
};
use gtk::{
    gio,
    gio::Settings,
    glib::{self},
    glib::{clone, markup_escape_text, Cast, CastNone, StaticType},
    prelude::*,
    Align, ApplicationWindow, FileDialog, Notebook, Orientation, PositionType,
};

mod distrobox_handler;
use distrobox_handler::{
    assemble_box, clone_box, create_box, delete_box, export_app_from_box, get_all_distroboxes,
    get_apps_in_box, get_available_images_with_distro_name, get_number_of_boxes,
    install_deb_in_box, install_rpm_in_box, open_terminal_in_box, remove_app_from_host,
    run_command_in_box, stop_box, upgrade_box, DBox, DBoxApp,
};

mod utils;
use utils::{
    get_assemble_icon, get_cpu_and_mem_usage, get_deb_distros, get_distro_img,
    get_download_dir_path, get_my_deb_boxes, get_my_rpm_boxes, get_rpm_distros,
    get_supported_terminals, get_supported_terminals_list, get_terminal_and_separator_arg,
    has_distrobox_installed, has_file_extension, has_home_or_host_access, has_host_access,
    has_podman_or_docker_installed, set_up_localisation,
};
const APP_ID: &str = "io.github.dvlv.boxbuddyrs";

enum AppsFetchMessage {
    AppsFetched(Vec<DBoxApp>),
}

enum BoxCreatedMessage {
    Success,
}

#[derive(Debug, Clone, Copy)]
enum BinaryPackageType {
    Deb,
    Rpm,
}

fn main() -> glib::ExitCode {
    set_up_localisation();

    // Create a new application
    let app = Application::builder()
        .application_id(APP_ID)
        .flags(gio::ApplicationFlags::HANDLES_OPEN)
        .build();

    app.connect_open(build_ui_as_open);
    app.connect_activate(build_ui);

    app.set_accels_for_action("win.refresh", &["F5", "<Ctrl>R"]);
    app.set_accels_for_action("win.close", &["<Ctrl>Q", "<Ctrl>W"]);

    // Run the application
    app.run()
}

fn make_window(app: &Application) -> ApplicationWindow {
    let window = ApplicationWindow::builder()
        .application(app)
        .title("BoxBuddy")
        .build();

    window.set_default_size(800, 525);

    make_titlebar(&window);

    let scrolled_win = gtk::ScrolledWindow::new();
    scrolled_win.set_vexpand(true);
    scrolled_win.set_hexpand(true);

    let scroll_area = gtk::Box::new(gtk::Orientation::Vertical, 5);
    scroll_area.set_vexpand(true);
    scroll_area.set_hexpand(true);

    let toast_overlay = ToastOverlay::new();
    let main_box = gtk::Box::new(Orientation::Vertical, 10);
    main_box.set_orientation(Orientation::Vertical);
    main_box.set_hexpand(true);
    main_box.set_vexpand(true);
    main_box.set_margin_top(10);
    main_box.set_margin_bottom(10);
    main_box.set_margin_start(10);
    main_box.set_margin_end(10);

    main_box.append(&scrolled_win);
    scrolled_win.set_child(Some(&scroll_area));

    toast_overlay.set_child(Some(&main_box));
    window.set_child(Some(&toast_overlay));

    if has_distrobox_installed() {
        if has_podman_or_docker_installed() {
            load_boxes(&scroll_area, &window, Some(0));
        } else {
            render_podman_not_installed(&scroll_area);
        }
    } else {
        render_not_installed(&scroll_area);
    }

    set_window_actions(&window);

    window.present();

    window
}

fn build_ui(app: &Application) {
    // Create a window and set the title
    let window = make_window(app);

    let (term, _) = get_terminal_and_separator_arg();
    if term.is_empty() {
        show_no_supported_terminal_popup(&window);
    }
}

fn build_ui_as_open(app: &Application, files: &[gio::File], _hint: &str) {
    let window = make_window(app);

    if !files.is_empty() {
        // BoxBuddy will only support opening one file at a time for now

        // TODO I dont like all this unwrapping
        let first_file = files.first().unwrap();
        let file_path = first_file.path().unwrap();
        let file_path_str = file_path.to_str().unwrap();

        if has_file_extension(file_path_str, "rpm") {
            show_install_binary_popup(&window, file_path_str, BinaryPackageType::Rpm);
        } else if has_file_extension(file_path_str, "deb") {
            show_install_binary_popup(&window, file_path_str, BinaryPackageType::Deb);
        }
    }

    // if file not recognised we COULD show a "not recognised" message, but
    // possibly better to just let BoxBuddy run as if there were no file
}

fn make_titlebar(window: &ApplicationWindow) {
    let add_btn = gtk::Button::from_icon_name("list-add-symbolic");
    // TRANSLATORS: Button tooltip
    add_btn.set_tooltip_text(Some(&gettext("Create A Distrobox")));

    let win_clone = window.clone();
    add_btn.connect_clicked(move |_btn| create_new_distrobox(&win_clone));

    let assemble_img = gtk::Image::from_file(get_assemble_icon());
    let assemble_btn = gtk::Button::new();
    assemble_btn.set_child(Some(&assemble_img));
    assemble_btn.add_css_class("flat");

    let assemble_btn_clone = assemble_btn.clone();
    let style_manager = StyleManager::default();
    style_manager.connect_dark_notify(move |_btn| {
        let new_image = gtk::Image::from_file(get_assemble_icon());
        assemble_btn_clone.set_child(Some(&new_image));
    });

    if has_home_or_host_access() {
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

    let menu_btn = gtk::MenuButton::new();
    menu_btn.set_icon_name("open-menu-symbolic");
    menu_btn.set_menu_model(Some(&get_main_menu_model()));
    //TRANSLATORS: Button tooltip
    menu_btn.set_tooltip_text(Some(&gettext("Menu")));

    let titlebar = adw::HeaderBar::new();

    titlebar.pack_start(&add_btn);
    if has_home_or_host_access() {
        titlebar.pack_start(&assemble_btn);
    }
    titlebar.pack_end(&menu_btn);

    window.set_titlebar(Some(&titlebar));
}

fn set_window_actions(window: &ApplicationWindow) {
    let action_close = gio::ActionEntry::builder("close")
        .activate(|window: &ApplicationWindow, _, _| {
            window.close();
        })
        .build();

    let action_refresh = gio::ActionEntry::builder("refresh")
        .activate(|window: &ApplicationWindow, _, _| {
            delayed_rerender(window, None);
        })
        .build();

    let action_about = gio::ActionEntry::builder("about")
        .activate(|window: &ApplicationWindow, _, _| {
            show_about_popup(window);
        })
        .build();

    let action_set_preferred_terminal = gio::ActionEntry::builder("set_preferred_terminal")
        .activate(|window: &ApplicationWindow, _, _| {
            show_preferred_terminal_popup(window);
        })
        .build();

    window.add_action_entries([
        action_refresh,
        action_about,
        action_close,
        action_set_preferred_terminal,
    ]);
}

fn get_main_menu_model() -> gio::MenuModel {
    // Massive thanks to https://blog.libove.org/posts/rust-gtk--creating-a-menu-bar-programmatically-with-gtk-rs/
    let menu = gio::Menu::new();

    menu.insert_item(
        0,
        //TRANSLATORS: Menu Item
        &gio::MenuItem::new(Some(&gettext("Refresh")), Some("win.refresh")),
    );
    menu.insert_item(
        1,
        &gio::MenuItem::new(
            //TRANSLATORS: Menu Item
            Some(&gettext("Set Preferred Terminal")),
            Some("win.set_preferred_terminal"),
        ),
    );
    menu.insert_item(
        2,
        //TRANSLATORS: Menu Item
        &gio::MenuItem::new(Some(&gettext("About BoxBuddy")), Some("win.about")),
    );
    menu.insert_item(
        3,
        //TRANSLATORS: Menu Item
        &gio::MenuItem::new(Some(&gettext("Quit")), Some("win.close")),
    );

    menu.into()
}

fn render_not_installed(scroll_area: &gtk::Box) {
    // TRANSLATORS: Error message
    let not_installed_lbl = gtk::Label::new(Some(&gettext("Distrobox not found!")));
    not_installed_lbl.add_css_class("title-1");

    // TRANSLATORS: Error message
    let not_installed_lbl_two = gtk::Label::new(Some(&gettext(
        "Distrobox could not be found, please ensure it is installed!",
    )));
    not_installed_lbl_two.add_css_class("title-2");

    scroll_area.append(&not_installed_lbl);
    scroll_area.append(&not_installed_lbl_two);
}

fn render_podman_not_installed(scroll_area: &gtk::Box) {
    // TRANSLATORS: Error message
    let not_installed_lbl = gtk::Label::new(Some(&gettext("Podman / Docker not found!")));
    not_installed_lbl.add_css_class("title-1");

    // TRANSLATORS: Error message
    let not_installed_lbl_two = gtk::Label::new(Some(&gettext(
        "Could not find podman or docker, please install one of them!",
    )));
    not_installed_lbl_two.add_css_class("title-2");

    scroll_area.append(&not_installed_lbl);
    scroll_area.append(&not_installed_lbl_two);
}

fn load_boxes(scroll_area: &gtk::Box, window: &ApplicationWindow, active_page: Option<u32>) {
    let tabs = Notebook::new();
    tabs.set_tab_pos(PositionType::Left);
    tabs.set_hexpand(true);
    tabs.set_vexpand(true);

    let boxes = get_all_distroboxes();

    if boxes.is_empty() {
        render_no_boxes_message(scroll_area);
        return;
    }

    for (box_num, dbox) in boxes.iter().enumerate() {
        let tab = make_box_tab(dbox, window, box_num as u32);
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

    while let Some(child) = scroll_area.first_child() {
        scroll_area.remove(&child);
    }

    scroll_area.append(&tabs);

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
        stop_box(&box_name_clone);
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
    boxed_list.set_selection_mode(gtk::SelectionMode::None);
    boxed_list.add_css_class("boxed-list");

    // Terminal Icon
    let open_terminal_icon = gtk::Image::from_icon_name("utilities-terminal-symbolic");

    let open_terminal_row = ActionRow::new();
    // TRANSLATORS: Row Label
    open_terminal_row.set_title(&gettext("Open Terminal"));
    open_terminal_row.add_suffix(&open_terminal_icon);
    open_terminal_row.set_activatable(true);

    let term_bn_clone = box_name.clone();
    open_terminal_row
        .connect_activated(move |_row| on_open_terminal_clicked(term_bn_clone.clone()));

    // Upgrade Icon
    let upgrade_icon = gtk::Image::from_icon_name("software-update-available-symbolic");

    let upgrade_row = ActionRow::new();
    // TRANSLATORS: Row Label
    upgrade_row.set_title(&gettext("Upgrade Box"));
    upgrade_row.add_suffix(&upgrade_icon);
    upgrade_row.set_activatable(true);

    let up_bn_clone = box_name.clone();
    upgrade_row.connect_activated(move |_row| on_upgrade_clicked(&up_bn_clone));

    // Show Applications Icon
    let show_applications_icon = gtk::Image::from_icon_name("application-x-executable-symbolic");

    let show_applications_row = ActionRow::new();
    // TRANSLATORS: Row Label
    show_applications_row.set_title(&gettext("View Applications"));
    show_applications_row.add_suffix(&show_applications_icon);
    show_applications_row.set_activatable(true);

    let show_bn_clone = box_name.clone();
    let win_clone = window.clone();
    show_applications_row.connect_activated(move |_row| {
        on_show_applications_clicked(&win_clone, show_bn_clone.clone());
    });

    // Install Deb Icon
    let deb_bn_clone = box_name.clone();
    let install_deb_icon = gtk::Image::from_icon_name("system-software-install-symbolic");

    // Install RPM Icon
    let rpm_bn_clone = box_name.clone();
    let install_rpm_icon = gtk::Image::from_icon_name("system-software-install-symbolic");

    // Delete Icon
    let delete_icon = gtk::Image::from_icon_name("user-trash-symbolic");

    let delete_row = ActionRow::new();
    //TRANSLATORS: Row Label
    delete_row.set_title(&gettext("Delete Box"));
    delete_row.add_suffix(&delete_icon);
    delete_row.set_activatable(true);

    let del_bn_clone = box_name.clone();
    let win_clone = window.clone();
    delete_row.connect_activated(move |_row| on_delete_clicked(&win_clone, del_bn_clone.clone()));

    // Clone Box Icon
    let clone_icon = gtk::Image::from_icon_name("edit-copy-symbolic");

    let clone_row = ActionRow::new();
    //TRANSLATORS: Row Label
    clone_row.set_title(&gettext("Clone Box"));
    clone_row.add_suffix(&clone_icon);
    clone_row.set_activatable(true);

    let clone_bn = box_name.clone();
    let win_clone = window.clone();
    clone_row.connect_activated(move |_row| on_clone_clicked(&win_clone, clone_bn.clone()));

    // put all into list
    boxed_list.append(&open_terminal_row);
    boxed_list.append(&upgrade_row);
    boxed_list.append(&show_applications_row);

    // Make deb / rpm row if applicable
    let deb_distros = get_deb_distros();
    let rpm_distros = get_rpm_distros();

    let binary_row = ActionRow::new();
    if deb_distros.contains(&dbox.distro) {
        // TRANSLATORS: Row Label
        binary_row.set_title(&gettext("Install .deb File"));
        binary_row.add_suffix(&install_deb_icon);
        binary_row.set_activatable(true);

        let win_clone = window.clone();
        binary_row.connect_activated(move |_row| {
            on_install_deb_clicked(&win_clone, deb_bn_clone.clone());
        });

        boxed_list.append(&binary_row);
    } else if rpm_distros.contains(&dbox.distro) {
        // TRANSLATORS: Row Label
        binary_row.set_title(&gettext("Install .rpm File"));
        binary_row.add_suffix(&install_rpm_icon);
        binary_row.set_activatable(true);

        let win_clone = window.clone();
        binary_row.connect_activated(move |_row| {
            on_install_rpm_clicked(&win_clone, rpm_bn_clone.clone());
        });

        boxed_list.append(&binary_row);
    }

    boxed_list.append(&clone_row);
    boxed_list.append(&delete_row);

    tab_box.append(&title_box);
    tab_box.append(&gtk::Separator::new(Orientation::Horizontal));
    tab_box.append(&boxed_list);

    // CPU and Mem Stats
    if dbox.is_running {
        let cpu_mem_stats = get_cpu_and_mem_usage(&box_name);
        if !cpu_mem_stats.cpu.is_empty() {
            let stats_box = gtk::Box::new(gtk::Orientation::Horizontal, 10);
            stats_box.set_hexpand(true);
            let cpu_label = gtk::Label::new(Some(&format!("CPU: {}", cpu_mem_stats.cpu)));
            let mem_label = gtk::Label::new(Some(&format!(
                "Memory: {} ({})",
                cpu_mem_stats.mem, cpu_mem_stats.mem_percent
            )));

            cpu_label.set_halign(Align::End);
            cpu_label.set_hexpand(true);

            mem_label.set_halign(Align::End);

            stats_box.append(&cpu_label);
            stats_box.append(&mem_label);

            tab_box.append(&stats_box);
        }
    }

    tab_box
}

fn assemble_new_distrobox(window: &ApplicationWindow, ini_file: String) {
    let assemble_box_popup = gtk::Window::builder()
        // TRANSLATORS: Popup Window Title
        .title(gettext("Create New Distrobox"))
        .transient_for(window)
        .default_width(700)
        .default_height(350)
        .modal(true)
        .build();

    let assemble_box_titlebar = adw::HeaderBar::new();
    assemble_box_titlebar.set_show_end_title_buttons(false);
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
        assemble_box(&ini_file);
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
    let new_box_popup = gtk::Window::builder()
        // TRANSLATORS: Popup Window Title
        .title(gettext("Create New Distrobox"))
        .transient_for(window)
        .default_width(700)
        .default_height(350)
        .modal(true)
        .build();

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

    let new_box_titlebar = adw::HeaderBar::new();
    new_box_titlebar.set_show_end_title_buttons(false);

    new_box_titlebar.pack_end(&create_btn);
    new_box_titlebar.pack_start(&cancel_btn);

    if !has_home_or_host_access() {
        new_box_titlebar.pack_end(&info_btn);
    }

    new_box_popup.set_titlebar(Some(&new_box_titlebar));

    let main_box = gtk::Box::new(Orientation::Vertical, 10);
    main_box.set_margin_start(10);
    main_box.set_margin_end(10);
    main_box.set_margin_top(10);
    main_box.set_margin_bottom(10);

    let boxed_list = gtk::ListBox::new();
    boxed_list.set_selection_mode(gtk::SelectionMode::None);
    boxed_list.add_css_class("boxed-list");

    // name input
    let name_entry_row = adw::EntryRow::new();
    name_entry_row.set_hexpand(true);

    // TRANSLATORS: Entry Label - Name input for new distrobox
    name_entry_row.set_title(&gettext("Name"));

    // custom home
    let choose_home_btn = gtk::Button::from_icon_name("document-open-symbolic");
    choose_home_btn.set_margin_top(10);
    choose_home_btn.set_margin_bottom(10);
    let home_select_row = adw::ActionRow::new();
    home_select_row.set_activatable_widget(Some(&choose_home_btn));
    home_select_row.add_suffix(&choose_home_btn);

    //home entry row for manual edit
    let home_entry_row = adw::EntryRow::new();
    home_entry_row.set_hexpand(true);

    //Additional Volumes - will not be shown without host access
    let volume_box_list = gtk::ListBox::new();
    volume_box_list.set_selection_mode(gtk::SelectionMode::None);
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
            let mut index = 0;
            let mut row = volume_box_list_clone.row_at_index(index);
            while row.is_some() {
                let entry_row = row
                    .clone()
                    .unwrap()
                    .first_child()
                    .unwrap()
                    .first_child()
                    .unwrap()
                    .first_child()
                    .unwrap()
                    .downcast::<adw::EntryRow>()
                    .unwrap();
                let volume_arg = format!("{}:{}", entry_row.title(), entry_row.text());
                volumes.push(volume_arg);
                index += 1;
                row = volume_box_list_clone.row_at_index(index);
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
            create_box(&name, &image, &home_path, use_init, volumes.as_slice());
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

    if has_home_or_host_access() {
        boxed_list.append(&home_select_row);
    }

    main_box.append(&boxed_list);

    //Volumes
    if has_host_access() {
        let volume_box_list_clone = volume_box_list.clone();

        let volume_add_btn = gtk::Button::from_icon_name("list-add-symbolic");
        volume_add_btn.add_css_class("flat");
        // TRANSLATORS: Button tooltip
        volume_add_btn.set_tooltip_text(Some(&gettext("Add a volume")));
        volume_add_btn.connect_clicked(clone!(@weak window, @weak volume_box_list_clone => move |_btn| {
            let file_dialog = FileDialog::builder().modal(false).build();
            file_dialog.select_folder(Some(&window), None::<&gio::Cancellable>, clone!(@weak window, @weak volume_box_list_clone => move |result| {
                if let Ok(file) = result {
                    let volume_path = file.path().unwrap().into_os_string().into_string().unwrap();

                    // /var/home is Silverblue and pals
                    if volume_path.starts_with("/home/") || volume_path.starts_with("/var/home/") {
                        show_volume_is_in_user_home_popup(&window);
                    } else {
                        let volume_remove_btn = gtk::Button::from_icon_name("list-remove-symbolic");
                        // TRANSLATORS: Button tooltip
                        volume_remove_btn.set_tooltip_text(Some(&gettext("Remove volume")));
                        volume_remove_btn.add_css_class("flat");
                        volume_remove_btn.set_margin_top(10);
                        volume_remove_btn.set_margin_bottom(10);

                        let volume_action_row = adw::ActionRow::new();
                        volume_action_row.add_suffix(&volume_remove_btn);
                        volume_action_row.set_selectable(false);

                        let volume_path_title = volume_path.clone().to_string();
                        let volume_entry_row = adw::EntryRow::new();
                        volume_entry_row.set_title(&volume_path_title);
                        // TRANSLATORS: Help text for volume input
                        volume_entry_row.set_tooltip_text(Some(&gettext("Enter the location to mount this folder inside your new box")));
                        volume_entry_row.set_hexpand(true);
                        volume_entry_row.set_width_request(600);
                        volume_entry_row.set_text(&volume_path);

                        let volume_action_row_clone = volume_action_row.clone();
                        let volume_box_list_button_clone = volume_box_list_clone.clone();
                        volume_remove_btn.connect_clicked(move |_btn| {
                            volume_box_list_button_clone.remove(&volume_action_row_clone);
                            if volume_box_list_button_clone.last_child().is_none() {
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
            // TRANSLATORS: Subheading
            .title(gettext("Additional Volumes:"))
            // TRANSLATORS: Context for the Additional Volumes subheading
            .description(gettext(
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
    d.set_version("2.2.13");
    d.set_developer_name("Dvlv");
    d.set_license_type(gtk::License::MitX11);
    // TRANSLATORS: Description of the application
    d.set_comments(&gettext(
        "A Graphical Manager for your Distroboxes.
    \nBoxBuddy is not partnered with or endorsed by any linux distributions or companies.
    \nTrademarks, service marks, and logos are the property of their respective owners.",
    ));
    d.set_website("https://github.com/Dvlv/BoxBuddyRS");
    d.set_issue_url("https://github.com/Dvlv/BoxBuddyRS/issues");
    d.set_support_url("https://dvlv.github.io/BoxBuddyRS");
    d.set_developers(&["Dvlv", "VortexAcherontic"]);
    d.set_application_icon("io.github.dvlv.boxbuddyrs");
    d.set_translator_credits(
        "MLSci - CN
VortexAcherontic - DE
Pyrofanis - EL
Sebrice - ES
fonskip - fr_FR
Scrambled777 - Hi
nalbanobattistella - IT
Luiz-C-Lima - pt_BR
Vovkiv - RU and UK",
    );
    d.present();
}

fn on_open_terminal_clicked(box_name: String) {
    open_terminal_in_box(box_name);
}

fn on_upgrade_clicked(box_name: &str) {
    upgrade_box(box_name);
}

fn on_show_applications_clicked(window: &ApplicationWindow, box_name: String) {
    let apps_popup = gtk::Window::builder()
        // TRANSLATORS: Window Title - shows list of installed applications in distrobox
        .title(gettext("Installed Applications"))
        .transient_for(window)
        .default_width(700)
        .default_height(350)
        .modal(true)
        .build();

    let titlebar = adw::HeaderBar::new();

    let main_box = gtk::Box::new(Orientation::Vertical, 10);
    main_box.set_margin_start(10);
    main_box.set_margin_end(10);
    main_box.set_margin_top(10);
    main_box.set_margin_bottom(10);

    let loading_spinner = gtk::Spinner::new();

    // TRANSLATORS: Loading Message
    let loading_lbl = gtk::Label::new(Some(&gettext("Loading...")));
    loading_lbl.add_css_class("title-2");

    let scrolled_win = gtk::ScrolledWindow::new();
    scrolled_win.set_vexpand(true);
    scrolled_win.set_hexpand(true);

    let scroll_area = gtk::Box::new(gtk::Orientation::Vertical, 5);
    scroll_area.set_vexpand(true);
    scroll_area.set_hexpand(true);

    scroll_area.append(&loading_lbl);
    scroll_area.append(&loading_spinner);

    scrolled_win.set_child(Some(&scroll_area));

    main_box.append(&scrolled_win);

    apps_popup.set_child(Some(&main_box));
    apps_popup.set_titlebar(Some(&titlebar));
    loading_spinner.start();
    apps_popup.present();
    apps_popup.queue_draw();

    let (sender, receiver) =
        glib::MainContext::channel::<AppsFetchMessage>(glib::Priority::DEFAULT);
    let box_name_clone = box_name.clone();

    // fetch these in background thread so we can render the window with loading message
    // Massive thanks to https://coaxion.net/blog/2019/02/mpsc-channel-api-for-painless-usage-of-threads-with-gtk-in-rust/
    thread::spawn(move || {
        let apps = get_apps_in_box(&box_name_clone);

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
                    scroll_area.append(&no_apps_lbl);
                } else {
                    //TRANSLATORS: Window Title
                    loading_lbl.set_text(&gettext("Available Applications"));

                    let boxed_list = gtk::ListBox::new();
                    boxed_list.set_selection_mode(gtk::SelectionMode::None);
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
                            run_app_in_box(&app_clone, &box_name_clone);
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
                                    &box_name_clone,
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
                                    &box_name_clone,
                                    &loading_lbl_clone.clone(),
                                );
                            });
                            row.add_suffix(&add_menu_btn);
                        }

                        boxed_list.append(&row);
                        scroll_area.append(&boxed_list);
                    }
                }
            }
        }

        glib::ControlFlow::Continue
    });
}

fn add_app_to_menu(app: &DBoxApp, box_name: &str, success_lbl: &gtk::Label) {
    let _ = export_app_from_box(&app.name, box_name);
    //TRANSLATORS: Success Message
    success_lbl.set_text(&gettext("App Exported!"));
}

fn remove_app_from_menu(app: &DBoxApp, box_name: &str, success_lbl: &gtk::Label) {
    let _ = remove_app_from_host(&app.name, box_name);
    //TRANSLATORS: Success Message
    success_lbl.set_text(&gettext("App Removed!"));
}

fn run_app_in_box(app: &DBoxApp, box_name: &str) {
    run_command_in_box(&app.exec_name, box_name);
}

fn on_delete_clicked(window: &ApplicationWindow, box_name: String) {
    let d = adw::MessageDialog::new(
        Some(window),
        //TRANSLATORS: Confirmation Dialogue
        Some(&gettext("Really Delete?")),
        //TRANSLATORS: Confirmation Dialogue - {} replaced with the name of the Distrobox
        Some(&gettext!("Are you sure you want to delete {}?", box_name)),
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
            delete_box(&box_name);
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

    d.present();
}

fn on_clone_clicked(window: &ApplicationWindow, box_name: String) {
    let name_input_popup = gtk::Window::builder()
        .transient_for(window)
        .default_width(700)
        .default_height(250)
        .modal(true)
        .build();

    // TRANSLATORS: Heading Label - has box name appended
    let clone_prefix = &gettext("Clone");
    name_input_popup.set_title(Some(&format!("{} {}", clone_prefix, box_name.clone())));

    // TRANSLATORS: Button Label
    let create_btn = gtk::Button::with_label(&gettext("Clone"));
    create_btn.add_css_class("suggested-action");

    // TRANSLATORS: Button Label
    let cancel_btn = gtk::Button::with_label(&gettext("Cancel"));

    cancel_btn.connect_clicked(move |btn| {
        let win = btn.root().and_downcast::<gtk::Window>().unwrap();
        win.destroy();
    });

    let new_box_titlebar = adw::HeaderBar::new();
    new_box_titlebar.set_show_end_title_buttons(false);

    new_box_titlebar.pack_end(&create_btn);
    new_box_titlebar.pack_start(&cancel_btn);

    name_input_popup.set_titlebar(Some(&new_box_titlebar));

    let main_box = gtk::Box::new(Orientation::Vertical, 20);
    main_box.set_margin_start(10);
    main_box.set_margin_end(10);
    main_box.set_margin_top(10);
    main_box.set_margin_bottom(10);

    //TRANSLATORS: Title / Instruction label
    let title_label = gtk::Label::new(Some(&gettext("Enter the name of your new box")));
    title_label.add_css_class("title-2");

    let notice_label = gtk::Label::new(Some(&gettext(
        "Note: Cloning can take a long time, please be patient",
    )));

    let boxed_list = gtk::ListBox::new();
    boxed_list.set_selection_mode(gtk::SelectionMode::None);
    boxed_list.add_css_class("boxed-list");

    // name input
    let name_entry_row = adw::EntryRow::new();
    name_entry_row.set_hexpand(true);

    // TRANSLATORS: Entry Label - Name input for new distrobox
    name_entry_row.set_title(&gettext("Name"));

    let loading_spinner = gtk::Spinner::new();

    let loading_spinner_clone = loading_spinner.clone();
    let win_clone = window.clone();
    let ne_row = name_entry_row.clone();
    create_btn.connect_clicked(move |btn| {
        loading_spinner_clone.start();
        let mut name = ne_row.text().to_string();

        if name.is_empty() {
            return;
        }

        name = name.replace(' ', "-");
        let name_clone = name.clone();

        let (sender, receiver) =
            glib::MainContext::channel::<BoxCreatedMessage>(glib::Priority::DEFAULT);

        let bn = box_name.clone();
        thread::spawn(move || {
            clone_box(&bn, &name);
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
    main_box.append(&title_label);
    main_box.append(&boxed_list);
    main_box.append(&notice_label);
    main_box.append(&loading_spinner);

    name_input_popup.set_child(Some(&main_box));

    name_input_popup.present();
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
    let message_body = gettext("Distrobox can already access folders in your home directory - even if you have specified a custom home folder");
    let d = adw::MessageDialog::new(
        Some(window),
        //TRANSLATORS: Popup Heading
        Some(&gettext("Volume is already accessible")),
        Some(&message_body),
    );
    d.set_transient_for(Some(window));
    //TRANSLATORS: Button Label
    d.add_response("ok", &gettext("Ok"));
    d.set_default_response(Some("ok"));
    d.set_close_response("ok");

    d.present();
}

fn show_install_binary_popup(
    window: &ApplicationWindow,
    file_path: &str,
    pkg_type: BinaryPackageType,
) {
    let binary_file_type = match pkg_type {
        BinaryPackageType::Deb => ".deb",
        BinaryPackageType::Rpm => ".rpm",
    };

    let available_boxes = match pkg_type {
        BinaryPackageType::Deb => get_my_deb_boxes(),
        BinaryPackageType::Rpm => get_my_rpm_boxes(),
    };

    if available_boxes.is_empty() {
        //TRANSLATORS: Error / Info Message - {} replaced with .deb or .rpm
        let message_body = gettext!(
            "You don't appear to have any boxes which can install {} files",
            binary_file_type
        );
        let d = adw::MessageDialog::new(
            Some(window),
            //TRANSLATORS: Popup Heading
            Some(&gettext("No Suitable Boxes Found")),
            Some(&message_body),
        );
        d.set_transient_for(Some(window));
        //TRANSLATORS: Button Label
        d.add_response("ok", &gettext("Ok"));
        d.set_default_response(Some("ok"));
        d.set_close_response("ok");

        return d.present();
    }

    let install_binary_popup = gtk::Window::builder()
        // TRANSLATORS: Popup Window Title - {} replaced with .deb or .rpm
        .title(gettext!("Install {} File", binary_file_type))
        .transient_for(window)
        .default_width(700)
        .default_height(350)
        .modal(true)
        .build();

    // TRANSLATORS: Button Label
    let create_btn = gtk::Button::with_label(&gettext("Install"));
    create_btn.add_css_class("suggested-action");

    // TRANSLATORS: Button Label
    let cancel_btn = gtk::Button::with_label(&gettext("Cancel"));
    cancel_btn.connect_clicked(move |btn| {
        let win = btn.root().and_downcast::<gtk::Window>().unwrap();
        win.destroy();
    });

    let install_binary_titlebar = adw::HeaderBar::new();
    install_binary_titlebar.set_show_end_title_buttons(false);
    install_binary_titlebar.pack_end(&create_btn);
    install_binary_titlebar.pack_start(&cancel_btn);

    install_binary_popup.set_titlebar(Some(&install_binary_titlebar));

    let main_box = gtk::Box::new(Orientation::Vertical, 10);
    main_box.set_margin_start(10);
    main_box.set_margin_end(10);
    main_box.set_margin_top(10);
    main_box.set_margin_bottom(10);

    // TRANSLATORS: Info message - {} replaced with a file path
    let file_path_label = gtk::Label::new(Some(&gettext!("Installing: {}", file_path)));

    // TRANSLATORS: Help / Instruction text
    let instruction_label =
        gtk::Label::new(Some(&gettext("Select a box to install this file into:")));
    instruction_label.add_css_class("title-1");

    let boxes_refs: Vec<&str> = available_boxes.iter().map(|s| s as &str).collect();
    let exp = gtk::PropertyExpression::new(
        gtk::StringObject::static_type(),
        None::<gtk::Expression>,
        "string",
    );

    let boxes_dd = gtk::DropDown::from_strings(boxes_refs.as_slice());
    boxes_dd.set_expression(Some(exp));
    boxes_dd.set_enable_search(true);
    boxes_dd.set_search_match_mode(gtk::StringFilterMatchMode::Substring);
    boxes_dd.set_width_request(600);

    let boxes_dd_row = adw::ActionRow::new();
    // TRANSLATORS - Label for Dropdown of existing Boxes to install .deb or .rpm into
    boxes_dd_row.set_title(&gettext("Box"));
    boxes_dd_row.set_activatable_widget(Some(&boxes_dd));
    boxes_dd_row.add_suffix(&boxes_dd);

    let dd_clone = boxes_dd.clone();
    let bin_clone = file_path.to_string();
    let pt_clone = pkg_type;
    let popup_clone = install_binary_popup.clone();
    create_btn.connect_clicked(move |_btn| {
        let box_name = dd_clone
            .selected_item()
            .unwrap()
            .downcast::<gtk::StringObject>()
            .unwrap()
            .string()
            .to_string();

        if !box_name.is_empty() && !bin_clone.is_empty() {
            match pt_clone {
                BinaryPackageType::Deb => install_deb_in_box(box_name, bin_clone.clone()),
                BinaryPackageType::Rpm => install_rpm_in_box(box_name, bin_clone.clone()),
            }
            popup_clone.destroy();
        }
    });

    main_box.append(&instruction_label);
    main_box.append(&boxes_dd_row);
    main_box.append(&file_path_label);

    install_binary_popup.set_child(Some(&main_box));
    install_binary_popup.present();
}

fn on_install_deb_clicked(window: &ApplicationWindow, box_name: String) {
    let deb_filter = gtk::FileFilter::new();

    //TRANSLATORS: File type
    deb_filter.set_name(Some(&gettext("DEB Files")));
    deb_filter.add_mime_type("application/vnd.debian.binary-package");

    let download_dir = get_download_dir_path();

    let file_dialog = FileDialog::builder()
        .default_filter(&deb_filter)
        .initial_folder(&gio::File::for_path(download_dir))
        .modal(false)
        .build();
    file_dialog.open(
        Some(window),
        None::<&gio::Cancellable>,
        clone!(@weak window => move |result| {
            if let Ok(file) = result {
                let deb_path = file.path().unwrap().into_os_string().into_string();
                if deb_path.is_ok() {
                    let dp = deb_path.unwrap();
                    if dp.starts_with("/run/user") {
                        show_sandbox_access_popup(&window);
                    } else if !has_file_extension(&dp, "deb") {
                        show_incorrect_binary_file_popup(&window, BinaryPackageType::Deb);
                    } else {
                        install_deb_in_box(box_name, dp);
                    }
                }
            }
        }),
    );
}

fn on_install_rpm_clicked(window: &ApplicationWindow, box_name: String) {
    let rpm_filter = gtk::FileFilter::new();

    //TRANSLATORS: File type
    rpm_filter.set_name(Some(&gettext("RPM Files")));
    rpm_filter.add_mime_type("application/x-rpm");

    let download_dir = get_download_dir_path();

    let file_dialog = FileDialog::builder()
        .default_filter(&rpm_filter)
        .initial_folder(&gio::File::for_path(download_dir))
        .modal(false)
        .build();
    file_dialog.open(
        Some(window),
        None::<&gio::Cancellable>,
        clone!(@weak window => move |result| {
            if let Ok(file) = result {
                let rpm_path = file.path().unwrap().into_os_string().into_string();
                if rpm_path.is_ok() {
                    let rp = rpm_path.unwrap();
                    if rp.starts_with("/run/user") {
                        show_sandbox_access_popup(&window);
                    } else if !has_file_extension(&rp, "rpm") {
                        show_incorrect_binary_file_popup(&window, BinaryPackageType::Rpm);
                    } else {
                        install_rpm_in_box(box_name, rp);
                    }
                }
            }
        }),
    );
}

fn show_sandbox_access_popup(window: &ApplicationWindow) {
    //TRANSLATORS: Error / Info Message
    let message_body = gettext("This file is not accessible to Flatpak - please copy it to your Downloads folder, or allow filesystem access. Please see the <a href='https://dvlv.github.io/BoxBuddyRS/tips'>documentation for details.</a>");
    let d = adw::MessageDialog::new(
        Some(window),
        //TRANSLATORS: Popup Heading
        Some(&gettext("File Not Accessible")),
        Some(&message_body),
    );
    d.set_transient_for(Some(window));
    d.set_body_use_markup(true);
    //TRANSLATORS: Button Label
    d.add_response("ok", &gettext("Ok"));
    d.set_default_response(Some("ok"));
    d.set_close_response("ok");

    d.present();
}

fn show_incorrect_binary_file_popup(window: &ApplicationWindow, file_type: BinaryPackageType) {
    let pkg_type = match file_type {
        BinaryPackageType::Deb => ".deb",
        BinaryPackageType::Rpm => ".rpm",
    };
    //TRANSLATORS: Error / Info Message - {} replaced with .deb or .rpm
    let message_body = gettext!("This file does not appear to be a {} file", pkg_type);
    let d = adw::MessageDialog::new(
        Some(window),
        //TRANSLATORS: Popup Heading
        Some(&gettext("Incorrect File Type")),
        Some(&message_body),
    );
    d.set_transient_for(Some(window));
    //TRANSLATORS: Button Label
    d.add_response("ok", &gettext("Ok"));
    d.set_default_response(Some("ok"));
    d.set_close_response("ok");

    d.present();
}

fn show_preferred_terminal_popup(window: &ApplicationWindow) {
    let terms = get_supported_terminals();

    let settings = Settings::new(APP_ID);
    let default_term = settings.string("default-terminal");
    let mut selected_term_idx: u32 = 0;

    for (idx, term) in terms.iter().enumerate() {
        if term.name == default_term {
            selected_term_idx = idx as u32;
            break;
        }
    }

    let term_pref_popup = gtk::Window::builder()
        // TRANSLATORS: Popup Window Title
        .title(gettext("Preferred Terminal"))
        .transient_for(window)
        .default_width(500)
        .default_height(250)
        .modal(true)
        .build();

    // TRANSLATORS: Button Label
    let save_btn = gtk::Button::with_label(&gettext("Save"));
    save_btn.add_css_class("suggested-action");

    // TRANSLATORS: Button Label
    let cancel_btn = gtk::Button::with_label(&gettext("Cancel"));
    cancel_btn.connect_clicked(move |btn| {
        let win = btn.root().and_downcast::<gtk::Window>().unwrap();
        win.destroy();
    });

    let term_pref_titlebar = adw::HeaderBar::new();
    term_pref_titlebar.set_show_end_title_buttons(false);
    term_pref_titlebar.pack_end(&save_btn);
    term_pref_titlebar.pack_start(&cancel_btn);

    term_pref_popup.set_titlebar(Some(&term_pref_titlebar));

    let main_box = gtk::Box::new(gtk::Orientation::Vertical, 20);
    main_box.set_margin_top(20);

    // TRANSLATORS: Instructions label
    let instruction_label = gtk::Label::new(Some(&gettext("Select your preferred terminal")));
    instruction_label.add_css_class("title-2");

    let exp = gtk::PropertyExpression::new(
        gtk::StringObject::static_type(),
        None::<gtk::Expression>,
        "string",
    );

    let term_names_as_refs: Vec<&str> = terms.iter().map(|t| t.name.as_ref()).collect();
    let term_names_strlist = gtk::StringList::new(&term_names_as_refs);
    let terms_dropdown = gtk::DropDown::new(Some(term_names_strlist), Some(exp));

    terms_dropdown.set_selected(selected_term_idx);
    terms_dropdown.set_enable_search(true);
    terms_dropdown.set_search_match_mode(gtk::StringFilterMatchMode::Substring);
    terms_dropdown.set_width_request(400);

    let terms_dd_row = adw::ActionRow::new();
    // TRANSLATORS: Label for Dropdown of terminals available
    terms_dd_row.set_title(&gettext("Terminal"));
    terms_dd_row.set_activatable_widget(Some(&terms_dropdown));
    terms_dd_row.add_suffix(&terms_dropdown);

    let dd_clone = terms_dropdown.clone();
    let popup_clone = term_pref_popup.clone();
    let win_clone = window.clone();
    save_btn.connect_clicked(move |_btn| {
        let term_name = dd_clone
            .selected_item()
            .unwrap()
            .downcast::<gtk::StringObject>()
            .unwrap()
            .string()
            .to_string();

        let settings = gio::Settings::new(APP_ID);
        if settings
            .set_string("default-terminal", term_name.as_ref())
            .is_ok()
        {
            // TRANSLATORS: Success Message
            let toast = adw::Toast::new(&gettext("Terminal Preference Saved!"));
            if let Some(child) = win_clone.clone().child() {
                let toast_area = child.downcast::<ToastOverlay>();
                toast_area.unwrap().add_toast(toast);
            }

            popup_clone.destroy();

            delayed_rerender(&win_clone, None);
        } else {
            // TRANSLATORS: Error Message
            let toast = adw::Toast::new(&gettext("Sorry, Preference Could Not Be Saved"));
            if let Some(child) = win_clone.clone().child() {
                let toast_area = child.downcast::<ToastOverlay>();
                toast_area.unwrap().add_toast(toast);
            }

            popup_clone.destroy();

            delayed_rerender(&win_clone, None);
        }
    });

    main_box.append(&instruction_label);
    main_box.append(&terms_dd_row);

    term_pref_popup.set_child(Some(&main_box));
    term_pref_popup.present();
}
