use gtk::prelude::*;
use adw::{Application, ToastOverlay, ActionRow, prelude::{PreferencesRowExt, ActionRowExt}};
use gtk::{glib, ApplicationWindow, Orientation, Notebook, NotebookPage, NotebookTab, PositionType, Align};

mod distrobox_handler;
use distrobox_handler::{get_all_distroboxes, DBox};

const APP_ID: &str = "io.github.Dvlv.BoxBuddyRs";

fn main() -> glib::ExitCode {
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

    load_boxes(&window, &main_box);

    // Present window
    window.present();
}

fn make_titlebar(window: &ApplicationWindow) {
    let add_btn = gtk::Button::from_icon_name("list-add-symbolic");
    add_btn.set_tooltip_text(Some("Create A Distrobox"));
    add_btn.connect_clicked(|_btn| create_new_distrobox());

    let about_btn = gtk::Button::from_icon_name("help-about-symbolic");
    about_btn.set_tooltip_text(Some("About BoxBuddy"));
    about_btn.connect_clicked(|_btn| show_about_popup());

    let title_lbl = gtk::Label::new(Some("BoxBuddy"));
    title_lbl.add_css_class("header");

    let titlebar = adw::HeaderBar::builder()
        .title_widget(&title_lbl)
        .build();

    titlebar.pack_start(&add_btn);
    titlebar.pack_end(&about_btn);

    window.set_titlebar(Some(&titlebar))
}

fn load_boxes(_window: &ApplicationWindow, main_box: &gtk::Box) {
    let tabs = Notebook::new();
    tabs.set_tab_pos(PositionType::Left);
    tabs.set_hexpand(true);
    tabs.set_vexpand(true);

    let boxes = get_all_distroboxes();

    for dbox in boxes.iter() {
        let tab = make_box_tab(&dbox);
        // TODO shouldnt this be in make_box_tab
        tab.set_hexpand(true);
        tab.set_vexpand(true);

        let tab_title = gtk::Box::new(Orientation::Horizontal, 5);
        let tab_title_lbl = gtk::Label::new(Some(&dbox.name));
        let tab_title_img = gtk::Label::new(Some("img"));

        tab_title.append(&tab_title_img);
        tab_title.append(&tab_title_lbl);

        tabs.append_page(&tab, Some(&tab_title));
    }

    main_box.append(&tabs);

}

fn make_box_tab(dbox: &DBox) -> gtk::Box {
    let tab_box = gtk::Box::new(Orientation::Vertical, 15);
    tab_box.set_hexpand(true);

    tab_box.set_margin_top(10);
    tab_box.set_margin_bottom(10);
    tab_box.set_margin_start(10);
    tab_box.set_margin_end(10);

    //title
    let page_img = gtk::Label::new(Some("img"));
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
    open_terminal_button.connect_clicked(|_btn| on_open_terminal_clicked());

    let open_terminal_row = ActionRow::new();
    open_terminal_row.set_title("Open Terminal");
    open_terminal_row.add_suffix(&open_terminal_button);
    open_terminal_row.set_activatable_widget(Some(&open_terminal_button));

    // upgrade button
    let upgrade_button = gtk::Button::from_icon_name("software-update-available-symbolic");
    upgrade_button.add_css_class("flat");
    upgrade_button.connect_clicked(|_btn| on_upgrade_clicked());

    let upgrade_row = ActionRow::new();
    upgrade_row.set_title("Upgrade Box");
    upgrade_row.add_suffix(&upgrade_button);
    upgrade_row.set_activatable_widget(Some(&upgrade_button));

    // show applications button
    let show_applications_button = gtk::Button::from_icon_name("application-x-executable-symbolic");
    show_applications_button.add_css_class("flat");
    show_applications_button.connect_clicked(|_btn| on_show_applications_clicked());

    let show_applications_row = ActionRow::new();
    show_applications_row.set_title("View Applications");
    show_applications_row.add_suffix(&show_applications_button);
    show_applications_row.set_activatable_widget(Some(&show_applications_button));

    // Delete Button
    let delete_button = gtk::Button::from_icon_name("user-trash-symbolic");
    delete_button.add_css_class("flat");
    delete_button.connect_clicked(|_btn| on_delete_clicked());

    let delete_row = ActionRow::new();
    delete_row.set_title("Delete Box");
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
fn create_new_distrobox() {
    println!("Create new DB clicked");
}
fn show_about_popup() {
    println!("About clicked");
}
fn on_open_terminal_clicked() {
    println!("Open terminal clicked");
}
fn on_upgrade_clicked() {
    println!("Upgrade clicked");
}
fn on_show_applications_clicked() {
    println!("Show applications clicked");
}
fn on_delete_clicked() {
    println!("Delete clicked");
}