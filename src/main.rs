use gettextrs::gettext;
use std::cell::Cell;
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
    assemble_box, box_command_path, build_assemble_ini, clone_box, create_box,
    create_box_streaming, delete_box, export_app_from_box, export_binary_from_box,
    get_all_distroboxes, get_apps_in_box, get_available_images_with_distro_name,
    get_binaries_exported_from_box, get_commands_in_box, get_number_of_boxes,
    host_command_conflicts, install_deb_in_box, install_rpm_in_box, is_app_exported,
    list_dispatchers_for_box, open_terminal_in_box, parse_assemble_ini, reboot_box,
    remove_app_from_host, remove_dispatcher, remove_exported_binary_from_box, run_command_in_box,
    start_box, stop_box, uninstall_app_in_box, upgrade_all_boxes_streaming, upgrade_box_streaming,
    valid_command_name, write_dispatcher, DBox, DBoxApp, HostCommandState,
};

mod utils;
use utils::{
    detect_pkg_manager, get_available_app_icon_name, get_available_icon_name, get_box_home,
    get_cpu_and_mem_usage, get_deb_distros, get_distro_color_css, get_distro_img,
    get_download_dir_path, get_exported_app_label, get_host_home_dir, get_installed_terminals,
    get_my_deb_boxes, get_my_rpm_boxes, get_profiles, get_rpm_distros,
    get_supported_terminals_list, get_terminal_and_separator_arg, has_distrobox_installed,
    has_file_extension, has_host_access, has_podman_or_docker_installed, image_publisher,
    open_path_in_file_manager, profile_label_for_home, remove_profile, set_exported_app_label,
    set_profile, set_up_localisation, valid_profile_name, PkgManager, ADD_ICON_NAMES,
    COPY_ICON_NAMES, INFO_ICON_NAMES, INSTALL_PACKAGE_ICON_NAMES, MENU_ICON_NAMES,
    MENU_LABEL_ICON_NAMES, REMOVE_ICON_NAMES, STOP_ICON_NAMES, TERMINAL_ICON_NAMES,
    TRASH_ICON_NAMES, UPGRADE_ICON_NAMES, WARNING_ICON_NAMES,
};
const APP_ID: &str = "io.github.dvlv.boxbuddyrs";

enum AppsFetchMessage {
    AppsFetched(Vec<DBoxApp>, Vec<String>, Vec<String>),
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

    // Actions first: the sidebar header binds a button to one of them, and
    // loading the boxes enables it.
    set_window_actions(&window);

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
    // TRANSLATORS: Menu Item under the "+" button - opens the form that writes a distrobox.ini
    new_menu.append(
        Some(&gettext("Write Assemble File…")),
        Some("win.create_assemble_ini"),
    );

    let add_btn = gtk::MenuButton::new();
    add_btn.set_icon_name(&get_available_icon_name(ADD_ICON_NAMES));
    add_btn.set_menu_model(Some(&new_menu));
    // TRANSLATORS: Button tooltip
    add_btn.set_tooltip_text(Some(&gettext("Create A Distrobox")));

    let upgrade_btn = gtk::Button::from_icon_name(&get_available_icon_name(UPGRADE_ICON_NAMES));
    // TRANSLATORS: Button tooltip
    upgrade_btn.set_tooltip_text(Some(&gettext("Upgrade All Boxes")));
    // Bound to the window action rather than a click handler, so its
    // sensitivity simply follows the action: populate_boxes enables it only
    // once there is at least one box to upgrade.
    upgrade_btn.set_action_name(Some("win.upgrade-all"));

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

    let titlebar = adw::HeaderBar::new();

    titlebar.pack_start(&add_btn);
    titlebar.pack_end(&menu_btn);
    titlebar.pack_end(&upgrade_btn);

    let _ = window;
    titlebar
}

/// "Upgrade All Boxes" is a window action so the header button's sensitivity
/// follows it. Upgrading zero boxes is a no-op, so it is only enabled once
/// populate_boxes has found something to upgrade - which also means it stays
/// off while distrobox or the container engine are missing.
fn set_upgrade_all_enabled(window: &ApplicationWindow, enabled: bool) {
    if let Some(action) = window
        .lookup_action("upgrade-all")
        .and_downcast::<gio::SimpleAction>()
    {
        action.set_enabled(enabled);
    }
}

/// Picks a distrobox.ini, shows what it would create, and assembles it on
/// Apply. A file the parser gets nothing out of skips the preview rather than
/// blocking the user.
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
            let Ok(file) = result else { return };
            let Some(path) = file.path() else { return };
            let path_str = path.to_string_lossy().into_owned();

            let contents = std::fs::read_to_string(&path).unwrap_or_default();
            let sections = parse_assemble_ini(&contents);

            if sections.is_empty() {
                assemble_new_distrobox(&window, path_str);
            } else {
                show_assemble_preview_dialog(&window, path_str, sections);
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

    let action_create_assemble_ini = gio::ActionEntry::builder("create_assemble_ini")
        .activate(|window: &ApplicationWindow, _, _| {
            show_create_assemble_ini_dialog(window);
        })
        .build();

    let action_upgrade_all = gio::ActionEntry::builder("upgrade-all")
        .activate(|window: &ApplicationWindow, _, _| {
            run_streamed_action(
                window,
                // TRANSLATORS: Title of the dialog streaming an upgrade of every box
                &gettext("Upgrading all boxes…"),
                // TRANSLATORS: Status line above the streamed upgrade output
                &gettext("Streaming output of `distrobox upgrade --all`…"),
                None,
                upgrade_all_boxes_streaming,
            );
        })
        .build();

    let action_show_profiles = gio::ActionEntry::builder("show_profiles")
        .activate(|window: &ApplicationWindow, _, _| {
            show_profiles_popup(window);
        })
        .build();

    window.add_action_entries([
        action_refresh,
        action_about,
        action_close,
        action_preferences,
        action_show_profiles,
        action_create_assemble_ini,
        action_upgrade_all,
        action_new_box,
        action_assemble,
    ]);

    set_upgrade_all_enabled(window, false);
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
    standard_section.append(Some(&gettext("Profiles")), Some("win.show_profiles"));
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

    set_upgrade_all_enabled(&ui.window, !boxes.is_empty());

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

/// Loads the distro-colour CSS classes into the display, once. Doing this
/// per box would pile up a provider for every tab of every rerender, and
/// since every provider defined the same class, each new box repainted every
/// existing bar with its own colour - the last box always won.
fn ensure_distro_color_styles() {
    static LOADED: std::sync::Once = std::sync::Once::new();
    LOADED.call_once(|| {
        if let Some(display) = gtk::gdk::Display::default() {
            let provider = gtk::CssProvider::new();
            provider.load_from_string(&get_distro_color_css());
            gtk::style_context_add_provider_for_display(
                &display,
                &provider,
                gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
            );
        }
    });
}

fn make_box_tab(dbox: &DBox, window: &ApplicationWindow, tab_num: u32) -> gtk::Box {
    let box_name = dbox.name.clone();

    // Read the box's home directory once, with the rest of the box's data,
    // rather than every time the row redraws.
    let box_home = get_box_home(&box_name);
    let profile_label = profile_label_for_home(&box_home);

    let tab_box = gtk::Box::new(Orientation::Vertical, 15);
    tab_box.set_hexpand(true);

    tab_box.set_margin_top(10);
    tab_box.set_margin_bottom(10);
    tab_box.set_margin_start(10);
    tab_box.set_margin_end(10);

    //title
    // A CSS-coloured bar in the distro's brand colour, in place of the
    // Unicode-dot label: a real widget scales and themes properly where the
    // text glyph rendered inconsistently.
    ensure_distro_color_styles();
    let color_bar = gtk::Box::new(Orientation::Vertical, 0);
    color_bar.set_size_request(4, 32);
    color_bar.set_valign(Align::Center);
    color_bar.add_css_class("distro-color-bar");
    color_bar.add_css_class(&format!("distro-color-bar-{}", dbox.distro));

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

    // Start is the counterpart of Stop and sits right next to it.
    let start_btn = gtk::Button::from_icon_name("media-playback-start-symbolic");
    // TRANSLATORS: Button tooltip
    start_btn.set_tooltip_text(Some(&gettext("Start Box")));

    let start_bn_clone = dbox.name.clone();
    let start_win_clone = window.clone();
    start_btn.connect_clicked(move |_btn| {
        start_box(&start_bn_clone);
        delayed_rerender(&start_win_clone, Some(tab_num));
    });

    let title_box = gtk::Box::new(Orientation::Horizontal, 10);
    title_box.set_margin_start(10);
    title_box.append(&color_bar);
    title_box.append(&page_title);
    title_box.append(&page_status);

    // Both buttons stay in place and the one that does not apply is disabled,
    // so the header keeps its shape and the state is readable at a glance.
    start_btn.set_sensitive(!dbox.is_running);
    stop_btn.set_sensitive(dbox.is_running);
    title_box.append(&start_btn);
    title_box.append(&stop_btn);

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
    let up_win = window.clone();
    upgrade_row.connect_activated(move |_row| on_upgrade_clicked(&up_win, &up_bn_clone, tab_num));

    // Reboot Box Icon
    let reboot_icon = gtk::Image::from_icon_name("system-reboot-symbolic");

    let reboot_row = ActionRow::new();
    // TRANSLATORS: Row Label
    reboot_row.set_title(&gettext("Reboot Box"));
    reboot_row.add_suffix(&reboot_icon);
    reboot_row.set_activatable(true);

    let reboot_bn_clone = box_name.clone();
    let reboot_win_clone = window.clone();
    reboot_row.connect_activated(move |_row| {
        reboot_box(&reboot_bn_clone);
        delayed_rerender(&reboot_win_clone, Some(tab_num));
    });

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

    // Deleting a running box would pull it out from under whatever is using it,
    // so make the user stop it first. The Stop button on the header is right
    // there while the box is up; once it is down the row enables itself.
    delete_row.set_sensitive(!dbox.is_running);

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

    // distrobox refuses to clone a running container, and the app used to get
    // round that by stopping the box behind the user's back. Like Delete, the
    // row is simply unavailable until the box is stopped - the Stop button is
    // right there in the header.
    clone_row.set_sensitive(!dbox.is_running);

    // These rows run inside the container, and distrobox quietly starts a
    // stopped box the moment one of them is used. Now that starting is an
    // explicit action, they stay disabled until the box is actually up. The
    // Applications row gates the whole applications page, package install
    // included.
    open_terminal_row.set_sensitive(dbox.is_running);
    upgrade_row.set_sensitive(dbox.is_running);
    show_applications_row.set_sensitive(dbox.is_running);

    // Profile - a fact about the box, not an action: no suffix icon, no click.
    let profile_row = ActionRow::new();
    //TRANSLATORS: Row label - shows which home directory the box is using
    profile_row.set_title(&gettext("Profile"));
    profile_row.set_subtitle(&profile_label);
    profile_row.set_activatable(false);

    // put all into list
    boxed_list.append(&profile_row);
    boxed_list.append(&open_terminal_row);
    boxed_list.append(&upgrade_row);
    // Rebooting only makes sense for a box that is up; a stopped one is started
    // with the Start button instead. The row stays put and is greyed out rather
    // than coming and going with the state.
    reboot_row.set_sensitive(dbox.is_running);
    boxed_list.append(&reboot_row);

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

/// Show a small dialog that helps the user author a `distrobox.ini` file from
/// scratch. The file is written with a `.ini` extension into a user-chosen
/// directory; nothing about it is executed, so a user who only wants to
/// inspect a draft can do so without any risk of an unintended container
/// being built.
///
/// We deliberately keep this dialog simple. The shape of `distrobox.ini` is
/// a flat INI with one section per box and one key per option, and the few
/// keys the user is most likely to want (`image`, `init`, `nvidia`, `home`,
/// `additional_packages`) cover the common cases. Anything beyond that has
/// to be edited by hand.
fn show_create_assemble_ini_dialog(window: &ApplicationWindow) {
    let popup = gtk::Window::builder()
        // TRANSLATORS: Popup Window Title
        .title(gettext("Create Assemble INI"))
        .transient_for(window)
        .default_width(560)
        .default_height(560)
        .modal(true)
        .build();

    let titlebar = adw::HeaderBar::new();

    let cancel_btn = gtk::Button::with_label(&gettext("Cancel"));
    // TRANSLATORS: Button tooltip
    cancel_btn.set_tooltip_text(Some(&gettext("Cancel")));
    let popup_clone = popup.clone();
    cancel_btn.connect_clicked(move |_btn| popup_clone.destroy());

    let save_btn = gtk::Button::with_label(&gettext("Save"));
    // TRANSLATORS: Button tooltip
    save_btn.set_tooltip_text(Some(&gettext("Save INI file")));
    save_btn.add_css_class("suggested-action");

    titlebar.pack_start(&cancel_btn);
    titlebar.pack_end(&save_btn);
    popup.set_titlebar(Some(&titlebar));

    let main_box = gtk::Box::new(Orientation::Vertical, 10);
    main_box.set_margin_start(10);
    main_box.set_margin_end(10);
    main_box.set_margin_top(10);
    main_box.set_margin_bottom(10);

    let form = gtk::ListBox::new();
    form.set_selection_mode(gtk::SelectionMode::None);
    form.add_css_class("boxed-list");

    // TRANSLATORS: Entry Label - section name in the assemble .ini
    let name_row = adw::EntryRow::new();
    name_row.set_title(&gettext("Section / Box name"));
    name_row.set_text("my-box");

    // TRANSLATORS: Entry Label - container image
    let image_row = adw::EntryRow::new();
    image_row.set_title(&gettext("Image"));
    image_row.set_text("ubuntu:24.04");

    // TRANSLATORS: Entry Label - comma-separated extra packages
    let packages_row = adw::EntryRow::new();
    packages_row.set_title(&gettext("Additional packages (comma-separated, optional)"));

    // TRANSLATORS: Entry Label - custom home directory
    let home_row = adw::EntryRow::new();
    home_row.set_title(&gettext("Custom home directory (optional)"));

    let init_row = adw::SwitchRow::new();
    init_row.set_title(&gettext("Enable init system (systemd)"));

    let nvidia_row = adw::SwitchRow::new();
    nvidia_row.set_title(&gettext("Enable NVIDIA GPU support"));

    form.append(&name_row);
    form.append(&image_row);
    form.append(&packages_row);
    form.append(&home_row);
    form.append(&init_row);
    form.append(&nvidia_row);

    let preview_label = gtk::Label::new(None);
    preview_label.set_xalign(0.0);
    preview_label.set_yalign(0.0);
    preview_label.set_wrap(true);
    preview_label.set_selectable(true);
    preview_label.add_css_class("monospace");
    preview_label.add_css_class("dim-label");
    // TRANSLATORS: Preview heading for the .ini contents the user is composing
    preview_label.set_markup(&gettext(
        "<b>Preview</b> — fill in the form to see what will be saved.",
    ));

    main_box.append(&form);
    main_box.append(&preview_label);

    popup.set_child(Some(&main_box));
    popup.present();

    // One closure re-renders the preview from the current field values and
    // toggles Save. Every field change calls it, and it runs once up front so
    // the preview is populated before the user touches anything.
    let update_preview = {
        let name_row = name_row.clone();
        let image_row = image_row.clone();
        let packages_row = packages_row.clone();
        let home_row = home_row.clone();
        let init_row = init_row.clone();
        let nvidia_row = nvidia_row.clone();
        let preview_label = preview_label.clone();
        let save_btn = save_btn.clone();
        std::rc::Rc::new(move || {
            let section = name_row.text();
            let image = image_row.text();
            if section.trim().is_empty() || image.trim().is_empty() {
                preview_label.set_markup(&gettext(
                    "<b>Preview</b> — section name and image are required.",
                ));
                save_btn.set_sensitive(false);
                return;
            }

            let body = build_assemble_ini(
                &section,
                &image,
                &packages_row.text(),
                &home_row.text(),
                init_row.is_active(),
                nvidia_row.is_active(),
            );
            preview_label.set_text(&body);
            save_btn.set_sensitive(true);
        })
    };

    for row in [&name_row, &image_row, &packages_row, &home_row] {
        let update = update_preview.clone();
        row.connect_changed(move |_row| update());
    }
    for row in [&init_row, &nvidia_row] {
        let update = update_preview.clone();
        row.connect_active_notify(move |_row| update());
    }
    update_preview();

    // Picking a destination and writing the file is wired up to the Save
    // button. We use the same FileDialog the assemble flow already uses for
    // `.ini` selection, with save mode and the `.ini` filter pre-applied.
    let popup_for_save = popup.clone();
    save_btn.connect_clicked(move |_btn| {
        let section = name_row.text().to_string();
        let image = image_row.text().to_string();
        let packages = packages_row.text().to_string();
        let home = home_row.text().to_string();
        let init = init_row.is_active();
        let nvidia = nvidia_row.is_active();

        if section.trim().is_empty() || image.trim().is_empty() {
            return;
        }

        let body = build_assemble_ini(&section, &image, &packages, &home, init, nvidia);

        // Default filename: <section>.ini in the user's Documents folder.
        let default_dir = if let Ok(home) = std::env::var("HOME") {
            std::path::PathBuf::from(home).join("Documents")
        } else {
            std::path::PathBuf::from(".")
        };
        let default_path = default_dir.join(format!("{section}.ini"));

        let ini_filter = gtk::FileFilter::new();
        //TRANSLATORS: File type
        ini_filter.set_name(Some(&gettext("INI-Files")));
        ini_filter.add_suffix("ini");

        let file_dialog = FileDialog::builder()
            .default_filter(&ini_filter)
            .modal(true)
            .build();
        file_dialog.set_initial_file(Some(&gio::File::for_path(default_path)));

        let body_clone = body.clone();
        let popup_clone2 = popup_for_save.clone();
        file_dialog.save(
            Some(&popup_for_save),
            None::<&gio::Cancellable>,
            move |result| {
                if let Ok(file) = result {
                    if let Some(path) = file.path() {
                        match std::fs::write(&path, &body_clone) {
                            Ok(()) => popup_clone2.destroy(),
                            Err(e) => {
                                // A failed save used to vanish without a
                                // trace; tell the user instead of pretending
                                // it worked.
                                let dialog = adw::MessageDialog::new(
                                    Some(&popup_clone2),
                                    //TRANSLATORS: Error dialog heading when the .ini file cannot be written
                                    Some(&gettext("Could not save file")),
                                    Some(&format!("{e}")),
                                );
                                //TRANSLATORS: Dialog button
                                dialog.add_response("ok", &gettext("OK"));
                                dialog.present();
                            }
                        }
                    }
                }
            },
        );
    });
}

/// Read the parsed `distrobox.ini` back to the user as a confirmation dialog:
/// one row per box section, titled with its name and image, listing every
/// other key the section sets. Keys BoxBuddy has no field for are shown too -
/// a confirmation that hid them would give false assurance, since the file
/// could mount a host path or run an init_hook that fetches and executes a
/// script. Apply proceeds to the existing `assemble_new_distrobox` flow;
/// Cancel just closes the dialog.
fn show_assemble_preview_dialog(
    window: &ApplicationWindow,
    ini_file: String,
    sections: Vec<(String, Vec<(String, String)>)>,
) {
    let popup = adw::MessageDialog::new(
        Some(window),
        // TRANSLATORS: Preview dialog title
        Some(&gettext("Assemble .ini preview")),
        // TRANSLATORS: Preview dialog body
        Some(&gettext(
            "The following boxes will be created from this .ini file. Apply to continue, Cancel to abort.",
        )),
    );

    // TRANSLATORS: Preview dialog Cancel button
    popup.add_response("cancel", &gettext("Cancel"));
    // TRANSLATORS: Preview dialog Apply button
    popup.add_response("apply", &gettext("Apply"));
    popup.set_default_response(Some("apply"));
    popup.set_close_response("cancel");

    let scroll = gtk::ScrolledWindow::new();
    scroll.set_vexpand(true);
    scroll.set_min_content_height(220);
    scroll.set_max_content_height(420);

    let list = gtk::ListBox::new();
    list.set_selection_mode(gtk::SelectionMode::None);
    list.add_css_class("boxed-list");

    for (name, keys) in &sections {
        let image = keys
            .iter()
            .find(|(key, _)| key == "image")
            .map_or("?", |(_, value)| value.as_str());
        let row = adw::ActionRow::new();
        row.set_title(&format!("{name} ({image})"));

        let details: Vec<String> = keys
            .iter()
            .filter(|(key, _)| key != "image")
            .map(|(key, value)| format!("{key} = {value}"))
            .collect();
        if !details.is_empty() {
            // Long values (a hook command, a volume list) must be readable in
            // full, not ellipsized to a teaser, so let the subtitle wrap.
            row.set_subtitle(&details.join("\n"));
            row.set_subtitle_lines(0);
        }

        list.append(&row);
    }

    scroll.set_child(Some(&list));
    // The content area of an `adw::MessageDialog` is its `extra_child`.
    popup.set_extra_child(Some(&scroll));

    let window_for_apply = window.clone();
    popup.connect_response(Some("apply"), move |_, _| {
        assemble_new_distrobox(&window_for_apply, ini_file.clone());
    });

    popup.present();
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

/// Runs a distrobox action whose output we want to watch inside the app instead
/// of a spawned terminal. `producer` is handed a channel it writes lines to on a
/// blocking thread; the live output shows in a dialog, and the box list is
/// refreshed once it finishes, returning to `active_page`.
fn run_streamed_action<P>(
    window: &ApplicationWindow,
    heading: &str,
    status: &str,
    active_page: Option<u32>,
    producer: P,
) where
    P: FnOnce(std::sync::mpsc::Sender<String>) + Send + 'static,
{
    let (line_tx, line_rx) = std::sync::mpsc::channel::<String>();
    let (done_tx, done_rx) = std::sync::mpsc::channel::<()>();

    gio::spawn_blocking(move || {
        producer(line_tx);
        let _ = done_tx.send(());
    });

    let win = window.clone();
    show_streamed_output_dialog(window, heading, status, line_rx, done_rx, move || {
        delayed_rerender(&win, active_page);
    });
}

/// Show a dialog with a `gtk::TextView` that fills with stdout/stderr from a
/// running distrobox command. The dialog stays open until the underlying
/// process reports completion, at which point it auto-destroys itself after
/// a short pause so the user can read the final lines.
///
/// The dialog is read-only and intentionally decoupled from the actual create
/// flow: it exists so the user has something to look at while a 30-second-to-
/// several-minute container build is running, instead of staring at a tiny
/// spinner.
///
/// `line_rx` is fed by the producer's stream threads; we drain it on a
/// short GLib timer so the producer thread does not need to know anything
/// about GTK. `done_rx` is a one-shot signal that the producer is done;
/// `on_done` runs once at the very end on the GLib main loop.
fn show_streamed_output_dialog<F>(
    window: &ApplicationWindow,
    heading: &str,
    status: &str,
    line_rx: std::sync::mpsc::Receiver<String>,
    done_rx: std::sync::mpsc::Receiver<()>,
    on_done: F,
) where
    F: Fn() + 'static,
{
    let popup = gtk::Window::builder()
        .title(heading)
        .transient_for(window)
        .default_width(720)
        .default_height(360)
        .modal(true)
        .build();

    let titlebar = adw::HeaderBar::new();
    let title_lbl = gtk::Label::new(Some(heading));
    titlebar.set_title_widget(Some(&title_lbl));

    let main_box = gtk::Box::new(Orientation::Vertical, 8);
    main_box.set_margin_start(10);
    main_box.set_margin_end(10);
    main_box.set_margin_top(10);
    main_box.set_margin_bottom(10);

    let status_lbl = gtk::Label::new(Some(status));
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
    // The chosen image entry, in the "<distro> - <url>" shape the list uses.
    let chosen_image: Rc<RefCell<String>> = Rc::new(RefCell::new(String::new()));

    // Both a name and an image are needed to create a box. The button used to
    // light up on the name alone, so clicking it with no image chosen did
    // nothing at all and said nothing about why.
    let ner_clone = name_entry_row.clone();
    let chosen_image_for_sens = chosen_image.clone();
    name_entry_row.connect_changed(clone!(@weak create_btn => move |_row| {
        let has_name = !ner_clone.text().to_string().is_empty();
        let has_image = !chosen_image_for_sens.borrow().is_empty();
        create_btn.set_sensitive(has_name && has_image);
    }));

    // TRANSLATORS: Entry Label - Name input for new distrobox
    name_entry_row.set_title(&gettext("Name"));

    //Additional Volumes - will not be shown without host access
    let volume_box_list = gtk::ListBox::new();
    volume_box_list.set_selection_mode(gtk::SelectionMode::None);
    volume_box_list.add_css_class("boxed-list");
    volume_box_list.set_visible(false);

    // One row decides where the box's home is: the host's, a profile's, or a
    // folder picked on the spot. A separate path field beside it would only
    // raise the question of which of the two wins.
    let profiles = get_profiles();
    //TRANSLATORS: Profile choice meaning "no separate home, share the host's"
    let mut profile_names = vec![gettext("Host (shared home)")];
    for (name, _path) in &profiles {
        profile_names.push(name.clone());
    }
    // The index where the dialog-triggering entry will sit, between the
    // profiles and the custom-folder one. Tracked now so the handler can
    // recognise it.
    let new_index = profile_names.len() as u32;
    //TRANSLATORS: Profile choice - opens a dialog to define a new profile
    profile_names.push(gettext("New profile…"));
    //TRANSLATORS: Last profile choice - opens a folder chooser for a one-off home
    profile_names.push(gettext("Custom folder…"));
    let custom_index = (profile_names.len() - 1) as u32;
    let profile_strlist = gtk::StringList::new(
        &profile_names
            .iter()
            .map(|s| s.as_str())
            .collect::<Vec<&str>>(),
    );

    // The home path the Create button will use; empty means the host's home.
    let chosen_home: Rc<RefCell<String>> = Rc::new(RefCell::new(String::new()));

    // The most recent non-"New profile…" selection, so a cancelled dialog
    // can put the row back where the user left it.
    let last_valid_selection: Rc<RefCell<u32>> = Rc::new(RefCell::new(0));
    // Set while we are changing the selection or model from inside the
    // handler, so the resulting re-entry does not loop.
    let suppress_handler: Rc<RefCell<bool>> = Rc::new(RefCell::new(false));

    let profile_combo = adw::ComboRow::new();
    //TRANSLATORS: Combo Row Title - which home the new box is given
    profile_combo.set_title(&gettext("Profile"));
    profile_combo.set_model(Some(&profile_strlist));
    profile_combo.set_selected(0);

    let combo_for_handler = profile_combo.clone();
    let chosen_home_combo = chosen_home.clone();
    let profiles_clone = profiles.clone();
    let last_valid_for_handler = last_valid_selection.clone();
    let suppress_for_handler = suppress_handler.clone();
    profile_combo.connect_selected_item_notify(clone!(@weak window => move |_combo| {
        if *suppress_for_handler.borrow() {
            return;
        }
        let selected = combo_for_handler.selected();
        if selected == custom_index {
            *last_valid_for_handler.borrow_mut() = custom_index;
            let combo_for_pick = combo_for_handler.clone();
            let chosen_for_pick = chosen_home_combo.clone();
            let last_valid_for_pick = last_valid_for_handler.clone();
            let file_dialog = FileDialog::builder().modal(false).build();
            file_dialog.select_folder(Some(&window), None::<&gio::Cancellable>, move |result| {
                if let Ok(file) = result {
                    if let Some(path) = file.path().and_then(|p| p.into_os_string().into_string().ok()) {
                        combo_for_pick.set_subtitle(&path);
                        chosen_for_pick.replace(path);
                        *last_valid_for_pick.borrow_mut() = custom_index;
                        return;
                    }
                }
                // Nothing picked: fall back to the host so the row cannot claim
                // a folder that was never chosen.
                combo_for_pick.set_selected(0);
            });
        } else if selected == new_index {
            // Putting the selection back from inside its own notify handler
            // does not stick - GTK is still applying the change being reacted
            // to, and the row would be left sitting on "New profile…". Do it
            // once the main loop is idle, then ask for the name.
            let previous = *last_valid_for_handler.borrow();
            let combo_deferred = combo_for_handler.clone();
            let chosen_deferred = chosen_home_combo.clone();
            let last_valid_deferred = last_valid_for_handler.clone();
            let suppress_deferred = suppress_for_handler.clone();
            let window_deferred = window.clone();
            glib::idle_add_local_once(move || {
                *suppress_deferred.borrow_mut() = true;
                combo_deferred.set_selected(previous);
                *suppress_deferred.borrow_mut() = false;
                show_new_profile_dialog(
                    &window_deferred,
                    &combo_deferred,
                    &chosen_deferred,
                    &last_valid_deferred,
                    &suppress_deferred,
                );
            });
        } else if selected == 0 {
            profile_combo_set_home(&combo_for_handler, &chosen_home_combo, "");
            *last_valid_for_handler.borrow_mut() = 0;
        } else if let Some((_name, path)) = profiles_clone.get((selected - 1) as usize) {
            profile_combo_set_home(&combo_for_handler, &chosen_home_combo, path);
            *last_valid_for_handler.borrow_mut() = selected;
        }
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
    // Distrobox offers well over a hundred images. A dropdown of that many
    // lines cannot be read, so the row opens a chooser that can be searched
    // and filtered instead. The chosen entry is kept in the same
    // "<distro> - <url>" shape the list has always used, so everything
    // downstream can go on splitting it the way it did.
    let available_images = get_available_images_with_distro_name();

    let image_select_row = adw::ActionRow::new();
    // TRANSLATORS - Label for the row where the user selects the container image to create
    image_select_row.set_title(&gettext("Image"));
    // TRANSLATORS - Shown in the Image row before an image has been picked
    image_select_row.set_subtitle(&gettext("None chosen"));
    image_select_row.set_activatable(true);

    let images_for_chooser = available_images.clone();
    let chosen_for_chooser = chosen_image.clone();
    let row_for_chooser = image_select_row.clone();
    let name_for_chooser = name_entry_row.clone();
    let btn_for_chooser = create_btn.clone();
    image_select_row.connect_activated(clone!(@weak window => move |_row| {
        show_image_chooser(
            &window,
            images_for_chooser.clone(),
            row_for_chooser.clone(),
            chosen_for_chooser.clone(),
            name_for_chooser.clone(),
            btn_for_chooser.clone(),
        );
    }));

    // init
    let init_row = adw::SwitchRow::new();
    // TRANSLATORS - Label for Toggle when creating box to add systemd support
    init_row.set_title(&gettext("Use init system"));
    // TRANSLATORS: Explanation of what the 'use init system' toggle does
    // The old wording ("adds systemd support") left out the half that
    // surprises people: distrobox's --init also unshares the process
    // namespace, so the box ends up more isolated, not less. Users reach for
    // this expecting it to open the box up to the host.
    init_row.set_subtitle(&gettext(
        "Runs systemd inside the box and hides the host's processes from it",
    ));
    // TRANSLATORS: Help text for the init system toggle
    init_row.set_tooltip_text(Some(&gettext(
        "Turn this on for services that have to run inside the box. It is not needed to use the host's tools, and it makes the box more isolated, not less.",
    )));
    init_row.set_active(false);

    let loading_spinner = gtk::Spinner::new();

    let chosen_home_for_create = chosen_home.clone();
    let hn_row = hostname_entry_row.clone();
    let ne_row = name_entry_row.clone();
    let chosen_image_for_create = chosen_image.clone();
    let in_row = init_row.clone();
    let loading_spinner_clone = loading_spinner.clone();
    let win_clone = window.clone();
    let volume_box_list_clone = volume_box_list.clone();
    create_btn.connect_clicked(move |btn| {
        let mut name = ne_row.text().to_string();
        let mut home_path = chosen_home_for_create.borrow().clone();
        let mut hostname = hn_row.text().to_string();
        let use_init = in_row.is_active();
        let mut image = chosen_image_for_create.borrow().clone();

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
        show_streamed_output_dialog(
            &w_clone_for_dialog,
            &gettext("Creating container…"),
            &gettext("Streaming output of `distrobox create`…"),
            line_rx,
            done_rx,
            move || {
                let win = b_clone.root().and_downcast::<gtk::Window>().unwrap();
                win.destroy();

                let num_boxes = get_number_of_boxes();
                delayed_rerender(&w_clone, Some(num_boxes - 1));
            },
        );
    });

    boxed_list.append(&name_entry_row);
    boxed_list.append(&image_select_row);
    boxed_list.append(&init_row);

    boxed_list.append(&profile_combo);
    boxed_list.append(&hostname_entry_row);

    main_box.append(&boxed_list);

    // The profile row is the one option whose consequences are not obvious: it
    // is what makes a box a separate profile of an application rather than
    // another way of running the host's copy.
    // TRANSLATORS: Explanation shown under the new-box form, about the Profile row
    let home_hint = gtk::Label::new(Some(&gettext(
        "A profile gives the box its own settings and logins. Your files on the host stay reachable, and anything you export still appears on the host.",
    )));
    home_hint.set_wrap(true);
    home_hint.set_xalign(0.0);
    home_hint.set_margin_top(6);
    home_hint.add_css_class("dim-label");
    main_box.append(&home_hint);

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

fn on_upgrade_clicked(window: &ApplicationWindow, box_name: &str, tab_num: u32) {
    let box_name = box_name.to_string();
    run_streamed_action(
        window,
        // TRANSLATORS: Title of the dialog streaming a box upgrade
        &gettext("Upgrading box…"),
        // TRANSLATORS: Status line above the streamed upgrade output
        &gettext("Streaming output of `distrobox upgrade`…"),
        Some(tab_num),
        move |tx| upgrade_box_streaming(&box_name, tx),
    );
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
    let box_image = dbox.image_url.clone();

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

    // Ways of getting applications into the box, and the label they get in the
    // host menu, sit above the list of them.
    let manage_group = adw::PreferencesGroup::new();
    if let Some(install_row) = build_install_package_row(&ui.window, &dbox) {
        manage_group.add(&install_row);
    }

    let menu_label_row = ActionRow::new();
    // TRANSLATORS: Row Label - opens a dialog to set the menu label for exported apps
    menu_label_row.set_title(&gettext("Menu Label"));
    menu_label_row.add_suffix(&gtk::Image::from_icon_name(&get_available_icon_name(
        MENU_LABEL_ICON_NAMES,
    )));
    menu_label_row.set_activatable(true);
    set_menu_label_subtitle(&menu_label_row, &box_name);
    let ml_bn_clone = box_name.clone();
    let ml_win = ui.window.clone();
    menu_label_row.connect_activated(move |row| {
        show_menu_label_dialog(&ml_win, ml_bn_clone.clone(), row.clone());
    });
    manage_group.add(&menu_label_row);
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
    let win_for_async = ui.window.clone();

    gio::spawn_blocking(move || {
        let apps = get_apps_in_box(&box_name_clone);
        let binaries = get_binaries_exported_from_box(&box_name_clone);
        let commands = get_commands_in_box(&box_name_clone);
        sender
            .send_blocking(AppsFetchMessage::AppsFetched(apps, binaries, commands))
            .expect("The channel needs to be open.");
    });

    glib::spawn_future_local(clone!(
        #[weak]
        scroll_area,
        async move {
            while let Ok(msg) = receiver.recv().await {
                match msg {
                    AppsFetchMessage::AppsFetched(apps, binaries, commands) => {
                        loading_spinner.stop();
                        scroll_area.remove(&loading_box);

                        // With both lists empty there is nothing to put in
                        // sections, and two empty headings would just split the
                        // page between them. One centred message says the same
                        // thing, the way the rest of the app does it.
                        if apps.is_empty() && binaries.is_empty() && commands.is_empty() {
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
                                add_empty_placeholder_row(
                                    &apps_group,
                                    &gettext("No Applications Installed"),
                                );
                            }

                            for app in apps {
                                // The row itself is only the app - icon, name,
                                // expander arrow - so every row comes out the
                                // same and the name gets the full width. What
                                // can be done with the app lives in activatable
                                // sub-rows underneath, instead of a battery of
                                // differently-sized buttons that line up into
                                // a ragged table.
                                let row = adw::ExpanderRow::new();
                                row.set_title(&markup_escape_text(&app.name.to_string()));

                                let img = gtk::Image::from_icon_name(&get_available_app_icon_name(
                                    &app.icon,
                                ));
                                row.add_prefix(&img);

                                let run_row = adw::ActionRow::new();
                                //TRANSLATORS: Button Label
                                run_row.set_title(&gettext("Run"));
                                run_row.set_activatable(true);
                                let box_name_clone = box_name.clone();
                                let app_clone = app.clone();
                                run_row.connect_activated(move |_row| {
                                    run_app_in_box(&app_clone, &box_name_clone);
                                });
                                row.add_row(&run_row);

                                // Uninstall row: removes the application
                                // from inside the box via the distro's
                                // package manager. The handler asks the
                                // box which package owns the executable,
                                // so the raw Exec= value is enough here.
                                // TRANSLATORS: Button Label
                                let uninstall_row = adw::ActionRow::new();
                                uninstall_row.set_title(&gettext("Uninstall"));
                                uninstall_row.set_activatable(true);
                                let un_box_name = box_name.clone();
                                let un_image = box_image.clone();
                                let un_exec = app.exec_name.clone();
                                uninstall_row.connect_activated(move |_row| {
                                    uninstall_app_in_box(
                                        un_box_name.clone(),
                                        un_image.clone(),
                                        un_exec.clone(),
                                    );
                                });
                                row.add_row(&uninstall_row);

                                // One row covers both directions of the menu
                                // entry: it exports the app or removes the
                                // export, and is retitled after each
                                // activation from what the host menu actually
                                // holds, so it is right straight away rather
                                // than on the next visit.
                                let menu_row = adw::ActionRow::new();
                                set_menu_row_title(&menu_row, app.is_on_host);
                                menu_row.set_activatable(true);

                                let box_name_clone = box_name.clone();
                                // The heading doubles as the place the
                                // export confirmation is written, so it has
                                // to be the one still in the window.
                                let success_lbl = available_lbl.clone();
                                let app_clone = app.clone();
                                menu_row.connect_activated(move |menu_row| {
                                    toggle_app_in_menu(
                                        &app_clone,
                                        &box_name_clone,
                                        menu_row,
                                        &success_lbl,
                                    );
                                });
                                row.add_row(&menu_row);

                                apps_group.add(&row);
                            }

                            scroll_area.append(&apps_group);

                            // Commands the user installed in this box. They
                            // have no .desktop file, so the applications list
                            // above cannot show them, which is why a tool
                            // installed in a box used to be invisible here.
                            let cmds_group = adw::PreferencesGroup::new();
                            //TRANSLATORS: Section heading - commands installed inside the box
                            cmds_group.set_title(&gettext("Commands"));
                            if commands.is_empty() {
                                //TRANSLATORS: Shown when a box has no commands of its own
                                add_empty_placeholder_row(
                                    &cmds_group,
                                    &gettext("No Commands Found"),
                                );
                            }

                            let bins_group = adw::PreferencesGroup::new();
                            bins_group.set_title(&gettext("Exported Binaries"));

                            //TRANSLATORS: Button Label
                            let add_cmd_btn = gtk::Button::with_label(&gettext("Add Command…"));
                            bins_group.set_header_suffix(Some(&add_cmd_btn));

                            if binaries.is_empty() {
                                //TRANSLATORS: Error Message
                                add_empty_placeholder_row(
                                    &bins_group,
                                    &gettext("No Binaries Exported"),
                                );
                            }

                            // A chooser carries distrobox's own export
                            // markers so distrobox keeps recognising the
                            // command, which also means `--list-binaries`
                            // reports it. It gets its own row below, so skip
                            // it here rather than listing it twice.
                            let choosers = list_dispatchers_for_box(&box_name);
                            for binary in binaries {
                                if choosers.iter().any(|(name, _, _)| {
                                    std::path::Path::new(&binary).file_name()
                                        == Some(std::ffi::OsStr::new(name))
                                }) {
                                    continue;
                                }
                                let row = adw::ActionRow::new();
                                row.set_title(&markup_escape_text(&binary.to_string()));

                                // TRANSLATORS: Button Text
                                let remove_btn = gtk::Button::with_label(&gettext("Remove"));
                                remove_btn.set_valign(Align::Center);

                                let box_name_clone = box_name.clone();
                                let row_clone = row.clone();
                                remove_btn.connect_clicked(move |btn| {
                                    remove_exported_binary(&box_name_clone, &binary, &row_clone);
                                    btn.set_sensitive(false);
                                });
                                row.add_suffix(&remove_btn);
                                bins_group.add(&row);
                            }

                            // BoxBuddy-managed dispatchers for this box. They
                            // sit in the same section as `distrobox-export`
                            // binaries because to the user they're just more
                            // commands available in the host terminal.
                            for (name, host, boxes) in choosers {
                                add_chooser_row(&bins_group, name, host, boxes);
                            }

                            let bins_group_for_add = bins_group.clone();
                            let box_name_for_add = box_name.clone();
                            let win_for_add = win_for_async.clone();
                            add_cmd_btn.connect_clicked(move |_btn| {
                                on_add_command_clicked(
                                    &win_for_add,
                                    box_name_for_add.clone(),
                                    bins_group_for_add.clone(),
                                    None,
                                );
                            });

                            for path in commands {
                                let Some(cmd_name) = std::path::Path::new(&path)
                                    .file_name()
                                    .and_then(|n| n.to_str())
                                    .map(str::to_string)
                                else {
                                    continue;
                                };
                                let row = adw::ActionRow::new();
                                row.set_title(&markup_escape_text(&cmd_name));
                                row.set_subtitle(&markup_escape_text(&path));

                                //TRANSLATORS: Button Label
                                let add_btn = gtk::Button::with_label(&gettext("Add to Terminal"));
                                add_btn.set_valign(Align::Center);
                                let win_for_cmd = win_for_async.clone();
                                let box_for_cmd = box_name.clone();
                                let bins_for_cmd = bins_group.clone();
                                add_btn.connect_clicked(move |_btn| {
                                    on_add_command_clicked(
                                        &win_for_cmd,
                                        box_for_cmd.clone(),
                                        bins_for_cmd.clone(),
                                        Some(cmd_name.clone()),
                                    );
                                });
                                row.add_suffix(&add_btn);
                                cmds_group.add(&row);
                            }
                            scroll_area.append(&cmds_group);

                            scroll_area.append(&bins_group);
                        }
                    }
                }
            }
        }
    ));
}

/// Writes the current menu label into the row that opens the dialog for it.
fn set_menu_label_subtitle(row: &ActionRow, box_name: &str) {
    row.set_subtitle(&format!(
        "{} \"{}\"",
        // TRANSLATORS: Row subtitle prefix, followed by the current menu label
        gettext("Exported apps show"),
        menu_label_for_export(box_name).unwrap_or_else(|| format!("(on {box_name})"))
    ));
}

/// Asks for a box's menu label; on Apply it is stored, the apps already on the
/// menu are re-exported with it, and `row` is updated to show it.
fn show_menu_label_dialog(window: &ApplicationWindow, box_name: String, row: ActionRow) {
    let dialog = adw::MessageDialog::new(
        Some(window),
        // TRANSLATORS: Title of the dialog that sets a box's exported-app menu label
        Some(&gettext("Menu Label")),
        // TRANSLATORS: Body of the menu-label dialog
        Some(&gettext(
            "Set the name shown in the menu after each exported app, as \"(on …)\". Leave empty to use the box name.",
        )),
    );

    let entry = adw::EntryRow::new();
    // TRANSLATORS: Entry field label in the menu-label dialog
    entry.set_title(&gettext("Menu label"));
    entry.set_activates_default(true);
    if let Some(current) = get_exported_app_label(&box_name) {
        entry.set_text(&current);
    }

    let group = adw::PreferencesGroup::new();
    group.add(&entry);
    dialog.set_extra_child(Some(&group));

    // TRANSLATORS: Button
    dialog.add_response("cancel", &gettext("Cancel"));
    // TRANSLATORS: Button
    dialog.add_response("apply", &gettext("Apply"));
    dialog.set_response_appearance("apply", adw::ResponseAppearance::Suggested);
    dialog.set_default_response(Some("apply"));
    dialog.set_close_response("cancel");

    dialog.connect_response(Some("apply"), move |_dialog, _res| {
        set_exported_app_label(&box_name, &entry.text());
        // Bring the entries already in the menu up to date with the new label.
        reexport_box_apps(&box_name);
        set_menu_label_subtitle(&row, &box_name);
    });

    dialog.present();
}

/// The `--export-label` to hand distrobox for a box, or `None` for its default.
/// A custom alias is wrapped in the same `(on …)` shape distrobox uses, so a
/// box with no alias set behaves exactly as before.
fn menu_label_for_export(box_name: &str) -> Option<String> {
    get_exported_app_label(box_name).map(|alias| format!("(on {alias})"))
}

/// Re-applies the current menu label to every app already exported from a box,
/// by unexporting and re-exporting each one. Used after the alias changes so the
/// entries already in the menu pick up the new label too.
fn reexport_box_apps(box_name: &str) {
    let label = menu_label_for_export(box_name);
    for app in get_apps_in_box(box_name) {
        if app.is_on_host {
            let _ = remove_app_from_host(&app.desktop_file, box_name);
            let _ = export_app_from_box(&app.desktop_file, box_name, label.as_deref());
        }
    }
}

/// Titles the menu row for the direction its next activation takes.
fn set_menu_row_title(row: &ActionRow, exported: bool) {
    row.set_title(&if exported {
        //TRANSLATORS: Button Label
        gettext("Remove From Menu")
    } else {
        //TRANSLATORS: Button Label
        gettext("Add To Menu")
    });
}

/// Exports the app to the host menu, or removes the export if it is already
/// there, then reads the host back so the row says what the menu now has.
/// The confirmation is only written when the menu actually changed.
fn toggle_app_in_menu(app: &DBoxApp, box_name: &str, row: &ActionRow, success_lbl: &gtk::Label) {
    let was_exported = is_app_exported(box_name, &app.desktop_file);
    if was_exported {
        let _ = remove_app_from_host(&app.desktop_file, box_name);
    } else {
        // Export by the desktop-file id, not the display name, so exactly this
        // one app is exported and it matches how the host copy is detected and
        // removed.
        let label = menu_label_for_export(box_name);
        let _ = export_app_from_box(&app.desktop_file, box_name, label.as_deref());
    }

    let exported = is_app_exported(box_name, &app.desktop_file);
    set_menu_row_title(row, exported);
    if exported != was_exported {
        success_lbl.set_text(&if exported {
            //TRANSLATORS: Success Message
            gettext("App Exported!")
        } else {
            //TRANSLATORS: Success Message
            gettext("App Removed!")
        });
    }
}

fn remove_exported_binary(box_name: &str, binary: &str, row: &adw::ActionRow) {
    remove_exported_binary_from_box(&box_name, &binary);
    row.set_title("Removed!");
}

fn run_app_in_box(app: &DBoxApp, box_name: &str) {
    run_command_in_box(&app.exec_name, box_name);
}

/// Appends one row representing a BoxBuddy dispatcher to `bins_group`.
/// Subtitle lists the targets (host first as the literal "host" when
/// present, then the box names). The Remove button deletes the dispatcher
/// for ALL its targets at once - that consequence goes in the tooltip,
/// not the subtitle, so the row stays short.
fn add_chooser_row(
    bins_group: &adw::PreferencesGroup,
    name: String,
    host: Option<String>,
    boxes: Vec<String>,
) {
    let row = adw::ActionRow::new();
    row.set_title(&markup_escape_text(&name));

    let mut targets: Vec<String> = Vec::new();
    if host.is_some() {
        targets.push("host".to_string());
    }
    targets.extend(boxes.iter().cloned());
    //TRANSLATORS: Subtitle for chooser row - {} replaced with comma-separated targets
    let subtitle = gettext(format!("Chooser: {}", targets.join(", ")));
    row.set_subtitle(&subtitle);
    //TRANSLATORS: Tooltip for chooser row explaining removing affects all targets
    row.set_tooltip_text(Some(&gettext(
        "Removing deletes this chooser for all its targets.",
    )));

    //TRANSLATORS: Button Label
    let remove_btn = gtk::Button::with_label(&gettext("Remove"));
    remove_btn.set_valign(Align::Center);

    let row_clone = row.clone();
    remove_btn.connect_clicked(move |btn| {
        remove_dispatcher(&name);
        row_clone.set_title("Removed!");
        btn.set_sensitive(false);
    });
    row.add_suffix(&remove_btn);
    bins_group.add(&row);
}

/// A section with nothing in it still gets its card, with the reason inside:
/// a bare description under the heading reads as a section that failed to
/// load rather than one that is simply empty.
fn add_empty_placeholder_row(group: &adw::PreferencesGroup, message: &str) {
    let row = adw::ActionRow::new();
    row.set_title(message);
    row.add_css_class("dim-label");
    group.add(&row);
}

/// "Add Command to Terminal" flow. Opens the name-entry dialog, asks
/// the box what the path is, looks at the host for clashes, and either
/// plain-exports the binary or asks whether to overwrite the host side
/// with a BoxBuddy dispatcher.
fn on_add_command_clicked(
    window: &ApplicationWindow,
    box_name: String,
    bins_group: adw::PreferencesGroup,
    prefill: Option<String>,
) {
    let name_dialog = adw::MessageDialog::new(
        Some(window),
        //TRANSLATORS: Popup Heading
        Some(&gettext("Add Command to Terminal")),
        //TRANSLATORS: Popup Body
        Some(&gettext(
            "Make a command from this box available in the host terminal.",
        )),
    );
    name_dialog.set_transient_for(Some(window));

    let entry_row = adw::EntryRow::new();
    //TRANSLATORS: Entry Label - command name to export
    entry_row.set_title(&gettext("Command"));
    entry_row.set_activates_default(true);

    // The host-side name is what makes several boxes usable as profiles of the
    // same tool: the command keeps its name inside the box, while the host gets
    // one entry per box. It follows the command until the user edits it.
    if let Some(cmd) = &prefill {
        entry_row.set_text(cmd);
    }

    let host_name_row = adw::EntryRow::new();
    //TRANSLATORS: Entry Label - the name the command gets on the host
    host_name_row.set_title(&gettext("Name on host"));
    host_name_row.set_activates_default(true);
    let host_name_edited = Rc::new(Cell::new(false));
    let edited_clone = host_name_edited.clone();
    host_name_row.connect_changed(move |_row| edited_clone.set(true));
    let host_name_follow = host_name_row.clone();
    let edited_for_cmd = host_name_edited.clone();
    entry_row.connect_changed(move |row| {
        if !edited_for_cmd.get() {
            let mirrored = row.text();
            host_name_follow.set_text(&mirrored);
            // set_text fires "changed" on the host row; that echo is ours, not
            // the user's, so it must not count as an edit.
            edited_for_cmd.set(false);
        }
    });

    let prefs_group = adw::PreferencesGroup::new();
    prefs_group.add(&entry_row);
    prefs_group.add(&host_name_row);
    name_dialog.set_extra_child(Some(&prefs_group));

    //TRANSLATORS: Button Label
    name_dialog.add_response("cancel", &gettext("Cancel"));
    //TRANSLATORS: Button Label
    name_dialog.add_response("add", &gettext("Add"));
    name_dialog.set_response_appearance("add", adw::ResponseAppearance::Suggested);
    name_dialog.set_default_response(Some("add"));
    name_dialog.set_close_response("cancel");

    let win_clone = window.clone();
    let box_name_clone = box_name.clone();
    let bins_group_clone = bins_group.clone();
    let entry_row_clone = entry_row.clone();
    let host_name_clone = host_name_row.clone();
    name_dialog.connect_response(None, move |_d, res| {
        if res != "add" {
            return;
        }
        let name = entry_row_clone.text().to_string();
        // An empty host name means "same as the command".
        let host_name = match host_name_clone.text().to_string() {
            t if t.is_empty() => name.clone(),
            t => t,
        };
        if !valid_command_name(&name) || !valid_command_name(&host_name) {
            // Silent no-op for invalid input; the entries are right there for
            // the user to fix.
            return;
        }
        let bin_path = match box_command_path(&box_name_clone, &name) {
            Some(p) => p,
            None => {
                let nf = adw::MessageDialog::new(
                    Some(&win_clone),
                    //TRANSLATORS: Popup Heading
                    Some(&gettext("Not Found")),
                    //TRANSLATORS: Popup Body - {} replaced with command name and box name
                    Some(&gettext(format!(
                        "{} was not found in {}",
                        name, box_name_clone
                    ))),
                );
                nf.set_transient_for(Some(&win_clone));
                //TRANSLATORS: Button Label
                nf.add_response("ok", &gettext("Ok"));
                nf.set_default_response(Some("ok"));
                nf.set_close_response("ok");
                nf.present();
                return;
            }
        };

        let host_state = host_command_conflicts(&host_name);
        let has_clash = !host_state.host_paths.is_empty()
            || host_state.wrapper_box.is_some()
            || host_state.dispatcher.is_some();

        // `distrobox-export --bin` always keeps the command's own name, so a
        // different host-side name can only be a chooser - even with nothing
        // in the way. A chooser with one target runs it without asking.
        if !has_clash && host_name != name {
            write_dispatcher(&host_name, &name, None, &[box_name_clone.clone()]);
            add_chooser_row(
                &bins_group_clone,
                host_name.clone(),
                None,
                vec![box_name_clone.clone()],
            );
            return;
        }

        if !has_clash {
            export_binary_from_box(&box_name_clone, &bin_path);
            append_binary_row(&bins_group_clone, &box_name_clone, &name, &bin_path);
            return;
        }

        ask_replace_with_dispatcher(
            &win_clone,
            box_name_clone.clone(),
            bins_group_clone.clone(),
            host_name,
            name,
            host_state,
        );
    });

    name_dialog.present();
}

/// Builds the row shown for a freshly-exported binary in the "Exported
/// Binaries" section. Mirrors `remove_exported_binary` style. `name` is
/// the command name the user typed, used as the row title; `bin_path` is
/// the original in-box path (`--bin` value) we passed to `distrobox-export`
/// at creation time and must keep targeting `remove_exported_binary_from_box`
/// so removal hits the same file.
fn append_binary_row(
    bins_group: &adw::PreferencesGroup,
    box_name: &str,
    name: &str,
    bin_path: &str,
) {
    let row = adw::ActionRow::new();
    row.set_title(&markup_escape_text(name));

    //TRANSLATORS: Button Label
    let remove_btn = gtk::Button::with_label(&gettext("Remove"));
    remove_btn.set_valign(Align::Center);

    let box_name_clone = box_name.to_string();
    let row_clone = row.clone();
    let bin_path_clone = bin_path.to_string();
    remove_btn.connect_clicked(move |btn| {
        // delete targets the original `--bin` value passed to distrobox-export
        remove_exported_binary_from_box(&box_name_clone, &bin_path_clone);
        row_clone.set_title("Removed!");
        btn.set_sensitive(false);
    });
    row.add_suffix(&remove_btn);
    bins_group.add(&row);
}

/// Second dialog in the Add Command flow: lists what already exists on
/// the host side (plain host paths, a wrapper box, or an existing
/// dispatcher's boxes/host) and asks the user whether to overwrite with
/// a dispatcher. Confirmation merges the targets and writes the file.
fn ask_replace_with_dispatcher(
    window: &ApplicationWindow,
    box_name: String,
    bins_group: adw::PreferencesGroup,
    name: String,
    command: String,
    host_state: HostCommandState,
) {
    let mut body_lines: Vec<String> = Vec::new();
    for p in &host_state.host_paths {
        body_lines.push(p.clone());
    }
    if let Some(wb) = &host_state.wrapper_box {
        //TRANSLATORS: Conflict entry - {} replaced with a box name
        body_lines.push(gettext(format!("in box {}", wb)));
    }
    if let Some((Some(h), _)) = &host_state.dispatcher {
        //TRANSLATORS: Conflict entry - {} replaced with an existing dispatcher's host
        body_lines.push(gettext(format!("in dispatcher (host: {})", h)));
    } else if let Some((None, boxes)) = &host_state.dispatcher {
        //TRANSLATORS: Conflict entry - {} replaced with an existing dispatcher's box list
        body_lines.push(gettext(format!(
            "in dispatcher (boxes: {})",
            boxes.join(", ")
        )));
    }
    let body = body_lines.join("\n");

    let d = adw::MessageDialog::new(
        Some(window),
        //TRANSLATORS: Popup Heading - {} replaced with the command name
        Some(&gettext(format!("{} Already Exists", name))),
        Some(&body),
    );
    d.set_transient_for(Some(window));
    //TRANSLATORS: Button Label
    d.add_response("cancel", &gettext("Cancel"));
    //TRANSLATORS: Button Label
    d.add_response("dispatcher", &gettext("Replace With Chooser"));
    d.set_response_appearance("dispatcher", adw::ResponseAppearance::Suggested);
    d.set_default_response(Some("dispatcher"));
    d.set_close_response("cancel");

    let box_name_clone = box_name.clone();
    let bins_group_clone = bins_group.clone();
    d.connect_response(None, move |_dlg, res| {
        if res != "dispatcher" {
            return;
        }
        let mut boxes_vec: Vec<String> = Vec::new();
        if let Some((_, existing_boxes)) = &host_state.dispatcher {
            boxes_vec.extend(existing_boxes.iter().cloned());
        }
        if let Some(wb) = &host_state.wrapper_box {
            if !boxes_vec.contains(wb) {
                boxes_vec.push(wb.clone());
            }
        }
        if !boxes_vec.contains(&box_name_clone) {
            boxes_vec.push(box_name_clone.clone());
        }

        let host: Option<String> = if let Some((Some(h), _)) = &host_state.dispatcher {
            Some(h.clone())
        } else {
            host_state.host_paths.first().cloned()
        };

        write_dispatcher(&name, &command, host.as_deref(), &boxes_vec);
        add_chooser_row(&bins_group_clone, name.clone(), host, boxes_vec);
    });

    d.present();
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

    let boxed_list = gtk::ListBox::new();
    boxed_list.set_selection_mode(gtk::SelectionMode::None);
    boxed_list.add_css_class("boxed-list");

    // name input
    let name_entry_row = adw::EntryRow::new();
    name_entry_row.set_hexpand(true);

    // TRANSLATORS: Entry Label - Name input for new distrobox
    name_entry_row.set_title(&gettext("Name"));

    // The combo drives the home path of the cloned box, the same way the
    // create form does. The selected profile's path is captured here and read
    // in the Clone click handler; an empty string means "Host (shared home)"
    // and leaves `--home` off the distrobox arguments.
    let chosen_home: Rc<RefCell<String>> = Rc::new(RefCell::new(String::new()));
    let profiles = get_profiles();
    let mut profile_names = vec![gettext("Host (shared home)")];
    for (name, _path) in &profiles {
        profile_names.push(name.clone());
    }
    let profile_strlist = gtk::StringList::new(
        &profile_names
            .iter()
            .map(|s| s.as_str())
            .collect::<Vec<&str>>(),
    );

    let profile_combo = adw::ComboRow::new();
    // TRANSLATORS: Combo Row Title - chooses which home the clone should use
    profile_combo.set_title(&gettext("Profile"));
    profile_combo.set_model(Some(&profile_strlist));
    profile_combo.set_selected(0);

    let profile_combo_clone = profile_combo.clone();
    let chosen_home_combo_clone = chosen_home.clone();
    let profiles_clone = profiles.clone();
    profile_combo.connect_selected_item_notify(move |_combo| {
        let selected = profile_combo_clone.selected();
        if selected == 0 {
            // "Host (shared home)" - empty path
            chosen_home_combo_clone.replace(String::new());
        } else if let Some((_name, path)) = profiles_clone.get((selected - 1) as usize) {
            chosen_home_combo_clone.replace(path.clone());
        }
    });

    let loading_spinner = gtk::Spinner::new();

    let loading_spinner_clone = loading_spinner.clone();
    let win_clone = window.clone();
    let ne_row = name_entry_row.clone();
    let chosen_home_btn_clone = chosen_home.clone();
    create_btn.connect_clicked(move |btn| {
        loading_spinner_clone.start();
        let mut name = ne_row.text().to_string();

        if name.is_empty() {
            return;
        }

        name = name.replace(' ', "-");
        let name_clone = name.clone();
        let bn = box_name.clone();
        // Read without consuming: a click that comes to nothing (an empty
        // name, a clone that fails) must leave the chosen profile in place for
        // the next attempt.
        let home_path = chosen_home_btn_clone.borrow().clone();

        let (sender, receiver) = async_channel::bounded(1);

        gio::spawn_blocking(move || {
            clone_box(&bn, &name, &home_path);
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
    boxed_list.append(&profile_combo);
    main_box.append(&title_label);
    main_box.append(&boxed_list);

    let notice_label = gtk::Label::new(Some(&gettext(
        "Note: Cloning can take a long time, please be patient",
    )));

    // TRANSLATORS: Explanatory text under the cloning notice - describes
    // what picking a profile does for the cloned box.
    let profile_explain_label = gtk::Label::new(Some(&gettext(
        "A profile gives the copy its own home directory, so it keeps separate \
         application settings and logins from the box it was cloned from. \
         The original box is not changed.",
    )));
    profile_explain_label.add_css_class("dim-label");
    profile_explain_label.set_wrap(true);
    profile_explain_label.set_xalign(0.0);

    main_box.append(&notice_label);
    main_box.append(&profile_explain_label);
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
/// open one. Only terminals that are actually installed are offered, so what is
/// shown is what will run. Picking one saves it straight away, the way GNOME
/// preferences do, and a toast confirms it.
fn show_preferences(window: &ApplicationWindow) {
    let terms = get_installed_terminals();
    if terms.is_empty() {
        show_no_supported_terminal_popup(window);
        return;
    }
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

/// One row of the profiles list: its name, the directory boxes using it get,
/// a button to look at that directory in the file manager, and one to forget
/// the profile. Removing it only forgets the setting - the directory and any
/// box already built on it are left alone.
fn add_profile_row(group: &adw::PreferencesGroup, name: &str, path: &str) {
    let row = adw::ActionRow::new();
    row.set_title(name);
    row.set_subtitle(path);

    // TRANSLATORS: Button Label - opens the profile's folder in the file manager
    let browse_btn = gtk::Button::with_label(&gettext("Browse"));
    browse_btn.set_valign(Align::Center);
    let browse_path = path.to_string();
    browse_btn.connect_clicked(move |_btn| {
        open_path_in_file_manager(&browse_path);
    });
    row.add_suffix(&browse_btn);

    // TRANSLATORS: Button Label
    let remove_btn = gtk::Button::with_label(&gettext("Remove"));
    remove_btn.set_valign(Align::Center);
    let name_clone = name.to_string();
    let row_clone = row.clone();
    let group_clone = group.clone();
    remove_btn.connect_clicked(move |_btn| {
        remove_profile(&name_clone);
        group_clone.remove(&row_clone);
    });
    row.add_suffix(&remove_btn);

    group.add(&row);
}

/// Keeps the row's subtitle and the home path it stands for in step: the
/// subtitle is the only place the chosen directory is visible now.
fn profile_combo_set_home(row: &adw::ComboRow, home: &Rc<RefCell<String>>, path: &str) {
    row.set_subtitle(path);
    home.replace(path.to_string());
}

/// Rebuild the profile combo's model from a fresh list of profiles. Used
/// after a new profile is created so the row appears in the dropdown.
fn rebuild_profile_combo(combo: &adw::ComboRow, profiles: &[(String, String)]) {
    // TRANSLATORS: Profile choice meaning "no separate home, share the host's"
    let mut profile_names = vec![gettext("Host (shared home)")];
    for (name, _path) in profiles {
        profile_names.push(name.clone());
    }
    // TRANSLATORS: Profile choice - opens a dialog to define a new profile
    profile_names.push(gettext("New profile…"));
    // TRANSLATORS: Last profile choice - opens a folder chooser for a one-off home
    profile_names.push(gettext("Custom folder…"));
    let strlist = gtk::StringList::new(
        &profile_names
            .iter()
            .map(|s| s.as_str())
            .collect::<Vec<&str>>(),
    );
    combo.set_model(Some(&strlist));
}

/// Asks the user for a new profile name and, on a valid answer, adds it
/// under `<host home>/boxes/<name with spaces replaced by dashes>` - the
/// same path the standalone Profiles window uses. On Cancel or an invalid
/// name the combo is put back on whatever the user had before the dialog
/// was opened.
fn show_new_profile_dialog(
    window: &ApplicationWindow,
    combo: &adw::ComboRow,
    chosen_home: &Rc<RefCell<String>>,
    last_valid_selection: &Rc<RefCell<u32>>,
    suppress_handler: &Rc<RefCell<bool>>,
) {
    let d = adw::MessageDialog::new(
        Some(window),
        //TRANSLATORS: Dialog heading - asking for a new profile name
        Some(&gettext("New Profile")),
        //TRANSLATORS: Dialog body - explains what a profile is
        Some(&gettext(
            "A profile is a home directory of its own, so boxes using it keep separate \
application settings and logins.",
        )),
    );
    d.set_transient_for(Some(window));

    //TRANSLATORS: Entry title - the new profile's name
    let name_entry = adw::EntryRow::new();
    name_entry.set_title(&gettext("Name"));
    name_entry.set_activates_default(true);
    d.set_extra_child(Some(&name_entry));

    //TRANSLATORS: Button label
    d.add_response("cancel", &gettext("Cancel"));
    //TRANSLATORS: Button label
    d.add_response("create", &gettext("Create"));
    d.set_default_response(Some("create"));
    d.set_close_response("cancel");
    d.set_response_appearance("create", adw::ResponseAppearance::Suggested);

    let combo_clone = combo.clone();
    let chosen_home_clone = chosen_home.clone();
    let last_valid_clone = last_valid_selection.clone();
    let suppress_clone = suppress_handler.clone();
    d.connect_response(None, move |d, res| {
        let name = name_entry.text().to_string();
        if res == "create" && valid_profile_name(&name) {
            let trimmed = name.trim();
            let host_home = get_host_home_dir();
            let safe_name = trimmed.replace(' ', "-");
            let home_path = format!("{host_home}/boxes/{safe_name}");
            set_profile(trimmed, &home_path);

            // Find where the new profile landed in the sorted list, rebuild
            // the model with it included, and select it.
            let new_profiles = get_profiles();
            let pos = new_profiles
                .iter()
                .position(|(n, _)| n == trimmed)
                .unwrap_or(0);
            let new_combo_index = 1 + pos as u32;

            *suppress_clone.borrow_mut() = true;
            rebuild_profile_combo(&combo_clone, &new_profiles);
            combo_clone.set_selected(new_combo_index);
            profile_combo_set_home(&combo_clone, &chosen_home_clone, &home_path);
            *last_valid_clone.borrow_mut() = new_combo_index;
            *suppress_clone.borrow_mut() = false;
        } else {
            // Cancel or invalid name: snap back to wherever the user was
            // before "New profile…" was picked.
            let previous = *last_valid_clone.borrow();
            *suppress_clone.borrow_mut() = true;
            combo_clone.set_selected(previous);
            *suppress_clone.borrow_mut() = false;
        }
        d.destroy();
    });

    d.present();
}

/// Splits one entry of `get_available_images_with_distro_name` into the parts
/// the chooser shows: the image URL, and whether it is already pulled. The
/// list marks a pulled image by appending " ✦ ", which is also why the URL has
/// to be taken from the end rather than the whole string.
fn image_entry_parts(entry: &str) -> (String, bool) {
    let downloaded = entry.contains('✦');
    let url = entry
        .split(" - ")
        .last()
        .unwrap_or(entry)
        .replace(" ✦ ", "")
        .trim()
        .to_string();
    (url, downloaded)
}

/// The chooser for a container image: over a hundred of them, so it can be
/// searched, filtered by package manager and narrowed to what is already on
/// the machine. Everything it shows comes from the image URL - nothing here
/// touches the network.
fn show_image_chooser(
    window: &ApplicationWindow,
    images: Vec<String>,
    image_row: adw::ActionRow,
    chosen: Rc<RefCell<String>>,
    name_row: adw::EntryRow,
    create_btn: gtk::Button,
) {
    let popup = gtk::Window::builder()
        // TRANSLATORS: Popup Window Title
        .title(gettext("Choose an Image"))
        .transient_for(window)
        .default_width(720)
        .default_height(560)
        .modal(true)
        .build();

    // TRANSLATORS: Button Label
    let close_btn = gtk::Button::with_label(&gettext("Close"));
    close_btn.connect_clicked(move |btn| {
        if let Some(win) = btn.root().and_downcast::<gtk::Window>() {
            win.destroy();
        }
    });
    let titlebar = adw::HeaderBar::new();
    titlebar.set_show_end_title_buttons(false);
    titlebar.pack_end(&close_btn);
    popup.set_titlebar(Some(&titlebar));

    let main_box = gtk::Box::new(Orientation::Vertical, 10);
    main_box.set_margin_start(10);
    main_box.set_margin_end(10);
    main_box.set_margin_top(10);
    main_box.set_margin_bottom(10);

    let search = gtk::SearchEntry::new();
    // TRANSLATORS: Placeholder in the image chooser's search box
    search.set_placeholder_text(Some(&gettext("Search images")));

    // One filter active at a time; "All" is the way back to everything.
    let filter_box = gtk::Box::new(Orientation::Horizontal, 0);
    filter_box.add_css_class("linked");
    filter_box.set_halign(Align::Center);
    //TRANSLATORS: Image filter - no package-manager filter at all
    let all_btn = gtk::ToggleButton::with_label(&gettext("All"));
    all_btn.set_active(true);
    let filters: Vec<(gtk::ToggleButton, Option<PkgManager>)> = vec![
        (all_btn.clone(), None),
        (gtk::ToggleButton::with_label("apt"), Some(PkgManager::Apt)),
        (gtk::ToggleButton::with_label("dnf"), Some(PkgManager::Dnf)),
        (gtk::ToggleButton::with_label("apk"), Some(PkgManager::Apk)),
        (
            gtk::ToggleButton::with_label("pacman"),
            Some(PkgManager::Pacman),
        ),
        (
            gtk::ToggleButton::with_label("zypper"),
            Some(PkgManager::Zypper),
        ),
    ];
    for (btn, _) in &filters {
        filter_box.append(btn);
    }

    //TRANSLATORS: Image filter - only images already pulled onto this machine
    let downloaded_check = gtk::CheckButton::with_label(&gettext("Downloaded only"));

    let count_label = gtk::Label::new(None);
    count_label.set_xalign(0.0);
    count_label.add_css_class("dim-label");

    let list = gtk::ListBox::new();
    list.set_selection_mode(gtk::SelectionMode::None);
    list.add_css_class("boxed-list");

    let scroller = gtk::ScrolledWindow::new();
    scroller.set_vexpand(true);
    scroller.set_child(Some(&list));

    // Redrawn on every change of search text or filter: the list is small
    // enough that rebuilding it is simpler than keeping rows in sync.
    let refill = {
        let list = list.clone();
        let count_label = count_label.clone();
        let search = search.clone();
        let downloaded_check = downloaded_check.clone();
        let images = images.clone();
        let filters = filters.clone();
        let image_row = image_row.clone();
        let chosen = chosen.clone();
        let name_row = name_row.clone();
        let create_btn = create_btn.clone();
        let popup = popup.clone();
        Rc::new(move || {
            list.remove_all();
            let needle = search.text().to_string().to_lowercase();
            let wanted = filters
                .iter()
                .find(|(btn, _)| btn.is_active())
                .and_then(|(_, mgr)| *mgr);
            let only_downloaded = downloaded_check.is_active();

            let mut shown = 0;
            for entry in &images {
                let (url, downloaded) = image_entry_parts(entry);
                if !needle.is_empty() && !entry.to_lowercase().contains(&needle) {
                    continue;
                }
                if only_downloaded && !downloaded {
                    continue;
                }
                if let Some(wanted) = wanted {
                    if detect_pkg_manager(&url) != Some(wanted) {
                        continue;
                    }
                }

                let row = adw::ActionRow::new();
                row.set_title(&markup_escape_text(entry));
                row.set_subtitle(&markup_escape_text(&url));
                row.set_activatable(true);

                let publisher = gtk::Label::new(Some(&image_publisher(&url)));
                publisher.add_css_class("dim-label");
                publisher.set_valign(Align::Center);
                row.add_suffix(&publisher);

                let entry_clone = entry.clone();
                let image_row = image_row.clone();
                let chosen = chosen.clone();
                let name_row = name_row.clone();
                let create_btn = create_btn.clone();
                let popup = popup.clone();
                row.connect_activated(move |_row| {
                    image_row.set_subtitle(&markup_escape_text(&entry_clone));
                    chosen.replace(entry_clone.clone());
                    create_btn.set_sensitive(!name_row.text().to_string().is_empty());
                    popup.destroy();
                });

                list.append(&row);
                shown += 1;
            }

            //TRANSLATORS: Image chooser count - first number is what is shown, second the total
            count_label.set_text(&gettext(format!("{} of {} images", shown, images.len())));
        })
    };

    let refill_for_search = refill.clone();
    search.connect_search_changed(move |_entry| refill_for_search());
    let refill_for_check = refill.clone();
    downloaded_check.connect_toggled(move |_btn| refill_for_check());
    for (btn, _) in &filters {
        let others: Vec<gtk::ToggleButton> = filters
            .iter()
            .map(|(b, _)| b.clone())
            .filter(|b| b != btn)
            .collect();
        let refill_for_btn = refill.clone();
        btn.connect_toggled(move |this| {
            if this.is_active() {
                for other in &others {
                    other.set_active(false);
                }
                refill_for_btn();
            } else if others.iter().all(|b| !b.is_active()) {
                // Refusing to leave every filter off keeps the list from
                // going blank with no way back.
                this.set_active(true);
            }
        });
    }

    refill();

    main_box.append(&search);
    main_box.append(&filter_box);
    main_box.append(&downloaded_check);
    main_box.append(&count_label);
    main_box.append(&scroller);

    popup.set_child(Some(&main_box));
    popup.present();
}

fn show_profiles_popup(window: &ApplicationWindow) {
    let profiles_popup = gtk::Window::builder()
        // TRANSLATORS: Popup Window Title
        .title(gettext("Profiles"))
        .transient_for(window)
        .default_width(600)
        .default_height(400)
        .modal(true)
        .build();

    // TRANSLATORS: Button Label
    let close_btn = gtk::Button::with_label(&gettext("Close"));
    close_btn.add_css_class("suggested-action");
    close_btn.connect_clicked(move |btn| {
        let win = btn.root().and_downcast::<gtk::Window>().unwrap();
        win.destroy();
    });

    let profiles_titlebar = adw::HeaderBar::new();
    profiles_titlebar.set_show_end_title_buttons(false);
    profiles_titlebar.pack_end(&close_btn);

    profiles_popup.set_titlebar(Some(&profiles_titlebar));

    let main_box = gtk::Box::new(Orientation::Vertical, 10);
    main_box.set_margin_start(10);
    main_box.set_margin_end(10);
    main_box.set_margin_top(10);
    main_box.set_margin_bottom(10);

    // TRANSLATORS: Dialog heading
    let heading = gtk::Label::new(Some(&gettext("Profiles")));
    heading.add_css_class("title-1");
    heading.set_xalign(0.0);

    // TRANSLATORS: Dialog body text
    let body = gtk::Label::new(Some(&gettext(
        "A profile gives a box its own home directory, so applications keep separate settings and logins. Boxes with no profile use your host home.",
    )));
    body.set_xalign(0.0);
    body.set_wrap(true);
    body.set_wrap_mode(gtk::pango::WrapMode::WordChar);

    let prefs_group = adw::PreferencesGroup::new();

    let profiles = get_profiles();
    for (name, path) in &profiles {
        add_profile_row(&prefs_group, name, path);
    }

    // Add new profile row
    let add_row = adw::ActionRow::new();

    let name_entry = adw::EntryRow::new();
    // TRANSLATORS: Entry title for new profile name
    name_entry.set_title(&gettext("New Profile Name"));
    name_entry.set_hexpand(true);

    let add_btn = gtk::Button::with_label(&gettext("Add"));
    add_btn.add_css_class("suggested-action");
    add_btn.set_valign(Align::Center);

    let name_entry_clone = name_entry.clone();
    let prefs_group_clone = prefs_group.clone();
    let popup_clone = profiles_popup.clone();
    add_btn.connect_clicked(move |_btn| {
        let name = name_entry_clone.text().to_string();
        let trimmed = name.trim();
        if !valid_profile_name(trimmed) {
            return;
        }
        let host_home = get_host_home_dir();
        let safe_name = trimmed.replace(' ', "-");
        let home_path = format!("{host_home}/boxes/{safe_name}");
        set_profile(trimmed, &home_path);

        add_profile_row(&prefs_group_clone, trimmed, &home_path);
        name_entry_clone.set_text("");
        popup_clone.queue_draw();
    });

    add_row.add_prefix(&name_entry);
    add_row.add_suffix(&add_btn);
    prefs_group.add(&add_row);

    main_box.append(&heading);
    main_box.append(&body);
    main_box.append(&prefs_group);

    profiles_popup.set_child(Some(&main_box));
    profiles_popup.present();
}
