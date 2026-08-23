use gettextrs::gettext;
use std::cell::RefCell;
use std::rc::Rc;
use std::thread;

use adw::{
    prelude::*, ActionRow, Application, ApplicationWindow, Breakpoint, BreakpointCondition,
    BreakpointConditionLengthType, LengthUnit, NavigationPage, NavigationSplitView, NavigationView,
    ToastOverlay, ToolbarView, WindowTitle,
};
use gtk::{
    gio,
    gio::Settings,
    glib::{self},
    glib::{clone, markup_escape_text},
    Align, FileDialog, Orientation,
};

thread_local! {
    /// Long-lived handles to the pieces of the main window that a refresh needs
    /// to touch. The split view in particular is created exactly once, because a
    /// breakpoint keeps a reference to it for the lifetime of the window - so a
    /// refresh repopulates the sidebar and content in place rather than swapping
    /// the whole thing out.
    static MAIN_UI: RefCell<Option<MainUi>> = const { RefCell::new(None) };
}

#[derive(Clone)]
struct MainUi {
    window: ApplicationWindow,
    toast_overlay: ToastOverlay,
    split_view: NavigationSplitView,
    sidebar_list: gtk::ListBox,
    /// The content pane is a navigation view of its own: the selected box's page
    /// is its root, and views that belong to that box (its applications) are
    /// pushed on top of it rather than opened in separate windows.
    content_nav: NavigationView,
    content_page: NavigationPage,
    content_scroll: gtk::ScrolledWindow,
    /// The boxes currently shown in the sidebar, indexed the same way the rows
    /// are, so the row-selected handler can find the box a row stands for.
    boxes: Rc<RefCell<Vec<DBox>>>,
}

mod distrobox_handler;
use distrobox_handler::{
    assemble_box, clone_box, create_box, create_box_streaming, delete_box, export_app_from_box, get_all_distroboxes,
    get_apps_in_box, get_available_images_with_distro_name, get_binaries_exported_from_box,
    get_number_of_boxes, install_deb_in_box, install_rpm_in_box, open_terminal_in_box,
    remove_app_from_host, remove_exported_binary_from_box, run_command_in_box, stop_box,
    upgrade_all_boxes, upgrade_box, DBox, DBoxApp,
};

mod utils;
use utils::{
    get_available_app_icon_name, get_available_icon_name, get_cpu_and_mem_usage, get_deb_distros,
    get_distro_img, get_download_dir_path, get_my_deb_boxes, get_my_rpm_boxes, get_rpm_distros,
    get_supported_terminals, get_supported_terminals_list, get_terminal_and_separator_arg,
    has_distrobox_installed, has_file_extension, has_host_access, has_podman_or_docker_installed,
    set_up_localisation, ADD_ICON_NAMES, COPY_ICON_NAMES, INFO_ICON_NAMES,
    INSTALL_PACKAGE_ICON_NAMES, MENU_ICON_NAMES, OPEN_FILE_ICON_NAMES, REMOVE_ICON_NAMES,
    STOP_ICON_NAMES, TERMINAL_ICON_NAMES, TRASH_ICON_NAMES, UPGRADE_ICON_NAMES, WARNING_ICON_NAMES,
};
const APP_ID: &str = "io.github.dvlv.boxbuddyrs";

enum AppsFetchMessage {
    AppsFetched(Vec<DBoxApp>, Vec<String>),
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

    // Both dependencies are probed up front: the result decides what the
    // window shows, and also which header buttons are worth offering.
    let has_distrobox = has_distrobox_installed();
    let has_container_engine = has_podman_or_docker_installed();

    // The sidebar lists the boxes; its header carries the global actions that
    // used to live in the window titlebar (create, assemble, upgrade, menu).
    let sidebar_list = gtk::ListBox::new();
    sidebar_list.add_css_class("navigation-sidebar");
    sidebar_list.set_selection_mode(gtk::SelectionMode::Single);

    let sidebar_scroll = gtk::ScrolledWindow::new();
    sidebar_scroll.set_vexpand(true);
    sidebar_scroll.set_child(Some(&sidebar_list));

    let sidebar_toolbar = ToolbarView::new();
    sidebar_toolbar
        .add_top_bar(&build_main_headerbar(&window, has_distrobox && has_container_engine));
    sidebar_toolbar.set_content(Some(&sidebar_scroll));

    let sidebar_page = NavigationPage::new(&sidebar_toolbar, &gettext("Boxes"));

    // The content pane shows the selected box. Its own header gives it a title
    // and, once the split view collapses on a narrow window, a back button.
    let content_scroll = gtk::ScrolledWindow::new();
    content_scroll.set_vexpand(true);
    content_scroll.set_hexpand(true);

    let content_toolbar = ToolbarView::new();
    content_toolbar.add_top_bar(&adw::HeaderBar::new());
    content_toolbar.set_content(Some(&content_scroll));

    let content_page = NavigationPage::new(&content_toolbar, "BoxBuddy");

    // One level of browsing below the box: pages about the selected box (its
    // applications) are pushed here, with the header's back button to return.
    let content_nav = NavigationView::new();
    content_nav.add(&content_page);
    let content_root = NavigationPage::new(&content_nav, "BoxBuddy");

    let split_view = NavigationSplitView::new();
    split_view.set_min_sidebar_width(200.0);
    split_view.set_max_sidebar_width(280.0);
    split_view.set_sidebar(Some(&sidebar_page));
    split_view.set_content(Some(&content_root));

    let toast_overlay = ToastOverlay::new();
    toast_overlay.set_child(Some(&split_view));
    window.set_content(Some(&toast_overlay));

    // Mobile-first: below a narrow width the split view folds into a single
    // pane - the box list is the root, and selecting a box pushes to it.
    let condition = BreakpointCondition::new_length(
        BreakpointConditionLengthType::MaxWidth,
        500.0,
        LengthUnit::Sp,
    );
    let breakpoint = Breakpoint::new(condition);
    breakpoint.add_setter(&split_view, "collapsed", Some(&true.to_value()));
    window.add_breakpoint(breakpoint);

    let ui = MainUi {
        window: window.clone(),
        toast_overlay: toast_overlay.clone(),
        split_view: split_view.clone(),
        sidebar_list: sidebar_list.clone(),
        content_nav: content_nav.clone(),
        content_page: content_page.clone(),
        content_scroll: content_scroll.clone(),
        boxes: Rc::new(RefCell::new(Vec::new())),
    };

    // Selecting a box swaps the content pane to it. Wired once, because the
    // sidebar list lives as long as the window; a refresh only repopulates it.
    let handler_ui = ui.clone();
    sidebar_list.connect_row_selected(move |_list, row| {
        let Some(row) = row else { return };
        let idx = row.index();
        if idx < 0 {
            return;
        }
        let boxes = handler_ui.boxes.borrow();
        let Some(dbox) = boxes.get(idx as usize) else {
            return;
        };
        let detail = make_box_tab(dbox, &handler_ui.window, idx as u32);
        handler_ui.content_scroll.set_child(Some(&detail));
        handler_ui.content_page.set_title(&dbox.name);
        // A page pushed for the previous box (its applications) is about that
        // box, so switching boxes comes back to the box page first.
        handler_ui.content_nav.pop_to_page(&handler_ui.content_page);
        handler_ui.split_view.set_show_content(true);
    });

    MAIN_UI.with(|cell| *cell.borrow_mut() = Some(ui));

    set_window_actions(&window);
    render_main_content(&window, Some(0));

    window.present();

    window
}

fn build_ui(app: &Application) {
    // Create a window and set the title
    let window = make_window(app);

    let (term, _, _) = get_terminal_and_separator_arg();
    if term.is_empty() {
        show_no_supported_terminal_popup(&window);
    }
}

fn build_ui_as_open(app: &Application, files: &[gio::File], _hint: &str) {
    let window = make_window(app);

    if !files.is_empty() {
        // BoxBuddy will only support opening one file at a time for now.
        // Bail out silently if anything is missing: an empty gio::File, a path
        // the host cannot represent, or a non-UTF-8 path. Falling through
        // here just means the user launched BoxBuddy without a usable file,
        // which is a normal startup; there is no error to surface.
        let Some(first_file) = files.first() else {
            return;
        };
        let Some(file_path) = first_file.path() else {
            return;
        };
        let Some(file_path_str) = file_path.to_str() else {
            return;
        };

        if has_file_extension(file_path_str, "rpm") {
            show_install_binary_popup(&window, file_path_str, BinaryPackageType::Rpm);
        } else if has_file_extension(file_path_str, "deb") {
            show_install_binary_popup(&window, file_path_str, BinaryPackageType::Deb);
        }
    }

    // if file not recognised we COULD show a "not recognised" message, but
    // possibly better to just let BoxBuddy run as if there were no file
}

/// Builds the image for the Assemble button. The icon is one BoxBuddy ships
/// itself rather than one from the icon theme, so it is loaded by path - and a
/// path which is not there leaves `gtk::Image` drawing a broken-image
/// placeholder, which is what the 2.6.0 Flatpak does since it contains no
/// `/app/icons` directory at all. Fall back on a themed icon instead, so the
/// button stays recognisable however BoxBuddy was packaged.
fn build_main_headerbar(window: &ApplicationWindow, dependencies_met: bool) -> adw::HeaderBar {
    // One "new" control with its variants behind it, as the HIG suggests for
    // actions that come in flavours: a box from the form, or boxes assembled
    // from a distrobox.ini manifest.
    let new_menu = gio::Menu::new();
    // TRANSLATORS: Menu Item under the "+" button - opens the create-box form
    new_menu.append(Some(&gettext("New Box…")), Some("win.new-box"));
    // TRANSLATORS: Menu Item under the "+" button - picks a distrobox.ini to assemble
    new_menu.append(Some(&gettext("Assemble from File…")), Some("win.assemble"));

    let add_btn = gtk::MenuButton::new();
    add_btn.set_icon_name(&get_available_icon_name(ADD_ICON_NAMES));
    add_btn.set_menu_model(Some(&new_menu));
    // TRANSLATORS: Button tooltip
    add_btn.set_tooltip_text(Some(&gettext("Create A Distrobox")));

    let upgrade_btn = gtk::Button::from_icon_name(&get_available_icon_name(UPGRADE_ICON_NAMES));
    // TRANSLATORS: Button tooltip
    upgrade_btn.set_tooltip_text(Some(&gettext("Upgrade All Boxes")));
    upgrade_btn.connect_clicked(move |_btn| upgrade_all_boxes());

    let menu_btn = gtk::MenuButton::new();
    menu_btn.set_icon_name(&get_available_icon_name(MENU_ICON_NAMES));
    menu_btn.set_menu_model(Some(&get_main_menu_model()));
    //TRANSLATORS: Button tooltip
    menu_btn.set_tooltip_text(Some(&gettext("Menu")));

    // Creating and upgrading shell out to distrobox, which in turn needs a
    // container engine, so neither can do anything useful while either is
    // missing. The menu stays available: it holds Refresh, Preferences and
    // About, none of which touch distrobox.
    add_btn.set_sensitive(dependencies_met);
    upgrade_btn.set_sensitive(dependencies_met);

    let titlebar = adw::HeaderBar::new();

    titlebar.pack_start(&add_btn);
    titlebar.pack_end(&menu_btn);
    titlebar.pack_end(&upgrade_btn);

    let _ = window;
    titlebar
}

/// Picks a distrobox.ini and assembles the boxes it describes.
fn assemble_from_file(window: &ApplicationWindow) {
    let ini_filter = gtk::FileFilter::new();

    //TRANSLATORS: File type
    ini_filter.set_name(Some(&gettext("INI-Files")));
    ini_filter.add_mime_type("text/plain".as_ref());
    ini_filter.add_mime_type("application/textedit".as_ref());
    ini_filter.add_mime_type("application/zz-winassoc-ini".as_ref());

    let file_dialog = FileDialog::builder()
        .default_filter(&ini_filter)
        .modal(false)
        .build();
    file_dialog.open(
        Some(window),
        None::<&gio::Cancellable>,
        clone!(@weak window => move |result| {
            if let Ok(file) = result {
                let ini_path = file.path().unwrap().into_os_string().into_string();
                if ini_path.is_ok() {
                    assemble_new_distrobox(&window, ini_path.unwrap());
                }
            }
        }),
    );
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

    let action_preferences = gio::ActionEntry::builder("preferences")
        .activate(|window: &ApplicationWindow, _, _| {
            show_preferences(window);
        })
        .build();

    let action_new_box = gio::ActionEntry::builder("new-box")
        .activate(|window: &ApplicationWindow, _, _| {
            create_new_distrobox(window);
        })
        .build();

    let action_assemble = gio::ActionEntry::builder("assemble")
        .activate(|window: &ApplicationWindow, _, _| {
            assemble_from_file(window);
        })
        .build();

    window.add_action_entries([
        action_refresh,
        action_about,
        action_close,
        action_preferences,
        action_new_box,
        action_assemble,
    ]);
}

fn get_main_menu_model() -> gio::MenuModel {
    // Massive thanks to https://blog.libove.org/posts/rust-gtk--creating-a-menu-bar-programmatically-with-gtk-rs/
    // Laid out the way the HIG describes a primary menu: the app's own items
    // first, the standard Preferences / About group at the end, and no Quit -
    // that is what Ctrl+Q and the window's close button are for.
    let menu = gio::Menu::new();

    let app_section = gio::Menu::new();
    //TRANSLATORS: Menu Item
    app_section.append(Some(&gettext("Refresh")), Some("win.refresh"));
    menu.append_section(None, &app_section);

    let standard_section = gio::Menu::new();
    //TRANSLATORS: Menu Item
    standard_section.append(Some(&gettext("Preferences")), Some("win.preferences"));
    //TRANSLATORS: Menu Item
    standard_section.append(Some(&gettext("About BoxBuddy")), Some("win.about"));
    menu.append_section(None, &standard_section);

    menu.into()
}

/// Stops a box name from dictating how wide the window has to be.
///
/// The tab strip and the page it opens both sit inside a scroller, which sizes
/// itself to the smallest width its child can live with - so a label showing a
/// name of any length makes that name the minimum width of the whole window.
/// Names within `max_chars` are left exactly as they were; a longer one gets
/// ellipsized, and the tooltip carries the full text either way.
fn cap_label_width(label: &gtk::Label, name: &str, max_chars: i32) {
    label.set_tooltip_text(Some(name));

    if i32::try_from(name.chars().count()).unwrap_or(i32::MAX) > max_chars {
        label.set_ellipsize(gtk::pango::EllipsizeMode::End);
        label.set_max_width_chars(max_chars);
        // The cap on its own only limits what the label asks for; an ellipsizing
        // label is also happy to shrink all the way down to a lone "...", which
        // would let the tab strip collapse to nothing. Half the cap gives it a
        // floor to stop at while still letting it give ground when the window is
        // too narrow for the full width.
        label.set_width_chars(max_chars / 2);
    }
}

/// Builds the status page shown when one of BoxBuddy's dependencies is missing.
fn build_not_installed_status_page(title: &str, body: &str) -> adw::StatusPage {
    let status_page = adw::StatusPage::new();
    status_page.set_icon_name(Some(&get_available_icon_name(WARNING_ICON_NAMES)));
    status_page.set_title(title);
    status_page.set_description(Some(body));
    // The scroll area only hands out spare height to children which ask for it,
    // so without this the status page would sit at the top instead of centring.
    status_page.set_vexpand(true);

    status_page
}

/// Decides what the window shows for the current dependency and box state, and
/// puts it on screen. This is the single entry point both the initial open and
/// every refresh go through.
fn render_main_content(window: &ApplicationWindow, active_page: Option<u32>) {
    let Some(ui) = MAIN_UI.with(|cell| cell.borrow().clone()) else {
        return;
    };

    // A missing dependency takes over the whole window - there is no box list to
    // put in a sidebar - so it replaces the split view with a single status page
    // whose header still offers the (disabled) actions and the menu.
    if !has_distrobox_installed() {
        show_full_page_status(
            window,
            &ui,
            &build_not_installed_status_page(
                // TRANSLATORS: Error message shown when distrobox is not installed
                &gettext("Distrobox not found!"),
                // TRANSLATORS: Error message shown when distrobox is not installed
                &gettext("Distrobox could not be found, please ensure it is installed!"),
            ),
        );
        return;
    }

    if !has_podman_or_docker_installed() {
        show_full_page_status(
            window,
            &ui,
            &build_not_installed_status_page(
                // TRANSLATORS: Error message shown when neither podman nor docker is installed
                &gettext("Podman / Docker not found!"),
                // TRANSLATORS: Error message shown when neither podman nor docker is installed
                &gettext("Could not find podman or docker, please install one of them!"),
            ),
        );
        return;
    }

    populate_boxes(&ui, active_page);
}

/// Swaps the whole window over to a single status page (missing dependency),
/// taking the split view off screen. The status page keeps the split view's
/// object alive, so the breakpoint attached to it stays valid.
fn show_full_page_status(
    window: &ApplicationWindow,
    ui: &MainUi,
    status_page: &adw::StatusPage,
) {
    let toolbar = ToolbarView::new();
    toolbar.add_top_bar(&build_main_headerbar(window, false));
    toolbar.set_content(Some(status_page));
    ui.toast_overlay.set_child(Some(&toolbar));
}

/// Fills the sidebar with the current boxes and shows the one at `active_page`,
/// re-using the long-lived split view rather than rebuilding it.
fn populate_boxes(ui: &MainUi, active_page: Option<u32>) {
    // A refresh may be recovering from a missing-dependency screen, so make sure
    // the split view is what is on screen again.
    let shown = ui.toast_overlay.child();
    if shown.as_ref() != Some(ui.split_view.upcast_ref::<gtk::Widget>()) {
        ui.toast_overlay.set_child(Some(&ui.split_view));
    }

    // Clearing drops each row; the row-selected handler ignores the resulting
    // "nothing selected", so the content pane keeps its last child until the new
    // selection below replaces it. Anything pushed above the box page was built
    // from the old list, so it goes too.
    ui.content_nav.pop_to_page(&ui.content_page);
    while let Some(row) = ui.sidebar_list.first_child() {
        ui.sidebar_list.remove(&row);
    }

    let boxes = get_all_distroboxes();

    if boxes.is_empty() {
        *ui.boxes.borrow_mut() = boxes;
        ui.content_page.set_title("BoxBuddy");
        ui.content_scroll.set_child(Some(&build_no_boxes_page()));
        // Show the sidebar (with its create button) rather than the empty pane.
        ui.split_view.set_show_content(false);
        return;
    }

    for dbox in &boxes {
        ui.sidebar_list.append(&build_sidebar_row(dbox));
    }
    // The handler reads this to map the selected row back to a box, so it has to
    // be current before the selection below fires.
    *ui.boxes.borrow_mut() = boxes;

    let idx = i32::try_from(active_page.unwrap_or(0)).unwrap_or(0);
    let row = ui
        .sidebar_list
        .row_at_index(idx)
        .or_else(|| ui.sidebar_list.row_at_index(0));
    ui.sidebar_list.select_row(row.as_ref());
}

/// The centred "you have no boxes yet" placeholder for the content pane.
fn build_no_boxes_page() -> adw::StatusPage {
    let status_page = adw::StatusPage::new();
    // TRANSLATORS: Error Message
    status_page.set_title(&gettext("No Boxes"));
    // TRANSLATORS: Instructions
    status_page.set_description(Some(&gettext(
        "Click the + at the top-left to create your first box!",
    )));
    status_page.set_vexpand(true);

    status_page
}

/// One sidebar entry: the distro's coloured dot and the box name.
fn build_sidebar_row(dbox: &DBox) -> gtk::ListBoxRow {
    let row = gtk::ListBoxRow::new();

    let row_box = gtk::Box::new(Orientation::Horizontal, 10);
    row_box.set_margin_top(6);
    row_box.set_margin_bottom(6);
    row_box.set_margin_start(6);
    row_box.set_margin_end(6);

    let img = gtk::Label::new(None);
    img.set_markup(&get_distro_img(&dbox.distro));

    let name = gtk::Label::new(Some(&dbox.name));
    name.set_halign(Align::Start);
    cap_label_width(&name, &dbox.name, 20);

    row_box.append(&img);
    row_box.append(&name);
    row.set_child(Some(&row_box));

    row
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
    // As on the tab, a long name has to give way rather than shove the status
    // label and the Stop button off the end of the row.
    cap_label_width(&page_title, &dbox.name, 30);

    let page_status = gtk::Label::new(Some(&dbox.status));
    page_status.set_halign(Align::End);
    page_status.set_hexpand(true);

    let stop_btn = gtk::Button::from_icon_name(&get_available_icon_name(STOP_ICON_NAMES));
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
    let open_terminal_icon =
        gtk::Image::from_icon_name(&get_available_icon_name(TERMINAL_ICON_NAMES));

    let open_terminal_row = ActionRow::new();
    // TRANSLATORS: Row Label
    open_terminal_row.set_title(&gettext("Open Terminal"));
    open_terminal_row.add_suffix(&open_terminal_icon);
    open_terminal_row.set_activatable(true);

    let term_bn_clone = box_name.clone();
    open_terminal_row
        .connect_activated(move |_row| on_open_terminal_clicked(term_bn_clone.clone()));

    // Upgrade Icon
    let upgrade_icon = gtk::Image::from_icon_name(&get_available_icon_name(UPGRADE_ICON_NAMES));

    let upgrade_row = ActionRow::new();
    // TRANSLATORS: Row Label
    upgrade_row.set_title(&gettext("Upgrade Box"));
    upgrade_row.add_suffix(&upgrade_icon);
    upgrade_row.set_activatable(true);

    let up_bn_clone = box_name.clone();
    upgrade_row.connect_activated(move |_row| on_upgrade_clicked(&up_bn_clone));

    // Applications come first, in an island of their own: a box's apps are what
    // it is for, while the rows below act on the container itself. The row is a
    // link to the applications page, so it carries the go-next arrow the HIG
    // gives rows that lead to another view.
    let apps_list = gtk::ListBox::new();
    apps_list.set_selection_mode(gtk::SelectionMode::None);
    apps_list.add_css_class("boxed-list");

    let show_applications_row = ActionRow::new();
    // TRANSLATORS: Row Label - opens the page listing the box's applications
    show_applications_row.set_title(&gettext("Applications"));
    show_applications_row.add_suffix(&gtk::Image::from_icon_name("go-next-symbolic"));
    show_applications_row.set_activatable(true);

    let show_dbox = dbox.clone();
    show_applications_row.connect_activated(move |_row| {
        on_show_applications_clicked(show_dbox.clone());
    });
    apps_list.append(&show_applications_row);

    // Delete Icon
    let delete_icon = gtk::Image::from_icon_name(&get_available_icon_name(TRASH_ICON_NAMES));

    let delete_row = ActionRow::new();
    //TRANSLATORS: Row Label
    delete_row.set_title(&gettext("Delete Box"));
    delete_row.add_suffix(&delete_icon);
    delete_row.set_activatable(true);

    let del_bn_clone = box_name.clone();
    let win_clone = window.clone();
    delete_row.connect_activated(move |_row| on_delete_clicked(&win_clone, del_bn_clone.clone()));

    // Clone Box Icon
    let clone_icon = gtk::Image::from_icon_name(&get_available_icon_name(COPY_ICON_NAMES));

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
    boxed_list.append(&clone_row);
    boxed_list.append(&delete_row);

    tab_box.append(&title_box);
    tab_box.append(&gtk::Separator::new(Orientation::Horizontal));
    tab_box.append(&apps_list);
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

    let (sender, receiver) = async_channel::bounded(1);

    gio::spawn_blocking(move || {
        assemble_box(&ini_file);
        sender
            .send_blocking(BoxCreatedMessage::Success)
            .expect("The channel needs to be open.");
    });

    let ls_clone = loading_spinner.clone();
    let w_clone = window.clone();
    let popup = assemble_box_popup.clone();

    glib::spawn_future_local(clone!(
        #[weak]
        popup,
        async move {
            while let Ok(msg) = receiver.recv().await {
                match msg {
                    BoxCreatedMessage::Success => {
                        ls_clone.stop();

                        let num_boxes = get_number_of_boxes();
                        delayed_rerender(&w_clone, Some(num_boxes - 1));
                        popup.destroy();
                    }
                }
            }
        }
    ));
}

/// Show a dialog with a `gtk::TextView` that fills with stdout/stderr from a
/// running `distrobox create`. The dialog stays open until the underlying
/// process reports completion, at which point it auto-destroys itself after
/// a short pause so the user can read the final lines.
///
/// The dialog is read-only and intentionally decoupled from the actual create
/// flow: it exists so the user has something to look at while a 30-second-to-
/// several-minute container build is running, instead of staring at a tiny
/// spinner.
///
/// `line_rx` is fed by the streaming `create_box_streaming`; we drain it on a
/// short GLib timer so the producer thread does not need to know anything
/// about GTK. `done_rx` is a one-shot signal that the producer is done;
/// `on_done` runs once at the very end on the GLib main loop.
fn show_create_output_stream_dialog<F>(
    window: &ApplicationWindow,
    box_name: &str,
    line_rx: std::sync::mpsc::Receiver<String>,
    done_rx: std::sync::mpsc::Receiver<()>,
    on_done: F,
) where
    F: Fn() + 'static,
{
    let popup = gtk::Window::builder()
        // TRANSLATORS: Window Title - showing live output of a container creation
        .title(gettext("Creating container…"))
        .transient_for(window)
        .default_width(720)
        .default_height(360)
        .modal(true)
        .build();

    let titlebar = adw::HeaderBar::new();
    let title_lbl = gtk::Label::new(Some(&gettext("Creating container…")));
    titlebar.set_title_widget(Some(&title_lbl));

    let main_box = gtk::Box::new(Orientation::Vertical, 8);
    main_box.set_margin_start(10);
    main_box.set_margin_end(10);
    main_box.set_margin_top(10);
    main_box.set_margin_bottom(10);

    // TRANSLATORS: Status label above the streaming textview
    let status_lbl = gtk::Label::new(Some(&gettext(
        "Streaming output of `distrobox create`…",
    )));
    status_lbl.set_xalign(0.0);

    let text_view = gtk::TextView::new();
    text_view.set_editable(false);
    text_view.set_monospace(true);
    text_view.set_top_margin(6);
    text_view.set_bottom_margin(6);
    text_view.set_left_margin(6);
    text_view.set_right_margin(6);
    let buffer = text_view.buffer();
    buffer.set_text("");

    let scrolled = gtk::ScrolledWindow::new();
    scrolled.set_vexpand(true);
    scrolled.set_hexpand(true);
    scrolled.set_child(Some(&text_view));

    // Spinner we hide once real output arrives. Keeps the dialog honest
    // while distrobox is still in the very first few seconds.
    let spinner = gtk::Spinner::new();
    spinner.start();

    main_box.append(&status_lbl);
    main_box.append(&spinner);
    main_box.append(&scrolled);

    popup.set_child(Some(&main_box));
    popup.set_titlebar(Some(&titlebar));
    popup.present();

    let popup_for_poll = popup.clone();
    let status_for_poll = status_lbl.clone();
    let spinner_for_poll = spinner.clone();
    let buffer_for_poll = buffer.clone();
    let empty_marker_count = std::cell::Cell::new(0u8);
    // We need to call `on_done` exactly once when polling finishes. The
    // polling closure is `FnMut` and re-runs until we Break, so it cannot
    // own an `FnOnce` directly. The trick is to hold it inside a
    // Mutex<Option> and `take` it once the producer is done.
    let on_done_slot: std::sync::Arc<std::sync::Mutex<Option<F>>> =
        std::sync::Arc::new(std::sync::Mutex::new(Some(on_done)));

    glib::timeout_add_local(std::time::Duration::from_millis(80), move || {
        loop {
            match line_rx.try_recv() {
                Ok(line) => {
                    if line.is_empty() {
                        empty_marker_count.set(empty_marker_count.get() + 1);
                        continue;
                    }
                    if spinner_for_poll.is_visible() {
                        spinner_for_poll.set_visible(false);
                    }
                    let mut end = buffer_for_poll.end_iter();
                    buffer_for_poll.insert(&mut end, &format!("{line}\n"));
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => break,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => break,
            }
        }

        let producer_done = matches!(
            done_rx.try_recv(),
            Ok(()) | Err(std::sync::mpsc::TryRecvError::Disconnected)
        );
        if producer_done && empty_marker_count.get() >= 2 {
            status_for_poll.set_text(&gettext("Done."));
            let mut slot = on_done_slot.lock().unwrap();
            let on_done = slot.take();
            drop(slot);
            let popup_for_close = popup_for_poll.clone();
            if let Some(on_done) = on_done {
                glib::timeout_add_local_once(
                    std::time::Duration::from_millis(700),
                    move || {
                        popup_for_close.destroy();
                        on_done();
                    },
                );
            } else {
                popup_for_close.destroy();
            }
            return glib::ControlFlow::Break;
        }

        glib::ControlFlow::Continue
    });

    // box_name is currently only used for the window title; keeping the
    // parameter for symmetry with the rest of BoxBuddy's dialog functions
    // and to leave room for adding a per-box header chip later.
    let _ = box_name;
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
    create_btn.set_sensitive(false);

    let info_btn = gtk::Button::from_icon_name(&get_available_icon_name(INFO_ICON_NAMES));
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

    new_box_titlebar.pack_end(&info_btn);

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

    // name input must have text in it to enable the create button
    let ner_clone = name_entry_row.clone();
    name_entry_row.connect_changed(clone!(@weak create_btn => move |_row| {
        if ner_clone.text().to_string().len() > 0 {
            create_btn.set_sensitive(true);
        } else {
            create_btn.set_sensitive(false);
        }
    }));

    // TRANSLATORS: Entry Label - Name input for new distrobox
    name_entry_row.set_title(&gettext("Name"));

    // custom home
    let choose_home_btn =
        gtk::Button::from_icon_name(&get_available_icon_name(OPEN_FILE_ICON_NAMES));
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

    // hostname
    let hostname_entry_row = adw::EntryRow::new();
    hostname_entry_row.set_hexpand(true);
    // TRANSLATORS: Entry Label - Custom hostname for the new distrobox
    hostname_entry_row.set_title(&gettext("Hostname (Leave blank for default)"));
    // TRANSLATORS: Help text explaining what distrobox uses when no hostname is given
    hostname_entry_row.set_tooltip_text(Some(&gettext(
        "Defaults to the box name followed by your machine's hostname",
    )));

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
    // TRANSLATORS: Explanation of what the 'use init system' toggle does
    init_row.set_subtitle(&gettext("Adds systemd support - ignore if you're not sure"));
    init_row.set_active(false);

    let loading_spinner = gtk::Spinner::new();

    let home_row = home_entry_row_future_clone.clone();
    let hn_row = hostname_entry_row.clone();
    let ne_row = name_entry_row.clone();
    let is_row = image_select_row.clone();
    let in_row = init_row.clone();
    let loading_spinner_clone = loading_spinner.clone();
    let win_clone = window.clone();
    let volume_box_list_clone = volume_box_list.clone();
    create_btn.connect_clicked(move |btn| {
        let mut name = ne_row.text().to_string();
        let mut home_path = home_row.text().to_string();
        let mut hostname = hn_row.text().to_string();
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

        loading_spinner_clone.start();

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
        hostname = hostname.trim().replace(' ', "-");
        home_path = home_path.replace(' ', "\\ "); //Escape spaces
        image = image.split(" - ").last().unwrap().to_string();
        image = image.replace(" ✦ ", "");

        let name_clone = name.clone();

        // Two channels: one carries `distrobox create` output lines (fed into
        // a TextView so the user sees progress), the other is a one-shot
        // signal that the underlying process has finished.
        let (line_tx, line_rx) = std::sync::mpsc::channel::<String>();
        let (done_tx, done_rx) = std::sync::mpsc::channel::<()>();

        gio::spawn_blocking(move || {
            create_box_streaming(
                &name,
                &image,
                &home_path,
                &hostname,
                use_init,
                volumes.as_slice(),
                line_tx,
            );
            let _ = done_tx.send(());
        });

        let b_clone = btn.clone();
        let w_clone = win_clone.clone();

        // Show the streaming output dialog. It self-destroys once both
        // stream threads have sent their terminator empty line AND the
        // producer has disconnected.
        let w_clone_for_dialog = w_clone.clone();
        let name_clone_for_dialog = name_clone.clone();
        show_create_output_stream_dialog(&w_clone_for_dialog, &name_clone_for_dialog, line_rx, done_rx, move || {
            let win = b_clone.root().and_downcast::<gtk::Window>().unwrap();
            win.destroy();

            let num_boxes = get_number_of_boxes();
            delayed_rerender(&w_clone, Some(num_boxes - 1));

            open_terminal_in_box(name_clone.clone());
        });
    });

    boxed_list.append(&name_entry_row);
    boxed_list.append(&image_select_row);
    boxed_list.append(&init_row);

    boxed_list.append(&home_select_row);
    boxed_list.append(&hostname_entry_row);

    main_box.append(&boxed_list);

    //Volumes
    if has_host_access() {
        let volume_box_list_clone = volume_box_list.clone();

        let volume_add_btn = gtk::Button::from_icon_name(&get_available_icon_name(ADD_ICON_NAMES));
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
                        let volume_remove_btn = gtk::Button::from_icon_name(&get_available_icon_name(REMOVE_ICON_NAMES));
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
    d.set_version("2.6.0");
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
Murat-Karakaya - TR
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

/// A centred placeholder for a page with nothing to show, so the message sits
/// in the middle of the space rather than clinging to the top of it.
fn build_empty_state_page(title: &str) -> adw::StatusPage {
    let status_page = adw::StatusPage::new();
    status_page.set_title(title);
    status_page.set_vexpand(true);

    status_page
}

/// The "Install .deb/.rpm File" row for a box whose distro takes one of those
/// package formats, or None when it takes neither.
fn build_install_package_row(window: &ApplicationWindow, dbox: &DBox) -> Option<ActionRow> {
    let (title, handler): (String, fn(&ApplicationWindow, String, String)) =
        if get_deb_distros().contains(&dbox.distro) {
            // TRANSLATORS: Row Label
            (gettext("Install .deb File"), on_install_deb_clicked)
        } else if get_rpm_distros().contains(&dbox.distro) {
            // TRANSLATORS: Row Label
            (gettext("Install .rpm File"), on_install_rpm_clicked)
        } else {
            return None;
        };

    let row = ActionRow::new();
    row.set_title(&title);
    row.add_suffix(&gtk::Image::from_icon_name(&get_available_icon_name(
        INSTALL_PACKAGE_ICON_NAMES,
    )));
    row.set_activatable(true);

    let win_clone = window.clone();
    let box_name = dbox.name.clone();
    let box_image = dbox.image_url.clone();
    row.connect_activated(move |_row| handler(&win_clone, box_name.clone(), box_image.clone()));

    Some(row)
}

/// Pushes the box's applications onto the content pane as a page of its own.
/// The box page stays underneath and the header's back button returns to it -
/// which also works once the split view has collapsed on a narrow window.
fn on_show_applications_clicked(dbox: DBox) {
    let Some(ui) = MAIN_UI.with(|cell| cell.borrow().clone()) else {
        return;
    };
    let box_name = dbox.name.clone();

    let header = adw::HeaderBar::new();
    header.set_title_widget(Some(&WindowTitle::new(
        // TRANSLATORS: Title of the page listing a box's applications
        &gettext("Applications"),
        &box_name,
    )));

    let loading_spinner = gtk::Spinner::new();

    // TRANSLATORS: Loading Message
    let loading_lbl = gtk::Label::new(Some(&gettext("Loading...")));
    loading_lbl.add_css_class("title-2");

    // Kept together in a box of their own so the pair can be centred, and so
    // clearing the loading state is one removal rather than two.
    let loading_box = gtk::Box::new(Orientation::Vertical, 10);
    loading_box.set_vexpand(true);
    loading_box.set_valign(Align::Center);
    loading_box.append(&loading_lbl);
    loading_box.append(&loading_spinner);

    let scrolled_win = gtk::ScrolledWindow::new();
    scrolled_win.set_vexpand(true);
    scrolled_win.set_hexpand(true);

    let scroll_area = gtk::Box::new(gtk::Orientation::Vertical, 15);
    scroll_area.set_vexpand(true);
    scroll_area.set_hexpand(true);
    scroll_area.set_margin_start(10);
    scroll_area.set_margin_end(10);
    scroll_area.set_margin_top(10);
    scroll_area.set_margin_bottom(10);

    // Ways of getting applications into the box sit above the list of them.
    let manage_group = adw::PreferencesGroup::new();
    if let Some(install_row) = build_install_package_row(&ui.window, &dbox) {
        manage_group.add(&install_row);
    }
    scroll_area.append(&manage_group);

    scroll_area.append(&loading_box);

    scrolled_win.set_child(Some(&scroll_area));

    let toolbar = ToolbarView::new();
    toolbar.add_top_bar(&header);
    toolbar.set_content(Some(&scrolled_win));

    // TRANSLATORS: Title of the page listing a box's applications
    let page = NavigationPage::new(&toolbar, &gettext("Applications"));
    loading_spinner.start();
    ui.content_nav.push(&page);

    let (sender, receiver) = async_channel::bounded(1);
    let box_name_clone = box_name.clone();

    gio::spawn_blocking(move || {
        let apps = get_apps_in_box(&box_name_clone);
        let binaries = get_binaries_exported_from_box(&box_name_clone);
        sender
            .send_blocking(AppsFetchMessage::AppsFetched(apps, binaries))
            .expect("The channel needs to be open.");
    });

    glib::spawn_future_local(clone!(
        #[weak]
        scroll_area,
        async move {
            while let Ok(msg) = receiver.recv().await {
                match msg {
                    AppsFetchMessage::AppsFetched(apps, binaries) => {
                        loading_spinner.stop();
                        scroll_area.remove(&loading_box);

                        // With both lists empty there is nothing to put in
                        // sections, and two empty headings would just split the
                        // page between them. One centred message says the same
                        // thing, the way the rest of the app does it.
                        if apps.is_empty() && binaries.is_empty() {
                            //TRANSLATORS: Error Message
                            scroll_area.append(&build_empty_state_page(&gettext(
                                "No Applications Installed",
                            )));
                        } else {
                            let apps_group = adw::PreferencesGroup::new();
                            //TRANSLATORS: Window Title
                            apps_group.set_title(&gettext("Available Applications"));

                            // Export confirmations used to overwrite the heading
                            // itself. A label beside it says the same thing
                            // without the section losing its name.
                            let available_lbl = gtk::Label::new(None);
                            apps_group.set_header_suffix(Some(&available_lbl));

                            if apps.is_empty() {
                                //TRANSLATORS: Error Message
                                apps_group
                                    .set_description(Some(&gettext("No Applications Installed")));
                            }

                            for app in apps {
                                let row = adw::ActionRow::new();
                                row.set_title(&markup_escape_text(&app.name.to_string()));

                                let img = gtk::Image::from_icon_name(&get_available_app_icon_name(
                                    &app.icon,
                                ));

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
                                    // The heading doubles as the place the
                                    // export confirmation is written, so it has
                                    // to be the one still in the window.
                                    let success_lbl = available_lbl.clone();
                                    let app_clone = app.clone();
                                    remove_from_menu_btn.connect_clicked(move |_btn| {
                                        remove_app_from_menu(
                                            &app_clone,
                                            &box_name_clone,
                                            &success_lbl.clone(),
                                        );
                                    });
                                    row.add_suffix(&remove_from_menu_btn);
                                } else {
                                    //TRANSLATORS: Button Label
                                    let add_menu_btn =
                                        gtk::Button::with_label(&gettext("Add To Menu"));
                                    add_menu_btn.add_css_class("pill");
                                    add_menu_btn.set_width_request(200);

                                    let box_name_clone = box_name.clone();
                                    let success_lbl = available_lbl.clone();
                                    let app_clone = app.clone();
                                    add_menu_btn.connect_clicked(move |_btn| {
                                        add_app_to_menu(
                                            &app_clone,
                                            &box_name_clone,
                                            &success_lbl.clone(),
                                        );
                                    });
                                    row.add_suffix(&add_menu_btn);
                                }

                                apps_group.add(&row);
                            }

                            scroll_area.append(&apps_group);

                            let bins_group = adw::PreferencesGroup::new();
                            bins_group.set_title(&gettext("Exported Binaries"));

                            if binaries.is_empty() {
                                //TRANSLATORS: Error Message
                                bins_group.set_description(Some(&gettext("No Binaries Exported")));
                            }

                            for binary in binaries {
                                let row = adw::ActionRow::new();
                                row.set_title(&markup_escape_text(&binary.to_string()));

                                // TRANSLATORS: Button Text
                                let remove_btn = gtk::Button::with_label(&gettext("Remove"));
                                remove_btn.add_css_class("pill");
                                //remove_btn.set_width_request(200);

                                let box_name_clone = box_name.clone();
                                let row_clone = row.clone();
                                remove_btn.connect_clicked(move |btn| {
                                    remove_exported_binary(&box_name_clone, &binary, &row_clone);
                                    btn.set_sensitive(false);
                                });
                                row.add_suffix(&remove_btn);
                                bins_group.add(&row);
                            }

                            scroll_area.append(&bins_group);
                        }
                    }
                }
            }
        }
    ));
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

fn remove_exported_binary(box_name: &str, binary: &str, row: &adw::ActionRow) {
    remove_exported_binary_from_box(&box_name, &binary);
    row.set_title("Removed!");
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
        Some(&gettext(&format!(
            "Are you sure you want to delete {}?",
            box_name
        ))),
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
            if let Some(child) = win_clone.content() {
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
        let bn = box_name.clone();

        let (sender, receiver) = async_channel::bounded(1);

        gio::spawn_blocking(move || {
            clone_box(&bn, &name);
            sender
                .send_blocking(BoxCreatedMessage::Success)
                .expect("The channel needs to be open.");
        });

        let b_clone = btn.clone();
        let ls_clone = loading_spinner_clone.clone();
        let w_clone = win_clone.clone();

        glib::spawn_future_local(clone!(
            #[weak]
            ls_clone,
            async move {
                while let Ok(msg) = receiver.recv().await {
                    match msg {
                        BoxCreatedMessage::Success => {
                            ls_clone.stop();

                            let win = b_clone.root().and_downcast::<gtk::Window>().unwrap();
                            win.destroy();

                            let num_boxes = get_number_of_boxes();
                            delayed_rerender(&w_clone, Some(num_boxes - 1));

                            open_terminal_in_box(name_clone.clone());
                        }
                    }
                }
            }
        ));
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
    // Refreshing re-runs the same dependency check the window did when it opened,
    // not just the box list. Asking a distrobox that is not there for its boxes
    // simply yields an empty list, which would swap the accurate "not found"
    // message for a "No Boxes" screen telling the user to create one. Going back
    // through render_main_content also lets the window recover on its own once a
    // missing command is installed.
    render_main_content(window, active_page);
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
        let message_body = gettext(&format!(
            "You don't appear to have any boxes which can install {} files",
            binary_file_type
        ));
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
        .title(gettext(&format!("Install {} File", binary_file_type)))
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
    let file_path_label = gtk::Label::new(Some(&gettext(&format!("Installing: {}", file_path))));

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
            // Look up the box's image so the right package manager is
            // picked. Done per-click rather than cached because the image
            // can change after a `distrobox upgrade`.
            let image = get_all_distroboxes()
                .into_iter()
                .find(|b| b.name == box_name)
                .map(|b| b.image_url)
                .unwrap_or_default();

            match pt_clone {
                BinaryPackageType::Deb => install_deb_in_box(box_name, image, bin_clone.clone()),
                BinaryPackageType::Rpm => install_rpm_in_box(box_name, image, bin_clone.clone()),
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

fn on_install_deb_clicked(window: &ApplicationWindow, box_name: String, box_image: String) {
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
                        install_deb_in_box(box_name, box_image, dp);
                    }
                }
            }
        }),
    );
}

fn on_install_rpm_clicked(window: &ApplicationWindow, box_name: String, box_image: String) {
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
                        install_rpm_in_box(box_name, box_image, rp);
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
    let message_body = gettext(&format!(
        "This file does not appear to be a {} file",
        pkg_type
    ));
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

/// The app's preferences: today that is the terminal used by the actions that
/// open one. Picking a terminal saves it straight away, the way GNOME
/// preferences do, and a toast confirms it.
fn show_preferences(window: &ApplicationWindow) {
    let terms = get_supported_terminals();
    let default_term = Settings::new(APP_ID).string("default-terminal");
    let selected = terms
        .iter()
        .position(|t| t.name == default_term)
        .and_then(|i| u32::try_from(i).ok())
        .unwrap_or(0);

    let names: Vec<&str> = terms.iter().map(|t| t.name.as_str()).collect();
    let terminal_row = adw::ComboRow::new();
    // TRANSLATORS: Label for Dropdown of terminals available
    terminal_row.set_title(&gettext("Terminal"));
    // TRANSLATORS: Subtitle explaining what the terminal preference is for
    terminal_row.set_subtitle(&gettext("Used by the actions that open a terminal"));
    terminal_row.set_model(Some(&gtk::StringList::new(&names)));
    terminal_row.set_enable_search(true);
    terminal_row.set_selected(selected);

    terminal_row.connect_selected_notify(move |row| {
        let Some(item) = row.selected_item().and_downcast::<gtk::StringObject>() else {
            return;
        };
        let saved = Settings::new(APP_ID)
            .set_string("default-terminal", &item.string())
            .is_ok();
        let message = if saved {
            // TRANSLATORS: Success Message
            gettext("Terminal Preference Saved!")
        } else {
            // TRANSLATORS: Error Message
            gettext("Sorry, Preference Could Not Be Saved")
        };
        if let Some(ui) = MAIN_UI.with(|cell| cell.borrow().clone()) {
            ui.toast_overlay.add_toast(adw::Toast::new(&message));
        }
    });

    let group = adw::PreferencesGroup::new();
    group.add(&terminal_row);

    let page = adw::PreferencesPage::new();
    page.add(&group);

    let prefs = adw::PreferencesWindow::new();
    prefs.set_transient_for(Some(window));
    prefs.set_modal(true);
    prefs.add(&page);
    prefs.present();
}
