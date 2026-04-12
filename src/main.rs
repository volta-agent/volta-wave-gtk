use gtk4::{glib, prelude::*, Application, ApplicationWindow, Box, Button, Label, ListBox, ListBoxRow, Orientation, Scale, DrawingArea, Entry, CssProvider, MenuButton, Image, Overlay, EventControllerKey, GestureClick, Popover, Separator, Window};
use glib::Propagation;
use gtk4::gdk::Display;
use gtk4::cairo;
use gstreamer::prelude::*;
use glib::clone;
use std::sync::{Arc, Mutex};
use walkdir::WalkDir;
use serde::Deserialize;
use anyhow::Result;
use std::collections::VecDeque;
use rand::Rng;

const MUSIC_DIR: &str = "Music";
const APP_ID: &str = "com.volta-agent.wave-gtk";

// Theme colors: (name, bg, fg, accent, dim, sidebar_bg, viz_accent, gradient_start, gradient_end)
const THEMES: &[(&str, &str, &str, &str, &str, &str, &str, &str, &str)] = &[
 ("Tokyo Night", "#1a1b26", "#c0caf5", "#7aa2f7", "#565f89", "#16161e", "#7aa2f7", "#7aa2f7", "#bb9af7"),
 ("Gruvbox", "#282828", "#ebdbb2", "#fe8019", "#928374", "#1d2021", "#fe8019", "#fe8019", "#fabd2f"),
 ("Dracula", "#282a36", "#f8f8f2", "#bd93f9", "#6272a4", "#21222c", "#bd93f9", "#bd93f9", "#ff79c6"),
 ("Nord", "#2e3440", "#d8dee9", "#88c0d0", "#4c566a", "#242933", "#88c0d0", "#88c0d0", "#81a1c1"),
 ("Catppuccin", "#1e1e2e", "#cdd6f4", "#cba6f7", "#6c7086", "#181825", "#cba6f7", "#f5c2e7", "#cba6f7"),
 ("Solarized", "#002b36", "#839496", "#268bd2", "#586e75", "#00232c", "#268bd2", "#2aa198", "#268bd2"),
 ("Cyberpunk", "#0d0d1a", "#f0f0f0", "#ff00ff", "#4a4a6a", "#0a0a12", "#00ffff", "#ff00ff", "#00ffff"),
 ("Forest", "#1a2e1a", "#c8e6c8", "#4caf50", "#2e5a2e", "#0f1f0f", "#8bc34a", "#4caf50", "#8bc34a"),
 ("Communist Red", "#1a0000", "#ffd700", "#ff0000", "#8b4513", "#0d0000", "#ffd700", "#ff0000", "#ffd700"),
];

#[derive(Clone, Debug)]
struct Track {
 path: String,
 title: String,
 artist: String,
 duration_secs: Option<u64>,
}

#[derive(Clone, Deserialize)]
struct LrcLine {
 #[serde(rename = "timeMs")]
 time_ms: f64,
 text: String,
}

#[derive(Clone, Copy, PartialEq)]
enum RepeatMode {
 Off,
 One,
 All,
}

#[derive(Clone, Copy, PartialEq)]
enum ViewMode {
 Full,
 Mini,
}

struct AppState {
 tracks: Vec<Track>,
 current_index: Option<usize>,
 lyrics: Vec<LrcLine>,
 is_playing: bool,
 current_theme: usize,
 viz_mode: usize,
 spectrum_data: Vec<f64>,
 peak_data: Vec<f64>,
 shuffle_mode: bool,
 repeat_mode: RepeatMode,
 queue: VecDeque<usize>,
 volume: f64,
 seeking: bool,
 view_mode: ViewMode,
 window_x: i32,
 window_y: i32,
}

fn main() -> Result<()> {
 gstreamer::init()?;
 
 let app = Application::builder()
 .application_id(APP_ID)
 .build();

 app.connect_activate(build_ui);
 
 let args: Vec<String> = std::env::args().collect();
 let args: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
 app.run_with_args(&args);
 
 Ok(())
}

fn build_ui(app: &Application) {
 let state = Arc::new(Mutex::new(AppState {
 tracks: Vec::new(),
 current_index: None,
 lyrics: Vec::new(),
 is_playing: false,
 current_theme: 0,
 viz_mode: 0,
 spectrum_data: vec![0.0; 64],
 peak_data: vec![0.0; 64],
 shuffle_mode: false,
 repeat_mode: RepeatMode::Off,
 queue: VecDeque::new(),
 volume: 0.75,
 seeking: false,
 view_mode: ViewMode::Full,
 window_x: 0,
 window_y: 0,
 }));

 let tracks = scan_music_dir();
 let track_count = tracks.len();
 state.lock().unwrap().tracks = tracks.clone();

 let window = ApplicationWindow::builder()
 .application(app)
 .title("Volta Wave")
 .default_width(1200)
 .default_height(800)
 .build();

 apply_theme(0);

 // Keyboard controller
 let key_controller = EventControllerKey::new();
 
 // Main container with background
 let main_box = Box::builder()
 .orientation(Orientation::Horizontal)
 .spacing(0)
 .css_classes(vec!["main-container".to_string()])
 .hexpand(true)
 .vexpand(true)
 .build();

 // ========== LEFT SIDEBAR ==========
 let sidebar = Box::builder()
 .orientation(Orientation::Vertical)
 .spacing(4)
 .width_request(280)
 .margin_top(4)
 .margin_bottom(4)
 .margin_start(4)
 .margin_end(4)
 .css_classes(vec!["sidebar".to_string()])
 .build();

 // Header with mini view toggle
 let header_box = Box::builder()
 .orientation(Orientation::Horizontal)
 .spacing(8)
 .margin_bottom(8)
 .build();

 let title_label = Label::builder()
 .label("Volta Wave")
 .css_classes(vec!["title-2".to_string()])
 .hexpand(true)
 .halign(gtk4::Align::Start)
 .build();

 // Mini view toggle button
 let mini_view_btn = Button::builder()
 .icon_name("view-fullscreen-symbolic")
 .tooltip_text("Mini View (D)")
 .css_classes(vec!["circular".to_string(), "mini-view-btn".to_string()])
 .build();

 // Theme dropdown
 let theme_btn = MenuButton::builder()
 .icon_name("preferences-color-symbolic")
 .tooltip_text("Select Theme")
 .css_classes(vec!["circular".to_string()])
 .build();

 let popover = gtk4::Popover::new();
 let theme_box = gtk4::Box::builder()
 .orientation(Orientation::Vertical)
 .spacing(2)
 .margin_top(4)
 .margin_bottom(4)
 .margin_start(4)
 .margin_end(4)
 .build();

 for (i, (name, _, _, _, _, _, _, _, _)) in THEMES.iter().enumerate() {
 let btn = Button::builder()
 .label(*name)
 .css_classes(vec!["flat".to_string(), "theme-btn".to_string()])
 .build();
 
 let state_clone = state.clone();
 let window_clone = window.clone();
 btn.connect_clicked(glib::clone!(@strong state_clone, @strong window_clone => move |_| {
 apply_theme(i);
 state_clone.lock().unwrap().current_theme = i;
 window_clone.queue_draw();
 }));
 
 theme_box.append(&btn);
 }
 
 popover.set_child(Some(&theme_box));
 theme_btn.set_popover(Some(&popover));

 header_box.append(&title_label);
 header_box.append(&mini_view_btn);
 header_box.append(&theme_btn);

 // Tab buttons for Library/Queue
 let tab_box = Box::builder()
 .orientation(Orientation::Horizontal)
 .spacing(0)
 .css_classes(vec!["tab-box".to_string()])
 .margin_bottom(4)
 .build();

 let library_tab = Button::builder()
 .label("Library")
 .css_classes(vec!["tab-btn".to_string(), "active".to_string()])
 .hexpand(true)
 .build();

 let queue_tab = Button::builder()
 .label("Queue")
 .css_classes(vec!["tab-btn".to_string()])
 .hexpand(true)
 .build();

 tab_box.append(&library_tab);
 tab_box.append(&queue_tab);

 // Search entry
 let search_entry = Entry::builder()
 .placeholder_text("Search tracks...")
 .margin_bottom(4)
 .css_classes(vec!["search-entry".to_string()])
 .build();

 // Track count label
 let track_count_label = Label::builder()
 .label(&format!("{} tracks", track_count))
 .css_classes(vec!["dim-label".to_string(), "track-count".to_string()])
 .halign(gtk4::Align::Start)
 .margin_bottom(4)
 .build();

 // Queue count label
 let queue_count_label = Label::builder()
 .label("Queue: 0")
 .css_classes(vec!["dim-label".to_string(), "queue-count".to_string()])
 .halign(gtk4::Align::Start)
 .margin_bottom(4)
 .visible(false)
 .build();

 // Playlist management buttons
 let playlist_box = Box::builder()
 .orientation(Orientation::Horizontal)
 .spacing(4)
 .margin_bottom(4)
 .build();

 let save_playlist_btn = Button::builder()
 .icon_name("document-save-symbolic")
 .tooltip_text("Save Playlist")
 .css_classes(vec!["pill".to_string()])
 .build();

 let load_playlist_btn = Button::builder()
 .icon_name("folder-open-symbolic")
 .tooltip_text("Load Playlist")
 .css_classes(vec!["pill".to_string()])
 .build();

 playlist_box.append(&save_playlist_btn);
 playlist_box.append(&load_playlist_btn);

 // ========== LIBRARY VIEW ==========
 // Track list header - simple header matching vertical layout
 let track_header = Label::builder()
 .label("Library")
 .css_classes(vec!["header-label".to_string()])
 .halign(gtk4::Align::Start)
 .margin_bottom(4)
 .margin_start(8)
 .build();

 // Track list
 let track_scrolled = gtk4::ScrolledWindow::builder()
 .vexpand(true)
 .hexpand(true)
 .min_content_width(300)
 .css_classes(vec!["track-scroll".to_string()])
 .build();

 let track_list = ListBox::builder()
 .css_classes(vec!["navigation-sidebar".to_string(), "track-list".to_string()])
 .selection_mode(gtk4::SelectionMode::Single)
 .build();

 track_scrolled.set_child(Some(&track_list));

 // ========== QUEUE VIEW ==========
 let queue_scrolled = gtk4::ScrolledWindow::builder()
 .vexpand(true)
 .hexpand(true)
 .min_content_width(300)
 .css_classes(vec!["track-scroll".to_string()])
 .visible(false)
 .build();

 let queue_list = ListBox::builder()
 .css_classes(vec!["navigation-sidebar".to_string(), "queue-list".to_string()])
 .selection_mode(gtk4::SelectionMode::Single)
 .build();

 queue_scrolled.set_child(Some(&queue_list));

 // Queue controls
 let queue_controls = Box::builder()
 .orientation(Orientation::Horizontal)
 .spacing(4)
 .margin_top(4)
 .visible(false)
 .build();

 let clear_queue_btn = Button::builder()
 .label("Clear Queue")
 .css_classes(vec!["pill".to_string()])
 .hexpand(true)
 .build();

 queue_controls.append(&clear_queue_btn);

 // ========== NOW POPULATE TRACKS WITH QUEUE ACCESS ==========
 // We need queue_list and queue_count_label to exist first, so we defer track population
 let track_rows: Arc<Mutex<Vec<ListBoxRow>>> = Arc::new(Mutex::new(Vec::new()));

 for (idx, track) in tracks.iter().enumerate() {
 let (row, add_btn) = create_track_row(track);

 // Connect add to queue button - now with queue UI update
 let state_clone = state.clone();
 let queue_list_clone = queue_list.clone();
 let queue_count_label_clone = queue_count_label.clone();
 let track_clone = track.clone();
 add_btn.connect_clicked(glib::clone!(@strong state_clone, @strong queue_list_clone, @strong queue_count_label_clone, @strong track_clone => move |_| {
 let mut s = state_clone.lock().unwrap();
 s.queue.push_back(idx);
 
 // Add to queue UI
 let queue_row = create_queue_row(&track_clone, idx, queue_list_clone.clone(), state_clone.clone());
 queue_list_clone.append(&queue_row);
 
 let queue_len = s.queue.len();
 drop(s);
 queue_count_label_clone.set_label(&format!("Queue: {}", queue_len));
 }));

 track_list.append(&row);
 track_rows.lock().unwrap().push(row);
 }

 sidebar.append(&header_box);
 sidebar.append(&tab_box);
 sidebar.append(&search_entry);
 sidebar.append(&track_count_label);
 sidebar.append(&queue_count_label);
 sidebar.append(&track_header);
 sidebar.append(&track_scrolled);
 sidebar.append(&queue_scrolled);
 sidebar.append(&queue_controls);
 sidebar.append(&playlist_box);

 // Tab switching
 let library_tab_clone = library_tab.clone();
 let queue_tab_clone = queue_tab.clone();
 let track_scrolled_clone = track_scrolled.clone();
 let queue_scrolled_clone = queue_scrolled.clone();
 let track_count_label_clone = track_count_label.clone();
 let queue_count_label_clone = queue_count_label.clone();
 let queue_controls_clone = queue_controls.clone();
 let track_header_clone = track_header.clone();

 library_tab.connect_clicked(glib::clone!(
 @strong library_tab_clone,
 @strong queue_tab_clone,
 @strong track_scrolled_clone,
 @strong queue_scrolled_clone,
 @strong track_count_label_clone,
 @strong queue_count_label_clone,
 @strong queue_controls_clone,
 @strong track_header_clone => move |_| {
 library_tab_clone.add_css_class("active");
 queue_tab_clone.remove_css_class("active");
 track_scrolled_clone.set_visible(true);
 queue_scrolled_clone.set_visible(false);
 track_count_label_clone.set_visible(true);
 queue_count_label_clone.set_visible(false);
 queue_controls_clone.set_visible(false);
 track_header_clone.set_visible(true);
 }));

 let library_tab_clone = library_tab.clone();
 let queue_tab_clone = queue_tab.clone();
 let track_scrolled_clone = track_scrolled.clone();
 let queue_scrolled_clone = queue_scrolled.clone();
 let track_count_label_clone = track_count_label.clone();
 let queue_count_label_clone = queue_count_label.clone();
 let queue_controls_clone = queue_controls.clone();
 let track_header_clone = track_header.clone();

 queue_tab.connect_clicked(glib::clone!(
 @strong library_tab_clone,
 @strong queue_tab_clone,
 @strong track_scrolled_clone,
 @strong queue_scrolled_clone,
 @strong track_count_label_clone,
 @strong queue_count_label_clone,
 @strong queue_controls_clone,
 @strong track_header_clone => move |_| {
 queue_tab_clone.add_css_class("active");
 library_tab_clone.remove_css_class("active");
 queue_scrolled_clone.set_visible(true);
 track_scrolled_clone.set_visible(false);
 queue_count_label_clone.set_visible(true);
 track_count_label_clone.set_visible(false);
 queue_controls_clone.set_visible(true);
 track_header_clone.set_visible(false);
 }));

 // ========== RIGHT CONTENT ==========
 let content = Box::builder()
 .orientation(Orientation::Vertical)
 .spacing(4)
 .hexpand(true)
 .vexpand(true)
 .margin_top(4)
 .margin_bottom(4)
 .margin_start(4)
 .margin_end(4)
 .css_classes(vec!["content-area".to_string()])
 .build();

 // Now Playing Header
 let now_playing = Box::builder()
 .orientation(Orientation::Horizontal)
 .spacing(12)
 .margin_bottom(4)
 .css_classes(vec!["now-playing".to_string()])
 .build();

 let album_art = Image::builder()
 .width_request(80)
 .height_request(80)
 .css_classes(vec!["album-art".to_string()])
 .icon_name("audio-x-generic-symbolic")
 .build();

 let track_info = Box::builder()
 .orientation(Orientation::Vertical)
 .spacing(2)
 .valign(gtk4::Align::Center)
 .hexpand(true)
 .build();

 let track_title = Label::builder()
 .label("No track selected")
 .css_classes(vec!["track-title".to_string()])
 .halign(gtk4::Align::Start)
 .ellipsize(gtk4::pango::EllipsizeMode::End)
 .build();

 let track_artist = Label::builder()
 .label("Select a track to begin")
 .css_classes(vec!["track-artist".to_string()])
 .halign(gtk4::Align::Start)
 .ellipsize(gtk4::pango::EllipsizeMode::End)
 .build();

 track_info.append(&track_title);
 track_info.append(&track_artist);
 
 now_playing.append(&album_art);
 now_playing.append(&track_info);

 // Visualization area
 let viz_overlay = Overlay::builder().build();
 
 let viz_area = DrawingArea::builder()
 .css_classes(vec!["card".to_string(), "viz-area".to_string()])
 .vexpand(true)
 .hexpand(true)
 .margin_bottom(4)
 .build();

 let state_clone = state.clone();
 viz_area.set_draw_func(move |area, cr, width, height| {
 draw_visualization(&state_clone, area, cr, width, height);
 });

 // Viz mode button
 let viz_mode_btn = Button::builder()
 .label("Bars")
 .css_classes(vec!["pill".to_string(), "viz-mode-btn".to_string()])
 .halign(gtk4::Align::End)
 .valign(gtk4::Align::Start)
 .margin_top(8)
 .margin_end(8)
 .build();

 viz_overlay.set_child(Some(&viz_area));
 viz_overlay.add_overlay(&viz_mode_btn);

 // Lyrics display
 let lyrics_box = Box::builder()
 .orientation(Orientation::Vertical)
 .spacing(2)
 .halign(gtk4::Align::Center)
 .css_classes(vec!["lyrics-container".to_string()])
 .margin_top(4)
 .margin_bottom(4)
 .height_request(80)
 .width_request(400)
 .hexpand(false)
 .build();

 let lyrics_prev = Label::builder()
 .css_classes(vec!["lyrics-dim".to_string()])
 .halign(gtk4::Align::Center)
 .label("")
 .width_chars(50)
 .ellipsize(gtk4::pango::EllipsizeMode::End)
 .build();

 let lyrics_current = Label::builder()
 .css_classes(vec!["lyrics-current".to_string()])
 .halign(gtk4::Align::Center)
 .label("Ready to play")
 .width_chars(50)
 .ellipsize(gtk4::pango::EllipsizeMode::End)
 .build();

 let lyrics_next = Label::builder()
 .css_classes(vec!["lyrics-dim".to_string()])
 .halign(gtk4::Align::Center)
 .label("")
 .width_chars(50)
 .ellipsize(gtk4::pango::EllipsizeMode::End)
 .build();

 lyrics_box.append(&lyrics_prev);
 lyrics_box.append(&lyrics_current);
 lyrics_box.append(&lyrics_next);

 // Player controls
 let controls = Box::builder()
 .orientation(Orientation::Horizontal)
 .spacing(8)
 .halign(gtk4::Align::Center)
 .margin_top(8)
 .css_classes(vec!["controls".to_string()])
 .build();

 let shuffle_btn = Button::builder()
 .icon_name("media-playlist-shuffle-symbolic")
 .css_classes(vec!["control-btn".to_string()])
 .tooltip_text("Shuffle")
 .build();

 let prev_btn = Button::builder()
 .icon_name("media-skip-backward-symbolic")
 .css_classes(vec!["control-btn".to_string()])
 .build();

 let play_btn = Button::builder()
 .icon_name("media-playback-start-symbolic")
 .css_classes(vec!["play-btn".to_string()])
 .width_request(56)
 .height_request(56)
 .build();

 let next_btn = Button::builder()
 .icon_name("media-skip-forward-symbolic")
 .css_classes(vec!["control-btn".to_string()])
 .build();

 let repeat_btn = Button::builder()
 .icon_name("media-playlist-repeat-symbolic")
 .css_classes(vec!["control-btn".to_string()])
 .tooltip_text("Repeat")
 .build();

 controls.append(&shuffle_btn);
 controls.append(&prev_btn);
 controls.append(&play_btn);
 controls.append(&next_btn);
 controls.append(&repeat_btn);

 // Progress bar
 let progress_box = Box::builder()
 .orientation(Orientation::Horizontal)
 .spacing(8)
 .margin_top(8)
 .css_classes(vec!["progress-box".to_string()])
 .build();

 let time_current = Label::builder()
 .label("0:00")
 .css_classes(vec!["time-label".to_string()])
 .build();

 let progress = Scale::builder()
 .orientation(Orientation::Horizontal)
 .hexpand(true)
 .adjustment(&gtk4::Adjustment::new(0.0, 0.0, 100.0, 1.0, 10.0, 0.0))
 .css_classes(vec!["progress-scale".to_string()])
 .build();

 let time_total = Label::builder()
 .label("0:00")
 .css_classes(vec!["time-label".to_string()])
 .build();

 progress_box.append(&time_current);
 progress_box.append(&progress);
 progress_box.append(&time_total);

 // Volume control
 let volume_box = Box::builder()
 .orientation(Orientation::Horizontal)
 .spacing(4)
 .halign(gtk4::Align::End)
 .css_classes(vec!["volume-box".to_string()])
 .build();

 let volume_btn = Button::builder()
 .icon_name("audio-volume-high-symbolic")
 .css_classes(vec!["volume-icon".to_string()])
 .build();

 let volume_scale = Scale::builder()
 .orientation(Orientation::Horizontal)
 .width_request(120)
 .adjustment(&gtk4::Adjustment::new(0.75, 0.0, 1.0, 0.05, 0.1, 0.0))
 .css_classes(vec!["volume-scale".to_string()])
 .build();

 volume_box.append(&volume_btn);
 volume_box.append(&volume_scale);

 // Bottom row
 let bottom_row = Box::builder()
 .orientation(Orientation::Horizontal)
 .spacing(16)
 .margin_top(8)
 .build();

 bottom_row.append(&progress_box);
 bottom_row.append(&volume_box);

 // Status bar
 let status_bar = Box::builder()
 .orientation(Orientation::Horizontal)
 .spacing(8)
 .margin_top(8)
 .css_classes(vec!["status-bar".to_string()])
 .build();

 let status_theme = Label::builder()
 .label("Tokyo Night")
 .css_classes(vec!["status-item".to_string()])
 .halign(gtk4::Align::Start)
 .build();

 let status_viz = Label::builder()
 .label("Bars")
 .css_classes(vec!["status-item".to_string()])
 .halign(gtk4::Align::End)
 .hexpand(true)
 .build();

 status_bar.append(&status_theme);
 status_bar.append(&status_viz);

 // Assemble content
 content.append(&now_playing);
 content.append(&viz_overlay);
 content.append(&lyrics_box);
 content.append(&controls);
 content.append(&bottom_row);
 content.append(&status_bar);

 main_box.append(&sidebar);
 main_box.append(&gtk4::Separator::new(Orientation::Vertical));
 main_box.append(&content);

 window.set_child(Some(&main_box));

 // ========== GSTREAMER PIPELINE ==========
 let pipeline: Arc<Mutex<Option<gstreamer::Pipeline>>> = Arc::new(Mutex::new(None));

 // ========== CONTEXT MENU FOR TRACK LIST ==========
 let track_list_for_context = track_list.clone();
 let track_list_context = GestureClick::new();
 track_list_context.set_button(gtk4::gdk::BUTTON_SECONDARY);
 
 let state_for_context = state.clone();
 let pipeline_for_context = pipeline.clone();
 let track_rows_for_context = track_rows.clone();
 let queue_list_for_context = queue_list.clone();
 let queue_count_label_for_context = queue_count_label.clone();
 
 track_list_context.connect_pressed(glib::clone!(
 @strong track_list_for_context,
 @strong state_for_context,
 @strong pipeline_for_context,
 @strong track_rows_for_context,
 @strong queue_list_for_context,
 @strong queue_count_label_for_context => move |_, _, x, y| {
 if let Some(row) = track_list_for_context.row_at_y(x as i32) {
 let index = row.index() as usize;
 let state = state_for_context.lock().unwrap();
 let is_current = state.current_index == Some(index);
 
 let track = if let Some(t) = state.tracks.get(index) {
 t.clone()
 } else {
 return;
 };
 drop(state);
 
 show_track_context_menu(
 &row,
 index,
 is_current,
 track,
 state_for_context.clone(),
 pipeline_for_context.clone(),
 track_rows_for_context.clone(),
 queue_list_for_context.clone(),
 queue_count_label_for_context.clone(),
 );
 }
 }));
 
track_list.add_controller(track_list_context);

 // ========== CONNECT SIGNALS ==========
 
 // Viz mode button
 let state_clone = state.clone();
 let viz_area_clone = viz_area.clone();
 let status_viz_clone = status_viz.clone();
 viz_mode_btn.connect_clicked(glib::clone!(@strong state_clone, @strong viz_area_clone, @strong status_viz_clone => move |btn| {
 let mut s = state_clone.lock().unwrap();
 s.viz_mode = (s.viz_mode + 1) % 6;
 let mode_name = match s.viz_mode {
 0 => "Bars",
 1 => "Wave",
 2 => "Circles",
 3 => "Stars",
 4 => "Mirror",
 5 => "Spectrum",
 _ => "Bars",
 };
 btn.set_label(mode_name);
 status_viz_clone.set_label(mode_name);
 drop(s);
 viz_area_clone.queue_draw();
 }));

 // Track selection (double-click to play)
 let state_clone = state.clone();
 let pipeline_clone = pipeline.clone();
 let play_btn_clone = play_btn.clone();
 let lyrics_prev_clone = lyrics_prev.clone();
 let lyrics_current_clone = lyrics_current.clone();
 let lyrics_next_clone = lyrics_next.clone();
 let progress_clone = progress.clone();
 let time_current_clone = time_current.clone();
 let time_total_clone = time_total.clone();
 let track_title_clone = track_title.clone();
 let track_artist_clone = track_artist.clone();
 let album_art_clone = album_art.clone();
 let track_rows_clone = track_rows.clone();

 track_list.connect_row_activated(glib::clone!(
 @strong state_clone,
 @strong pipeline_clone,
 @strong play_btn_clone,
 @strong lyrics_prev_clone,
 @strong lyrics_current_clone,
 @strong lyrics_next_clone,
 @strong progress_clone,
 @strong time_current_clone,
 @strong time_total_clone,
 @strong track_title_clone,
 @strong track_artist_clone,
 @strong album_art_clone,
 @strong track_list,
 @strong track_rows_clone => move |_, row| {
 let index = row.index() as usize;
 let state = state_clone.lock().unwrap();
 
 if let Some(track) = state.tracks.get(index) {
 let track = track.clone();
 drop(state);
 
 state_clone.lock().unwrap().current_index = Some(index);
 track_list.select_row(track_list.row_at_index(index as i32).as_ref());
 
 // Update row highlighting
 highlight_current_track(&track_rows_clone, index);
 
 play_track(
 &track,
 pipeline_clone.clone(),
 play_btn_clone.clone(),
 lyrics_prev_clone.clone(),
 lyrics_current_clone.clone(),
 lyrics_next_clone.clone(),
 progress_clone.clone(),
 time_current_clone.clone(),
 time_total_clone.clone(),
 track_title_clone.clone(),
 track_artist_clone.clone(),
 album_art_clone.clone(),
 state_clone.clone(),
 );
 }
 }));

 // Queue list row activation - play the selected queue item
 let pipeline_clone = pipeline.clone();
 let play_btn_clone = play_btn.clone();
 let state_clone = state.clone();
 let lyrics_prev_clone = lyrics_prev.clone();
 let lyrics_current_clone = lyrics_current.clone();
 let lyrics_next_clone = lyrics_next.clone();
 let progress_clone = progress.clone();
 let time_current_clone = time_current.clone();
 let time_total_clone = time_total.clone();
 let track_title_clone = track_title.clone();
 let track_artist_clone = track_artist.clone();
 let album_art_clone = album_art.clone();

 queue_list.connect_row_activated(glib::clone!(
 @strong state_clone,
 @strong pipeline_clone,
 @strong play_btn_clone,
 @strong lyrics_prev_clone,
 @strong lyrics_current_clone,
 @strong lyrics_next_clone,
 @strong progress_clone,
 @strong time_current_clone,
 @strong time_total_clone,
 @strong track_title_clone,
 @strong track_artist_clone,
 @strong album_art_clone => move |_, row| {
 let row_index = row.index() as usize;
 let state = state_clone.lock().unwrap();
 
 // Get the track index from the queue
 if let Some(&track_idx) = state.queue.iter().nth(row_index) {
 if let Some(track) = state.tracks.get(track_idx) {
 let track = track.clone();
 drop(state);
 
 state_clone.lock().unwrap().current_index = Some(track_idx);
 
 play_track(
 &track,
 pipeline_clone.clone(),
 play_btn_clone.clone(),
 lyrics_prev_clone.clone(),
 lyrics_current_clone.clone(),
 lyrics_next_clone.clone(),
 progress_clone.clone(),
 time_current_clone.clone(),
 time_total_clone.clone(),
 track_title_clone.clone(),
 track_artist_clone.clone(),
 album_art_clone.clone(),
 state_clone.clone(),
 );
 }
 }
 }));

 // Play/pause button
 let pipeline_clone = pipeline.clone();
 let play_btn_clone = play_btn.clone();
 let state_clone = state.clone();

 play_btn.connect_clicked(glib::clone!(@strong pipeline_clone, @strong play_btn_clone, @strong state_clone => move |_| {
 let is_playing = state_clone.lock().unwrap().is_playing;
 if is_playing {
 if let Some(p) = pipeline_clone.lock().unwrap().as_ref() {
 p.set_state(gstreamer::State::Paused).ok();
 }
 play_btn_clone.set_icon_name("media-playback-start-symbolic");
 state_clone.lock().unwrap().is_playing = false;
 } else {
 if let Some(p) = pipeline_clone.lock().unwrap().as_ref() {
 p.set_state(gstreamer::State::Playing).ok();
 }
 play_btn_clone.set_icon_name("media-playback-pause-symbolic");
 state_clone.lock().unwrap().is_playing = true;
 }
 }));

 // Previous track
 let track_list_clone = track_list.clone();
 let state_clone = state.clone();
 let pipeline_clone = pipeline.clone();
 let play_btn_clone = play_btn.clone();
 let lyrics_prev_clone = lyrics_prev.clone();
 let lyrics_current_clone = lyrics_current.clone();
 let lyrics_next_clone = lyrics_next.clone();
 let progress_clone = progress.clone();
 let time_current_clone = time_current.clone();
 let time_total_clone = time_total.clone();
 let track_title_clone = track_title.clone();
 let track_artist_clone = track_artist.clone();
 let album_art_clone = album_art.clone();
 let track_rows_clone = track_rows.clone();

 prev_btn.connect_clicked(glib::clone!(
 @strong track_list_clone,
 @strong state_clone,
 @strong pipeline_clone,
 @strong play_btn_clone,
 @strong lyrics_prev_clone,
 @strong lyrics_current_clone,
 @strong lyrics_next_clone,
 @strong progress_clone,
 @strong time_current_clone,
 @strong time_total_clone,
 @strong track_title_clone,
 @strong track_artist_clone,
 @strong album_art_clone,
 @strong track_rows_clone => move |_| {
 let s = state_clone.lock().unwrap();
 if let Some(idx) = s.current_index {
 let new_idx = if idx > 0 { idx - 1 } else { s.tracks.len() - 1 };
 if let Some(track) = s.tracks.get(new_idx) {
 let track = track.clone();
 drop(s);
 state_clone.lock().unwrap().current_index = Some(new_idx);
 track_list_clone.select_row(track_list_clone.row_at_index(new_idx as i32).as_ref());
 highlight_current_track(&track_rows_clone, new_idx);
 play_track(
 &track,
 pipeline_clone.clone(),
 play_btn_clone.clone(),
 lyrics_prev_clone.clone(),
 lyrics_current_clone.clone(),
 lyrics_next_clone.clone(),
 progress_clone.clone(),
 time_current_clone.clone(),
 time_total_clone.clone(),
 track_title_clone.clone(),
 track_artist_clone.clone(),
 album_art_clone.clone(),
 state_clone.clone(),
 );
 }
 }
 }));

 // Next track
 let track_list_clone = track_list.clone();
 let state_clone = state.clone();
 let pipeline_clone = pipeline.clone();
 let play_btn_clone = play_btn.clone();
 let lyrics_prev_clone = lyrics_prev.clone();
 let lyrics_current_clone = lyrics_current.clone();
 let lyrics_next_clone = lyrics_next.clone();
 let progress_clone = progress.clone();
 let time_current_clone = time_current.clone();
 let time_total_clone = time_total.clone();
 let track_title_clone = track_title.clone();
 let track_artist_clone = track_artist.clone();
 let album_art_clone = album_art.clone();
 let track_rows_clone = track_rows.clone();

 next_btn.connect_clicked(glib::clone!(
 @strong track_list_clone,
 @strong state_clone,
 @strong pipeline_clone,
 @strong play_btn_clone,
 @strong lyrics_prev_clone,
 @strong lyrics_current_clone,
 @strong lyrics_next_clone,
 @strong progress_clone,
 @strong time_current_clone,
 @strong time_total_clone,
 @strong track_title_clone,
 @strong track_artist_clone,
 @strong album_art_clone,
 @strong track_rows_clone => move |_| {
 let s = state_clone.lock().unwrap();
 let shuffle_mode = s.shuffle_mode;
 let repeat_mode = s.repeat_mode;
 let track_count = s.tracks.len();
 
 if let Some(idx) = s.current_index {
 let new_idx = if shuffle_mode {
 let mut rng = rand::thread_rng();
 let mut next = rng.gen_range(0..track_count);
 if next == idx && track_count > 1 {
 next = (next + 1) % track_count;
 }
 next
 } else {
 if idx + 1 < track_count {
 idx + 1
 } else if repeat_mode == RepeatMode::All {
 0
 } else {
 return;
 }
 };
 
 if let Some(track) = s.tracks.get(new_idx) {
 let track = track.clone();
 drop(s);
 state_clone.lock().unwrap().current_index = Some(new_idx);
 track_list_clone.select_row(track_list_clone.row_at_index(new_idx as i32).as_ref());
 highlight_current_track(&track_rows_clone, new_idx);
 play_track(
 &track,
 pipeline_clone.clone(),
 play_btn_clone.clone(),
 lyrics_prev_clone.clone(),
 lyrics_current_clone.clone(),
 lyrics_next_clone.clone(),
 progress_clone.clone(),
 time_current_clone.clone(),
 time_total_clone.clone(),
 track_title_clone.clone(),
 track_artist_clone.clone(),
 album_art_clone.clone(),
 state_clone.clone(),
 );
 }
 }
 }));

 // Shuffle toggle
 let shuffle_btn_clone = shuffle_btn.clone();
 let state_clone = state.clone();
 shuffle_btn.connect_clicked(glib::clone!(@strong shuffle_btn_clone, @strong state_clone => move |_| {
 let mut s = state_clone.lock().unwrap();
 s.shuffle_mode = !s.shuffle_mode;
 if s.shuffle_mode {
 shuffle_btn_clone.add_css_class("active");
 shuffle_btn_clone.set_tooltip_text(Some("Shuffle: ON"));
 } else {
 shuffle_btn_clone.remove_css_class("active");
 shuffle_btn_clone.set_tooltip_text(Some("Shuffle: OFF"));
 }
 }));

 // Repeat toggle
 let repeat_btn_clone = repeat_btn.clone();
 let state_clone = state.clone();
 repeat_btn.connect_clicked(glib::clone!(@strong repeat_btn_clone, @strong state_clone => move |_| {
 let mut s = state_clone.lock().unwrap();
 s.repeat_mode = match s.repeat_mode {
 RepeatMode::Off => RepeatMode::All,
 RepeatMode::All => RepeatMode::One,
 RepeatMode::One => RepeatMode::Off,
 };
 let (icon, tooltip) = match s.repeat_mode {
 RepeatMode::Off => ("media-playlist-repeat-symbolic", "Repeat: Off"),
 RepeatMode::All => ("media-playlist-repeat-symbolic", "Repeat: All"),
 RepeatMode::One => ("media-playlist-repeat-song-symbolic", "Repeat: One"),
 };
 repeat_btn_clone.set_icon_name(icon);
 repeat_btn_clone.set_tooltip_text(Some(tooltip));
 if s.repeat_mode != RepeatMode::Off {
 repeat_btn_clone.add_css_class("active");
 } else {
 repeat_btn_clone.remove_css_class("active");
 }
 }));

 // Search
 let track_rows_clone = track_rows.clone();
 let tracks_clone = tracks.clone();
 search_entry.connect_changed(glib::clone!(@strong track_rows_clone, @strong tracks_clone => move |entry| {
 let query = entry.text().to_lowercase();
 let rows = track_rows_clone.lock().unwrap();
 for (i, row) in rows.iter().enumerate() {
 let track = &tracks_clone[i];
 let matches = query.is_empty() || 
 track.title.to_lowercase().contains(&query) ||
 track.artist.to_lowercase().contains(&query);
 if matches { row.show(); } else { row.hide(); }
 }
 }));

 // Volume
 let pipeline_clone = pipeline.clone();
 let volume_btn_clone = volume_btn.clone();
 volume_scale.connect_value_changed(glib::clone!(@strong pipeline_clone, @strong volume_btn_clone, @strong state => move |scale| {
 let vol = scale.value();
 state.lock().unwrap().volume = vol;
 if let Some(p) = pipeline_clone.lock().unwrap().as_ref() {
 if let Some(playbin) = p.by_name("playbin") {
 playbin.set_property("volume", vol);
 }
 }
 let icon = if vol == 0.0 {
 "audio-volume-muted-symbolic"
 } else if vol < 0.33 {
 "audio-volume-low-symbolic"
 } else if vol < 0.66 {
 "audio-volume-medium-symbolic"
 } else {
 "audio-volume-high-symbolic"
 };
 volume_btn_clone.set_icon_name(icon);
 }));

 // Volume mute toggle
 let volume_scale_clone = volume_scale.clone();
 volume_btn.connect_clicked(glib::clone!(@strong volume_scale_clone, @strong state => move |_| {
 let mut s = state.lock().unwrap();
 if s.volume > 0.0 {
 s.volume = 0.0;
 volume_scale_clone.set_value(0.0);
 } else {
 s.volume = 0.75;
 volume_scale_clone.set_value(0.75);
 }
 }));

 // Playlist save button
 let tracks_for_save = tracks.clone();
 let state_for_save = state.clone();
 save_playlist_btn.connect_clicked(glib::clone!(@strong tracks_for_save, @strong state_for_save => move |_| {
 let dialog = gtk4::MessageDialog::builder()
 .text("Save Playlist")
 .secondary_text("Enter playlist name:")
 .build();
 
 dialog.add_button("Save", gtk4::ResponseType::Ok);
 dialog.add_button("Cancel", gtk4::ResponseType::Cancel);
 
 let entry = gtk4::Entry::new();
 entry.set_placeholder_text(Some("My Playlist"));
 entry.set_hexpand(true);
 
 let content = dialog.content_area();
 content.append(&entry);
 
 let tracks = tracks_for_save.clone();
 let state = state_for_save.clone();
 dialog.connect_response(glib::clone!(@strong entry => move |dialog, response| {
 if response == gtk4::ResponseType::Ok {
 let name = entry.text().to_string();
 if !name.is_empty() {
 let s = state.lock().unwrap();
 let tracks_ref: Vec<Track> = s.tracks.iter().map(|t| Track {
 path: t.path.clone(),
 title: t.title.clone(),
 artist: t.artist.clone(),
 duration_secs: t.duration_secs,
 }).collect();
 drop(s);
 if let Err(e) = save_playlist(&tracks_ref, &name) {
 eprintln!("Failed to save playlist: {}", e);
 }
 }
 }
 dialog.close();
 }));
 
 dialog.show();
 }));

 // Playlist load button
 let track_list_for_load = track_list.clone();
 let track_rows_for_load = track_rows.clone();
 let track_count_label_for_load = track_count_label.clone();
 let state_for_load = state.clone();
 
 load_playlist_btn.connect_clicked(glib::clone!(@strong track_list_for_load, @strong track_rows_for_load, @strong track_count_label_for_load, @strong state_for_load => move |_| {
 let playlists = list_playlists();
 if playlists.is_empty() {
 let dialog = gtk4::MessageDialog::builder()
 .text("No Playlists")
 .secondary_text("No saved playlists found. Save a playlist first.")
 .build();
 dialog.add_button("OK", gtk4::ResponseType::Ok);
 dialog.connect_response(|d, _| d.close());
 dialog.show();
 return;
 }
 
 let dialog = gtk4::MessageDialog::builder()
 .text("Load Playlist")
 .secondary_text("Select a playlist to load:")
 .build();
 
 dialog.add_button("Load", gtk4::ResponseType::Ok);
 dialog.add_button("Cancel", gtk4::ResponseType::Cancel);
 
 let listbox = gtk4::ListBox::new();
 listbox.set_selection_mode(gtk4::SelectionMode::Single);
 for name in &playlists {
 let label = gtk4::Label::new(Some(name));
 label.set_halign(gtk4::Align::Start);
 let row = gtk4::ListBoxRow::new();
 row.set_child(Some(&label));
 listbox.append(&row);
 }
 
 let content = dialog.content_area();
 content.append(&listbox);
 
 let listbox_clone = listbox.clone();
 let track_list_clone = track_list_for_load.clone();
 let track_rows_clone = track_rows_for_load.clone();
 let track_count_label_clone = track_count_label_for_load.clone();
 let state_clone = state_for_load.clone();
 
 dialog.connect_response(glib::clone!(@strong listbox_clone => move |dialog, response| {
 if response == gtk4::ResponseType::Ok {
 if let Some(row) = listbox_clone.selected_row() {
 if let Some(label) = row.child().and_then(|c| c.downcast::<gtk4::Label>().ok()) {
 let name = label.text().to_string();
 if let Ok(loaded_tracks) = load_playlist(&name) {
 // Clear existing rows
 let mut rows = track_rows_clone.lock().unwrap();
 for row in rows.iter() {
 track_list_clone.remove(row);
 }
 rows.clear();
 
 // Update state
 state_clone.lock().unwrap().tracks = loaded_tracks.clone();
 
 // Add loaded tracks
 for track in &loaded_tracks {
 let (row, _add_btn) = create_track_row(track);
 track_list_clone.append(&row);
 rows.push(row);
 }
 
 track_count_label_clone.set_label(&format!("{} tracks", loaded_tracks.len()));
 }
 }
 }
 }
 dialog.close();
 }));
 
 dialog.show();
 }));

 // Clear queue button
 let queue_list_clone = queue_list.clone();
 let queue_count_label_clone = queue_count_label.clone();
 let state_clone = state.clone();
 
 clear_queue_btn.connect_clicked(glib::clone!(@strong queue_list_clone, @strong queue_count_label_clone, @strong state_clone => move |_| {
 state_clone.lock().unwrap().queue.clear();
 
 // Clear queue list UI
 while let Some(row) = queue_list_clone.first_child() {
 queue_list_clone.remove(&row);
 }
 
 queue_count_label_clone.set_label("Queue: 0");
 }));

 // Mini view toggle
 let window_clone = window.clone();
 let state_clone = state.clone();
 let sidebar_clone = sidebar.clone();
 let content_clone = content.clone();
 let main_box_clone = main_box.clone();
 
 mini_view_btn.connect_clicked(glib::clone!(@strong window_clone, @strong state_clone, @strong sidebar_clone, @strong content_clone, @strong main_box_clone, @strong mini_view_btn => move |_| {
 let mut s = state_clone.lock().unwrap();
 
 if s.view_mode == ViewMode::Full {
 // Switch to mini view
 s.view_mode = ViewMode::Mini;
 
 // Save window position (using Window trait)
 let window_ext: &Window = window_clone.upcast_ref();
 s.window_x = window_ext.default_width();
 s.window_y = window_ext.default_height();
 drop(s);
 
 // Hide sidebar and extra elements
 sidebar_clone.set_visible(false);
 
 // Resize to mini
 window_clone.set_default_width(400);
 window_clone.set_default_height(150);
 window_clone.set_resizable(false);
 
 mini_view_btn.set_icon_name("view-restore-symbolic");
 mini_view_btn.set_tooltip_text(Some("Full View (D)"));
 } else {
 // Switch to full view
 s.view_mode = ViewMode::Full;
 drop(s);
 
 sidebar_clone.set_visible(true);
 
 window_clone.set_default_width(1200);
 window_clone.set_default_height(800);
 window_clone.set_resizable(true);
 
 mini_view_btn.set_icon_name("view-fullscreen-symbolic");
 mini_view_btn.set_tooltip_text(Some("Mini View (D)"));
 }
 }));

 // Keyboard shortcuts
 let pipeline_clone = pipeline.clone();
 let state_clone = state.clone();
 let play_btn_clone = play_btn.clone();
 let mini_view_btn_clone = mini_view_btn.clone();
 let viz_mode_btn_clone = viz_mode_btn.clone();
 let viz_area_clone = viz_area.clone();
 let window_clone = window.clone();

 key_controller.connect_key_pressed(glib::clone!(@strong pipeline_clone, @strong state_clone, @strong play_btn_clone, @strong mini_view_btn_clone, @strong viz_mode_btn_clone, @strong viz_area_clone, @strong window_clone => move |_, key, _, _| {
 match key {
 gtk4::gdk::Key::space => {
 let is_playing = state_clone.lock().unwrap().is_playing;
 if is_playing {
 if let Some(p) = pipeline_clone.lock().unwrap().as_ref() {
 p.set_state(gstreamer::State::Paused).ok();
 }
 play_btn_clone.set_icon_name("media-playback-start-symbolic");
 state_clone.lock().unwrap().is_playing = false;
 } else {
 if let Some(p) = pipeline_clone.lock().unwrap().as_ref() {
 p.set_state(gstreamer::State::Playing).ok();
 }
 play_btn_clone.set_icon_name("media-playback-pause-symbolic");
 state_clone.lock().unwrap().is_playing = true;
 }
 Propagation::Stop
 }
 gtk4::gdk::Key::Left | gtk4::gdk::Key::a => {
 // Seek backward
 if let Some(p) = pipeline_clone.lock().unwrap().as_ref() {
 if let Some(playbin) = p.by_name("playbin") {
 if let Some(pos) = playbin.query_position::<gstreamer::ClockTime>() {
 let new_pos = pos.saturating_sub(gstreamer::ClockTime::from_seconds(5));
 playbin.seek_simple(gstreamer::SeekFlags::FLUSH, new_pos).ok();
 }
 }
 }
 Propagation::Stop
 }
 gtk4::gdk::Key::Right => {
 // Seek forward
 if let Some(p) = pipeline_clone.lock().unwrap().as_ref() {
 if let Some(playbin) = p.by_name("playbin") {
 if let Some(pos) = playbin.query_position::<gstreamer::ClockTime>() {
 let new_pos = pos.saturating_add(gstreamer::ClockTime::from_seconds(5));
 playbin.seek_simple(gstreamer::SeekFlags::FLUSH, new_pos).ok();
 }
 }
 }
 Propagation::Stop
 }
 gtk4::gdk::Key::d => {
 // Toggle mini view
 mini_view_btn_clone.emit_clicked();
 Propagation::Stop
 }
 gtk4::gdk::Key::v => {
 // Cycle visualization mode
 let mut s = state_clone.lock().unwrap();
 s.viz_mode = (s.viz_mode + 1) % 6;
 let mode_name = match s.viz_mode {
 0 => "Bars",
 1 => "Wave",
 2 => "Circles",
 3 => "Stars",
 4 => "Mirror",
 5 => "Spectrum",
 _ => "Bars",
 };
 drop(s);
 viz_mode_btn_clone.set_label(mode_name);
 viz_area_clone.queue_draw();
 Propagation::Stop
 }
 gtk4::gdk::Key::t => {
 // Cycle theme
 let mut s = state_clone.lock().unwrap();
 s.current_theme = (s.current_theme + 1) % THEMES.len();
 let theme_idx = s.current_theme;
 drop(s);
 apply_theme(theme_idx);
 viz_area_clone.queue_draw();
 window_clone.queue_draw();
 Propagation::Stop
 }
 _ => Propagation::Proceed
 }
 }));

 window.add_controller(key_controller);

 // ========== TIMERS ==========
 
 // Animation timer
 let viz_area_clone = viz_area.clone();
 glib::timeout_add_local(std::time::Duration::from_millis(33), glib::clone!(@strong state, @strong viz_area_clone => move || {
 let mut s = state.lock().unwrap();
 
 if s.is_playing {
 for i in 0..64 {
 let base = (i as f64 / 64.0).powf(0.5);
 let noise = rand::thread_rng().gen::<f64>() * 0.3;
 let target = base * 0.4 + noise + 0.1;
 
 s.spectrum_data[i] = s.spectrum_data[i] * 0.6 + target * 0.4;
 
 if s.spectrum_data[i] > s.peak_data[i] {
 s.peak_data[i] = s.spectrum_data[i];
 } else {
 s.peak_data[i] = s.peak_data[i] * 0.95;
 }
 }
 } else {
 for i in 0..64 {
 s.spectrum_data[i] *= 0.9;
 s.peak_data[i] *= 0.95;
 }
 }
 
 drop(s);
 viz_area_clone.queue_draw();
 glib::ControlFlow::Continue
 }));

 // Sync timer for lyrics and progress
 let pipeline_clone = pipeline.clone();
 let state_clone = state.clone();
 let lyrics_prev_clone = lyrics_prev.clone();
 let lyrics_current_clone = lyrics_current.clone();
 let lyrics_next_clone = lyrics_next.clone();
 let progress_clone = progress.clone();
 let time_current_clone = time_current.clone();
 let time_total_clone = time_total.clone();
 let track_list_clone = track_list.clone();
 let play_btn_clone = play_btn.clone();
 let track_title_clone = track_title.clone();
 let track_artist_clone = track_artist.clone();
 let album_art_clone = album_art.clone();
 let track_rows_clone = track_rows.clone();

 glib::timeout_add_local(std::time::Duration::from_millis(100), glib::clone!(
 @strong pipeline_clone,
 @strong state_clone,
 @strong lyrics_prev_clone,
 @strong lyrics_current_clone,
 @strong lyrics_next_clone,
 @strong progress_clone,
 @strong time_current_clone,
 @strong time_total_clone,
 @strong track_list_clone,
 @strong play_btn_clone,
 @strong track_title_clone,
 @strong track_artist_clone,
 @strong album_art_clone,
 @strong track_rows_clone => move || {
 let (position_ms, duration_ms) = if let Some(p) = pipeline_clone.lock().unwrap().as_ref() {
 let playbin = p.by_name("playbin").unwrap();
 let pos = playbin.query_position::<gstreamer::ClockTime>()
 .map(|t| t.nseconds() / 1_000_000)
 .unwrap_or(0);
 let dur = playbin.query_duration::<gstreamer::ClockTime>()
 .map(|t| t.nseconds() / 1_000_000)
 .unwrap_or(0);
 (pos, dur)
 } else {
 (0, 0)
 };

 // Update progress
 if duration_ms > 0 {
 let progress_adj = progress_clone.adjustment();
 progress_adj.set_upper(duration_ms as f64 / 1000.0);
 progress_adj.set_value(position_ms as f64 / 1000.0);

 let pos_min = position_ms / 60000;
 let pos_sec = (position_ms % 60000) / 1000;
 let dur_min = duration_ms / 60000;
 let dur_sec = (duration_ms % 60000) / 1000;
 time_current_clone.set_label(&format!("{}:{:02}", pos_min, pos_sec));
 time_total_clone.set_label(&format!("{}:{:02}", dur_min, dur_sec));
 }

 // Update lyrics
 let state = state_clone.lock().unwrap();
 let lyrics = &state.lyrics;
 
 if !lyrics.is_empty() {
 let mut current_idx = 0;
 for (i, line) in lyrics.iter().enumerate() {
 if line.time_ms <= position_ms as f64 {
 current_idx = i;
 } else {
 break;
 }
 }

 if current_idx > 0 {
 lyrics_prev_clone.set_label(&lyrics[current_idx - 1].text);
 } else {
 lyrics_prev_clone.set_label("");
 }

 lyrics_current_clone.set_label(&lyrics[current_idx].text);

 if current_idx + 1 < lyrics.len() {
 lyrics_next_clone.set_label(&lyrics[current_idx + 1].text);
 } else {
 lyrics_next_clone.set_label("");
 }
 }

 // Auto-advance on track end
 if duration_ms > 0 && position_ms >= duration_ms - 100 && state.current_index.is_some() {
 let current_idx = state.current_index;
 let shuffle_mode = state.shuffle_mode;
 let repeat_mode = state.repeat_mode;
 let track_count = state.tracks.len();
 drop(state);
 
 // Handle next track
 let new_idx = if shuffle_mode {
 let mut rng = rand::thread_rng();
 let mut next = rng.gen_range(0..track_count);
 if let Some(idx) = current_idx {
 if next == idx && track_count > 1 {
 next = (next + 1) % track_count;
 }
 }
 next
 } else if let Some(idx) = current_idx {
 if idx + 1 < track_count {
 idx + 1
 } else if repeat_mode == RepeatMode::All {
 0
 } else if repeat_mode == RepeatMode::One {
 idx
 } else {
 return glib::ControlFlow::Continue;
 }
 } else {
 return glib::ControlFlow::Continue;
 };
 
 let s = state_clone.lock().unwrap();
 if let Some(track) = s.tracks.get(new_idx) {
 let track = track.clone();
 drop(s);
 state_clone.lock().unwrap().current_index = Some(new_idx);
 track_list_clone.select_row(track_list_clone.row_at_index(new_idx as i32).as_ref());
 highlight_current_track(&track_rows_clone, new_idx);
 play_track(
 &track,
 pipeline_clone.clone(),
 play_btn_clone.clone(),
 lyrics_prev_clone.clone(),
 lyrics_current_clone.clone(),
 lyrics_next_clone.clone(),
 progress_clone.clone(),
 time_current_clone.clone(),
 time_total_clone.clone(),
 track_title_clone.clone(),
 track_artist_clone.clone(),
 album_art_clone.clone(),
 state_clone.clone(),
 );
 }
 }

 glib::ControlFlow::Continue
 }));

 window.present();
}

fn show_track_context_menu(
 row: &ListBoxRow,
 index: usize,
 is_current: bool,
 track: Track,
 state: Arc<Mutex<AppState>>,
 pipeline: Arc<Mutex<Option<gstreamer::Pipeline>>>,
 track_rows: Arc<Mutex<Vec<ListBoxRow>>>,
 queue_list: ListBox,
 queue_count_label: Label,
) {
 let popover = Popover::new();
 let vbox = Box::builder()
 .orientation(Orientation::Vertical)
 .spacing(2)
 .margin_top(4)
 .margin_bottom(4)
 .margin_start(4)
 .margin_end(4)
 .build();

 // Add to Queue
 let add_queue_btn = Button::builder()
 .label("Add to Queue")
 .css_classes(vec!["flat".to_string(), "context-btn".to_string()])
 .build();
 
 let state_clone = state.clone();
 let queue_list_clone = queue_list.clone();
 let queue_count_label_clone = queue_count_label.clone();
 let track_clone = track.clone();
 
 add_queue_btn.connect_clicked(glib::clone!(@strong state_clone, @strong queue_list_clone, @strong queue_count_label_clone, @strong track_clone, @strong popover => move |_| {
 let mut s = state_clone.lock().unwrap();
 
 // Add track to library if not present
 let new_idx = s.tracks.len();
 s.tracks.push(track_clone.clone());
 s.queue.push_back(new_idx);
 
 // Add to queue UI
 let queue_row = create_queue_row(&track_clone, new_idx, queue_list_clone.clone(), state_clone.clone());
 queue_list_clone.append(&queue_row);
 
 let queue_len = s.queue.len();
 drop(s);
 queue_count_label_clone.set_label(&format!("Queue: {}", queue_len));
 popover.popdown();
 }));
 
 vbox.append(&add_queue_btn);

 // Play Next
 let play_next_btn = Button::builder()
 .label("Play Next")
 .css_classes(vec!["flat".to_string(), "context-btn".to_string()])
 .build();
 
 let state_clone = state.clone();
 let queue_list_clone = queue_list.clone();
 let queue_count_label_clone = queue_count_label.clone();
 let track_clone = track.clone();
 
 play_next_btn.connect_clicked(glib::clone!(@strong state_clone, @strong queue_list_clone, @strong queue_count_label_clone, @strong track_clone, @strong popover => move |_| {
 let mut s = state_clone.lock().unwrap();
 
 // Add track to library if not present
 let new_idx = s.tracks.len();
 s.tracks.push(track_clone.clone());
 
 // Add to front of queue
 if let Some(front) = s.queue.front().copied() {
 s.queue.push_front(new_idx);
 } else {
 s.queue.push_front(new_idx);
 }
 
 // Add to queue UI
 let queue_row = create_queue_row(&track_clone, new_idx, queue_list_clone.clone(), state_clone.clone());
 queue_list_clone.prepend(&queue_row);
 
 let queue_len = s.queue.len();
 drop(s);
 queue_count_label_clone.set_label(&format!("Queue: {}", queue_len));
 popover.popdown();
 }));
 
 vbox.append(&play_next_btn);

 // Separator
 vbox.append(&Separator::new(Orientation::Horizontal));

 // Remove
 let remove_btn = Button::builder()
 .label(if is_current { "Remove (Current Track)" } else { "Remove" })
 .css_classes(vec!["flat".to_string(), "context-btn".to_string(), "destructive".to_string()])
 .build();
 
 let state_clone = state.clone();
 let track_rows_clone = track_rows.clone();
 let index_clone = index;
 let is_current_clone = is_current;
 let popover_clone = popover.clone();
 
 remove_btn.connect_clicked(glib::clone!(@strong state_clone, @strong track_rows_clone, @strong popover_clone => move |_| {
 if is_current_clone {
 // Show confirmation dialog
 let dialog = gtk4::MessageDialog::builder()
 .text("Remove Current Track?")
 .secondary_text("The currently playing track will be stopped and removed.")
 .build();
 dialog.add_button("Cancel", gtk4::ResponseType::Cancel);
 dialog.add_button("Remove", gtk4::ResponseType::Ok);
 
 dialog.connect_response(glib::clone!(@strong state_clone, @strong track_rows_clone, @strong index_clone, @strong popover_clone => move |dialog, response| {
 if response == gtk4::ResponseType::Ok {
 // Note: We can't stop playback from here without pipeline reference
 // The track will be removed but playback might continue
 
 // Remove track
 let mut s = state_clone.lock().unwrap();
 s.tracks.remove(index_clone);
 if let Some(ref mut idx) = s.current_index {
 if *idx > index_clone {
 *idx -= 1;
 } else if *idx == index_clone {
 s.current_index = None;
 }
 }
 drop(s);
 
 // Remove row from UI
 let mut rows = track_rows_clone.lock().unwrap();
 if index_clone < rows.len() {
 rows.remove(index_clone);
 }
 }
 dialog.close();
 popover_clone.popdown();
 }));
 
 dialog.show();
 } else {
 // Just remove without confirmation
 let mut s = state_clone.lock().unwrap();
 s.tracks.remove(index_clone);
 if let Some(ref mut idx) = s.current_index {
 if *idx > index_clone {
 *idx -= 1;
 }
 }
 drop(s);
 
 let mut rows = track_rows_clone.lock().unwrap();
 if index_clone < rows.len() {
 rows.remove(index_clone);
 }
 
 popover_clone.popdown();
 }
 }));
 
 vbox.append(&remove_btn);

 popover.set_child(Some(&vbox));
 popover.set_parent(row);
 popover.popup();
}

fn create_queue_row(track: &Track, index: usize, queue_list: ListBox, state: Arc<Mutex<AppState>>) -> ListBoxRow {
 let hbox = Box::builder()
 .orientation(Orientation::Horizontal)
 .spacing(8)
 .margin_top(4)
 .margin_bottom(4)
 .margin_start(8)
 .margin_end(8)
 .build();

 let info_box = Box::builder()
 .orientation(Orientation::Vertical)
 .spacing(2)
 .hexpand(true)
 .build();

 let title = Label::builder()
 .label(&track.title)
 .halign(gtk4::Align::Start)
 .ellipsize(gtk4::pango::EllipsizeMode::End)
 .build();

 let artist = Label::builder()
 .label(&track.artist)
 .halign(gtk4::Align::Start)
 .css_classes(vec!["dim-label".to_string()])
 .ellipsize(gtk4::pango::EllipsizeMode::End)
 .build();

 info_box.append(&title);
 info_box.append(&artist);
 
 hbox.append(&info_box);

 // Remove from queue button
 let remove_btn = Button::builder()
 .icon_name("list-remove-symbolic")
 .css_classes(vec!["flat".to_string(), "small-btn".to_string()])
 .build();
 
 let state_clone = state.clone();
 let queue_list_clone = queue_list.clone();
 let index_clone = index;
 
 remove_btn.connect_clicked(glib::clone!(@strong state_clone, @strong queue_list_clone => move |btn| {
 // Remove from queue
 let mut s = state_clone.lock().unwrap();
 s.queue.retain(|&i| i != index_clone);
 
 // Find parent row and remove
 if let Some(parent) = btn.parent() {
 if let Some(row) = parent.ancestor(ListBoxRow::static_type()).and_then(|w| w.downcast::<ListBoxRow>().ok()) {
 queue_list_clone.remove(&row);
 }
 }
 }));
 
 hbox.append(&remove_btn);

 ListBoxRow::builder().child(&hbox).build()
}

fn highlight_current_track(track_rows: &Arc<Mutex<Vec<ListBoxRow>>>, current_index: usize) {
 let rows = track_rows.lock().unwrap();
 for (i, row) in rows.iter().enumerate() {
 if i == current_index {
 row.add_css_class("playing");
 } else {
 row.remove_css_class("playing");
 }
 }
}

fn create_track_from_path(path: &str) -> Track {
 let path_buf = std::path::PathBuf::from(path);
 let filename = path_buf
 .file_stem()
 .and_then(|n| n.to_str())
 .unwrap_or("Unknown");
 
 let (artist, title) = if filename.contains(" - ") {
 let parts: Vec<&str> = filename.splitn(2, " - ").collect();
 (parts[0].to_string(), parts[1].to_string())
 } else {
 ("Unknown".to_string(), filename.to_string())
 };

 Track {
 path: path.to_string(),
 title,
 artist,
 duration_secs: None, // Could use lofty to extract
 }
}

fn play_track(
 track: &Track,
 pipeline: Arc<Mutex<Option<gstreamer::Pipeline>>>,
 play_btn: Button,
 lyrics_prev: Label,
 lyrics_current: Label,
 lyrics_next: Label,
 _progress: Scale,
 _time_current: Label,
 _time_total: Label,
 track_title: Label,
 track_artist: Label,
 album_art: Image,
 state: Arc<Mutex<AppState>>,
) {
 // Stop current pipeline
 if let Some(old_pipeline) = pipeline.lock().unwrap().take() {
 old_pipeline.set_state(gstreamer::State::Null).ok();
 }

 // Create new pipeline
 let new_pipeline = gstreamer::Pipeline::builder().name("audio-player").build();
 let playbin = gstreamer::ElementFactory::make("playbin")
 .name("playbin")
 .build()
 .expect("Could not create playbin");
 
 let uri = glib::filename_to_uri(&track.path, None).unwrap_or_default();
 playbin.set_property("uri", &uri);
 
 let volume = state.lock().unwrap().volume;
 playbin.set_property("volume", volume);

 new_pipeline.add(&playbin).unwrap();
 new_pipeline.set_state(gstreamer::State::Playing).ok();
 *pipeline.lock().unwrap() = Some(new_pipeline);

 // Update UI
 play_btn.set_icon_name("media-playback-pause-symbolic");
 state.lock().unwrap().is_playing = true;
 
 track_title.set_label(&track.title);
 track_artist.set_label(&track.artist);

 // Extract and display album art
 let path_clone = track.path.clone();
 let album_art_clone = album_art.clone();
 let texture = extract_album_art(&path_clone);
 if let Some(tex) = texture {
 album_art_clone.set_paintable(Some(&tex));
 album_art_clone.set_pixel_size(-1); // Clear pixel_size for paintable
 } else {
 album_art_clone.set_paintable(None::<&gtk4::gdk::Paintable>);
 album_art_clone.set_pixel_size(64); // Set size for icon
 album_art_clone.set_icon_name(Some("audio-x-generic-symbolic"));
 }

 // Load lyrics
 let path = track.path.clone();
 let state_clone = state.clone();
 let artist = track.artist.clone();
 let title = track.title.clone();
 
 lyrics_current.set_label("Loading lyrics...");
 lyrics_prev.set_label("");
 lyrics_next.set_label("");

 glib::spawn_future_local(async move {
 let lyrics = if let Ok(content) = std::fs::read_to_string(&format!("{}.lrc", path.rsplit_once('.').map(|(b, _)| b).unwrap_or(&path))) {
 parse_lrc(&content)
 } else {
 match fetch_lyrics(&artist, &title).await {
 Ok(l) => l,
 Err(_) => Vec::new(),
 }
 };

 let mut s = state_clone.lock().unwrap();
 s.lyrics = lyrics.into_iter().take(200).collect();
 
 if s.lyrics.is_empty() {
 drop(s);
 lyrics_current.set_label("No lyrics found");
 }
 });
}

fn extract_album_art(path: &str) -> Option<gtk4::gdk::Texture> {
 use lofty::probe::Probe;
 use lofty::picture::PictureType;
 use lofty::file::TaggedFileExt;
 use std::io::Write;
 
 // Try to read from audio file metadata
 if let Ok(tagged_file) = Probe::open(path).and_then(|p| p.read()) {
 // Check primary tag (ID3v2, Vorbis, etc.)
 if let Some(tag) = tagged_file.primary_tag() {
 for picture in tag.pictures() {
 if picture.pic_type() == PictureType::CoverFront || picture.pic_type() == PictureType::CoverBack {
 let data = picture.data();
 
 // Write to temp file and load from there
 if let Ok(temp_dir) = std::env::var("XDG_RUNTIME_DIR").or_else(|_| std::env::var("TMPDIR")) {
 let temp_path = std::path::Path::new(&temp_dir).join("volta_wave_cover_temp.jpg");
 if let Ok(mut file) = std::fs::File::create(&temp_path) {
 if file.write_all(data).is_ok() {
 let gio_file = gtk4::gio::File::for_path(&temp_path);
 if let Ok(texture) = gtk4::gdk::Texture::from_file(&gio_file) {
 let _ = std::fs::remove_file(&temp_path);
 return Some(texture);
 }
 }
 }
 }
 }
 }
 }
 }
 
 // Fallback: look for folder.jpg or cover.jpg in the same directory
 let dir = std::path::Path::new(path).parent()?;
 for cover_name in &["folder.jpg", "cover.jpg", "album.jpg", "Folder.jpg", "Cover.jpg"] {
 let cover_path = dir.join(cover_name);
 if cover_path.exists() {
 let gio_file = gtk4::gio::File::for_path(&cover_path);
 if let Ok(texture) = gtk4::gdk::Texture::from_file(&gio_file) {
 return Some(texture);
 }
 }
 }
 
 None
}

fn apply_theme(theme_index: usize) {
 let (name, bg, fg, accent, dim, sidebar_bg, viz_accent, _grad_start, _grad_end) = THEMES[theme_index];
 
let css = format!(r#"
* {{ -gtk-icon-transform: none; }}

window, 
window.background,
.background,
.main-container {{
 background-color: {bg};
}}

.content-area {{
 background-color: transparent;
}}

.sidebar {{ 
 background-color: {sidebar_bg}; 
 border-radius: 8px; 
 padding: 4px;
}}
.sidebar * {{ color: {fg}; }}

label {{ color: {fg}; }}

.title-2 {{ 
 color: {accent}; 
 font-size: 18px; 
 font-weight: bold; 
}}

.now-playing {{
 background: {sidebar_bg};
 border-radius: 8px;
 padding: 8px;
}}

.album-art {{
 border-radius: 6px;
}}

.track-title {{
 font-size: 16px;
 font-weight: bold;
 color: {fg};
}}

.track-artist {{
 font-size: 13px;
 color: {dim};
}}

.viz-area {{
 background: transparent;
 border-radius: 8px;
}}

.lyrics-container {{ 
 background: {sidebar_bg};
 border-radius: 8px; 
 padding: 12px 16px;
 border: 1px solid {dim}40;
 min-width: 400px;
 max-width: 400px;
}}

.lyrics-current {{ 
 color: {viz_accent}; 
 font-size: 18px; 
 font-weight: bold;
}}

.lyrics-dim {{ 
 color: {dim}; 
 font-size: 13px; 
 opacity: 0.7;
}}

.card {{ 
 background-color: {sidebar_bg}; 
 border-radius: 8px; 
 border: 1px solid {dim}30;
}}

.controls {{
 padding: 4px;
}}

.control-btn {{
 background: transparent;
 color: {fg};
 border-radius: 50%;
 min-width: 40px;
 min-height: 40px;
 padding: 0;
}}

.control-btn:hover {{
 background: {dim}40;
}}

.control-btn.active {{
 color: {accent};
}}

.play-btn {{
 background: {accent};
 color: {bg};
 border-radius: 50%;
 min-width: 56px;
 min-height: 56px;
 padding: 0;
}}

.play-btn:hover {{
 background: {viz_accent};
}}

.progress-scale {{
 min-height: 8px;
}}

scale trough {{
 background-color: {dim}40;
 border-radius: 4px;
 min-height: 8px;
}}

scale highlight {{
 background: {accent};
 border-radius: 4px;
 min-height: 8px;
}}

scale slider {{
 background: {fg};
 min-width: 16px;
 min-height: 16px;
 border-radius: 50%;
}}

.volume-scale {{
 min-width: 120px;
}}

.volume-icon {{
 background: transparent;
 color: {fg};
}}

.dim-label {{ color: {dim}; }}

.search-entry {{
 background-color: {sidebar_bg};
 color: {fg};
 border-radius: 10px;
 border: 1px solid {dim}40;
 padding: 8px 12px;
 min-height: 36px;
}}

.search-entry:focus {{
 border-color: {accent};
}}

.navigation-sidebar {{ 
 background-color: transparent; 
}}

.navigation-sidebar > row {{ 
 border-radius: 8px; 
 margin: 2px 0; 
 background-color: transparent;
 padding: 4px;
}}

.navigation-sidebar > row:selected {{ 
 background: {accent};
 color: {bg};
}}

.navigation-sidebar > row:selected label {{ 
 color: {bg}; 
}}

.navigation-sidebar > row:hover:not(:selected) {{ 
 background-color: {dim}20;
}}

/* Playing track highlight */
.navigation-sidebar > row.playing {{
 background: {accent}40;
 border-left: 3px solid {accent};
}}

.navigation-sidebar > row.playing label {{
 color: {accent};
}}

scrolledwindow, list {{ 
 background-color: transparent; 
}}

scrollbar {{ 
 background-color: transparent; 
}}

scrollbar slider {{ 
 background-color: {dim}; 
 border-radius: 4px; 
 min-width: 6px;
}}

.pill {{
 background: {bg};
 color: {fg};
 border-radius: 16px;
 padding: 6px 14px;
 border: 1px solid {dim}40;
}}

.pill:hover {{
 background: {dim}30;
}}

.viz-mode-btn {{
 font-weight: bold;
}}

button {{
 background: transparent;
 color: {fg};
 border: none;
}}

menubutton > button {{
 background: {sidebar_bg};
 border-radius: 50%;
 min-width: 36px;
 min-height: 36px;
}}

menubutton > button:hover {{
 background: {dim}40;
}}

separator {{ 
 background-color: {dim}30; 
}}

popover {{
 background: {bg};
 border-radius: 8px;
 border: 1px solid {dim}40;
 padding: 4px;
}}

popover > contents {{
 background: {bg};
 border-radius: 8px;
}}

popover button {{
 background: transparent;
 color: {fg};
 padding: 6px 12px;
 border-radius: 6px;
}}

popover button:hover {{
 background: {accent}30;
}}

popover label {{
 color: {fg};
}}

.status-bar {{
 padding: 4px 8px;
 border-top: 1px solid {dim}20;
}}

.status-item {{
 font-size: 11px;
 color: {dim};
}}

.track-count {{
 font-size: 12px;
}}

.tab-box {{
 background: {sidebar_bg};
 border-radius: 8px;
 padding: 2px;
 margin-bottom: 8px;
}}

.tab-btn {{
 background: transparent;
 color: {dim};
 border-radius: 6px;
 padding: 6px 12px;
 font-weight: 500;
}}

.tab-btn.active {{
 background: {accent};
 color: {bg};
}}

.tab-btn:hover:not(.active) {{
 background: {dim}20;
 color: {fg};
}}

.queue-list {{
 background: transparent;
}}

/* Track header styling */
.track-header {{
 padding: 4px 8px;
 border-bottom: 1px solid {dim}30;
}}

.header-label {{
 font-size: 11px;
 font-weight: bold;
 color: {dim};
}}

/* File tree styling */
.file-tree {{
 background: transparent;
}}

.file-tree treeview {{
 background: transparent;
}}

.file-tree treeview:selected {{
 background: {accent};
 color: {bg};
}}

.file-tree treeview:hover:not(:selected) {{
 background: {dim}20;
}}

/* Context menu buttons */
.context-btn {{
 text-align: left;
}}

.context-btn.destructive {{
 color: #ff4444;
}}

/* Small buttons */
.small-btn {{
 min-width: 24px;
 min-height: 24px;
 padding: 0;
}}
"#);

 let provider = CssProvider::new();
 provider.load_from_data(&css);

 if let Some(display) = Display::default() {
 gtk4::style_context_add_provider_for_display(&display, &provider, 800);
 }
}

fn draw_visualization(state: &Arc<Mutex<AppState>>, _area: &DrawingArea, cr: &cairo::Context, width: i32, height: i32) {
 let state = state.lock().unwrap();
 let data = &state.spectrum_data;
 let peaks = &state.peak_data;
 let mode = state.viz_mode;
 let theme_idx = state.current_theme;
 
 let theme = THEMES.get(theme_idx).unwrap_or(&THEMES[0]);
 let (_, _, _, _, _, _, viz_accent, _, _) = *theme;

 let w = width as f64;
 let h = height as f64;

 let (r, g, b) = parse_hex_color(viz_accent);

 match mode {
 0 => draw_bars(cr, data, peaks, w, h, r, g, b),
 1 => draw_wave(cr, data, w, h, r, g, b),
 2 => draw_circles(cr, data, w, h, r, g, b),
 3 => draw_stars(cr, data, w, h, r, g, b),
 4 => draw_mirror(cr, data, peaks, w, h, r, g, b),
 5 => draw_spectrum(cr, data, peaks, w, h, r, g, b),
 _ => draw_bars(cr, data, peaks, w, h, r, g, b),
 }
}

fn parse_hex_color(hex: &str) -> (f64, f64, f64) {
 let r = i32::from_str_radix(&hex[1..3], 16).unwrap_or(255) as f64 / 255.0;
 let g = i32::from_str_radix(&hex[3..5], 16).unwrap_or(255) as f64 / 255.0;
 let b = i32::from_str_radix(&hex[5..7], 16).unwrap_or(255) as f64 / 255.0;
 (r, g, b)
}

fn draw_bars(cr: &cairo::Context, data: &[f64], peaks: &[f64], w: f64, h: f64, r: f64, g: f64, b: f64) {
 let bar_width = w / data.len() as f64 * 0.75;
 let gap = w / data.len() as f64 * 0.25;
 
 for (i, &val) in data.iter().enumerate() {
 let x = i as f64 * (bar_width + gap);
 let bar_h = val * h * 0.85;

 cr.set_source_rgba(r, g, b, 0.8);
 cr.rectangle(x, h - bar_h, bar_width, bar_h);
 cr.fill();

 if i < peaks.len() {
 let peak_h = peaks[i] * h * 0.85;
 if peak_h > bar_h + 5.0 {
 cr.set_source_rgba(r, g, b, 0.9);
 cr.rectangle(x, h - peak_h - 3.0, bar_width, 3.0);
 cr.fill();
 }
 }
 }
}

fn draw_wave(cr: &cairo::Context, data: &[f64], w: f64, h: f64, r: f64, g: f64, b: f64) {
 cr.move_to(0.0, h / 2.0);
 
 for (i, &val) in data.iter().enumerate() {
 let x = i as f64 * w / data.len() as f64;
 let y = h / 2.0 + val * h * 0.4 * (if i % 2 == 0 { 1.0 } else { -1.0 });
 cr.line_to(x, y);
 }
 
 cr.line_to(w, h / 2.0);

 cr.set_source_rgba(r, g, b, 0.8);
 cr.set_line_width(3.0);
 cr.stroke();
}

fn draw_circles(cr: &cairo::Context, data: &[f64], w: f64, h: f64, r: f64, g: f64, b: f64) {
 let (cx, cy) = (w / 2.0, h / 2.0);
 let max_radius = h.min(w) / 2.0 * 0.85;

 for ring in 0..10 {
 let ring_data = data.get(ring * 6 % data.len()).unwrap_or(&0.5);
 let base_radius = max_radius * (ring as f64 + 1.0) / 10.0;
 let radius = base_radius * (0.6 + ring_data * 0.4);

 let alpha = 0.6 - ring as f64 * 0.05;
 cr.set_source_rgba(r, g, b, alpha);
 cr.set_line_width(2.0 + (10 - ring) as f64 * 0.3);
 cr.arc(cx, cy, radius, 0.0, std::f64::consts::PI * 2.0);
 cr.stroke();
 }

 for (i, &val) in data.iter().enumerate().take(48) {
 let angle = i as f64 * std::f64::consts::PI * 2.0 / 48.0;
 let dist = max_radius * (0.3 + val * 0.7);
 let (x, y) = (cx + dist * angle.cos(), cy + dist * angle.sin());

 cr.set_source_rgba(r, g, b, 0.5 + val * 0.5);
 cr.arc(x, y, 2.0 + val * 4.0, 0.0, std::f64::consts::PI * 2.0);
 cr.fill();
 }
}

fn draw_stars(cr: &cairo::Context, data: &[f64], w: f64, h: f64, r: f64, g: f64, b: f64) {
 let (cx, cy) = (w / 2.0, h / 2.0);
 let max_dist = h.min(w) / 2.0 * 0.9;

 let center_val = data.get(0).unwrap_or(&0.5);

 // Glow layers
 for glow in (0..8).rev() {
 let alpha = 0.08 + 0.04 * glow as f64;
 let size = 20.0 + center_val * 25.0 + glow as f64 * 12.0;
 cr.set_source_rgba(r, g, b, alpha);
 draw_star_shape(cr, cx, cy, size);
 cr.fill();
 }

 cr.set_source_rgba(r, g, b, 1.0);
 draw_star_shape(cr, cx, cy, 15.0 + center_val * 15.0);
 cr.fill();

 for (i, &val) in data.iter().enumerate().take(16).skip(1) {
 let time = std::time::SystemTime::now()
 .duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as f64 * 0.002;
 let base_angle = i as f64 * std::f64::consts::PI * 2.0 / 15.0;
 let angle = base_angle + time;
 let dist = max_dist * (0.4 + val * 0.5);
 let (x, y) = (cx + dist * angle.cos(), cy + dist * angle.sin());
 let size = 6.0 + val * 12.0;

 cr.set_source_rgba(r, g, b, 0.7 + val * 0.3);
 draw_star_shape(cr, x, y, size);
 cr.fill();
 }
}

fn draw_star_shape(cr: &cairo::Context, x: f64, y: f64, size: f64) {
 for i in 0..5 {
 let outer_angle = i as f64 * std::f64::consts::PI * 0.4 - std::f64::consts::FRAC_PI_2;
 let inner_angle = outer_angle + std::f64::consts::PI * 0.2;

 let outer_x = x + size * outer_angle.cos();
 let outer_y = y + size * outer_angle.sin();
 let inner_x = x + size * 0.4 * inner_angle.cos();
 let inner_y = y + size * 0.4 * inner_angle.sin();

 if i == 0 {
 cr.move_to(outer_x, outer_y);
 } else {
 cr.line_to(outer_x, outer_y);
 }
 cr.line_to(inner_x, inner_y);
 }
 cr.close_path();
}

fn draw_mirror(cr: &cairo::Context, data: &[f64], peaks: &[f64], w: f64, h: f64, r: f64, g: f64, b: f64) {
 let bar_width = w / 32.0 * 0.8;
 let gap = w / 32.0 * 0.2;

 for (i, &val) in data.iter().take(32).enumerate() {
 let x = i as f64 * (bar_width + gap);
 let bar_h = val * h * 0.42;

 // Top half
 cr.set_source_rgba(r, g, b, 0.8);
 cr.rectangle(x, h / 2.0 - bar_h, bar_width, bar_h);
 cr.fill();

 // Bottom half (reflection)
 cr.set_source_rgba(r * 0.6, g * 0.6, b * 0.6, 0.5);
 cr.rectangle(x, h / 2.0, bar_width, bar_h * 0.7);
 cr.fill();

 // Peak
 if i < peaks.len() {
 let peak_h = peaks[i] * h * 0.42;
 cr.set_source_rgba(r, g, b, 0.9);
 cr.rectangle(x, h / 2.0 - peak_h - 2.0, bar_width, 2.0);
 cr.fill();
 }
 }
}

fn draw_spectrum(cr: &cairo::Context, data: &[f64], peaks: &[f64], w: f64, h: f64, r: f64, g: f64, b: f64) {
 let bar_width = w / data.len() as f64 * 0.85;
 let gap = w / data.len() as f64 * 0.15;

 for (i, &val) in data.iter().enumerate() {
 let x = i as f64 * (bar_width + gap);
 let bar_h = val * h * 0.9;

 // Color gradient based on frequency
 let ratio = i as f64 / data.len() as f64;
 let (br, bg, bb) = if ratio < 0.33 {
 (r, g * 0.5, b * 0.5)
 } else if ratio < 0.66 {
 (r * 0.7, g, b * 0.7)
 } else {
 (r * 0.5, g * 0.7, b)
 };

 // Segments
 let segments = (val * 20.0) as i32;
 let segment_h = bar_h / segments.max(1) as f64 * 0.9;

 for s in 0..segments {
 let seg_y = h - (s + 1) as f64 * (segment_h + 1.0);
 let alpha = 0.5 + 0.5 * (s as f64 / segments as f64);
 cr.set_source_rgba(br, bg, bb, alpha);
 cr.rectangle(x, seg_y, bar_width, segment_h);
 cr.fill();
 }

 // Peak
 if i < peaks.len() {
 let peak_h = peaks[i] * h * 0.9;
 cr.set_source_rgba(r, g, b, 1.0);
 cr.rectangle(x, h - peak_h - 4.0, bar_width, 3.0);
 cr.fill();
 }
 }
}

fn create_track_row(track: &Track) -> (ListBoxRow, Button) {
 let hbox = Box::builder()
 .orientation(Orientation::Horizontal)
 .spacing(8)
 .margin_top(4)
 .margin_bottom(4)
 .margin_start(8)
 .margin_end(8)
 .build();

 // Left side: title on top, artist below
 let vbox = Box::builder()
 .orientation(Orientation::Vertical)
 .spacing(2)
 .hexpand(true)
 .build();

 let title = Label::builder()
 .label(&track.title)
 .halign(gtk4::Align::Start)
 .ellipsize(gtk4::pango::EllipsizeMode::End)
 .css_classes(vec!["track-row-title".to_string()])
 .build();

 let artist = Label::builder()
 .label(&track.artist)
 .halign(gtk4::Align::Start)
 .ellipsize(gtk4::pango::EllipsizeMode::End)
 .css_classes(vec!["track-row-artist".to_string(), "dim-label".to_string()])
 .build();

 vbox.append(&title);
 vbox.append(&artist);

 // Add to queue button
 let add_btn = Button::builder()
 .icon_name("list-add-symbolic")
 .css_classes(vec!["flat".to_string(), "add-queue-btn".to_string()])
 .tooltip_text("Add to Queue")
 .valign(gtk4::Align::Center)
 .build();

 hbox.append(&vbox);
 hbox.append(&add_btn);

 let row = ListBoxRow::builder()
 .child(&hbox)
 .css_classes(vec!["track-row".to_string()])
 .build();

 (row, add_btn)
}

fn scan_music_dir() -> Vec<Track> {
 let music_path = glib::home_dir().join(MUSIC_DIR);
 let mut tracks = Vec::new();

 for entry in WalkDir::new(&music_path).into_iter().filter_map(|e| e.ok()) {
 let path = entry.path();
 let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");

 if ["mp3", "flac", "ogg", "wav", "m4a", "aac", "webm"].contains(&ext) {
 let filename = path.file_stem().and_then(|n| n.to_str()).unwrap_or("Unknown");
 let (artist, title) = if filename.contains(" - ") {
 let parts: Vec<&str> = filename.splitn(2, " - ").collect();
 (parts[0].to_string(), parts[1].to_string())
 } else {
 ("Unknown".to_string(), filename.to_string())
 };

 tracks.push(Track {
 path: path.display().to_string(),
 title,
 artist,
 duration_secs: None,
 });
 }
 }

 tracks.sort_by(|a, b| {
 a.artist.to_lowercase().cmp(&b.artist.to_lowercase())
 .then(a.title.to_lowercase().cmp(&b.title.to_lowercase()))
 });
 tracks
}

async fn fetch_lyrics(artist: &str, title: &str) -> Result<Vec<LrcLine>> {
 let url = format!(
 "https://lrclib.net/api/search?artist_name={}&track_name={}",
 urlencoding::encode(artist),
 urlencoding::encode(title)
 );

 let client = reqwest::Client::builder()
 .timeout(std::time::Duration::from_secs(10))
 .build()?;
 
 let response = client.get(&url).send().await?;

 if !response.status().is_success() {
 anyhow::bail!("HTTP {}", response.status());
 }
 
 let results: Vec<serde_json::Value> = response.json().await?;

 if let Some(first) = results.first() {
 if let Some(synced) = first.get("syncedLyrics").and_then(|s| s.as_str()) {
 if !synced.is_empty() {
 return Ok(parse_lrc(synced));
 }
 }
 if let Some(plain) = first.get("plainLyrics").and_then(|s| s.as_str()) {
 if !plain.is_empty() {
 let lines: Vec<&str> = plain.lines().filter(|l| !l.trim().is_empty()).collect();
 let mut result = Vec::new();
 for (i, line) in lines.iter().enumerate() {
 result.push(LrcLine {
 time_ms: i as f64 * 5000.0,
 text: line.to_string()
 });
 }
 return Ok(result);
 }
 }
 }

 Ok(Vec::new())
}

fn parse_lrc(lrc: &str) -> Vec<LrcLine> {
 let mut lines = Vec::new();
 for line in lrc.lines() {
 if let Some((time_ms, text)) = parse_lrc_line(line) {
 lines.push(LrcLine { time_ms, text });
 }
 }
 lines.sort_by(|a, b| a.time_ms.partial_cmp(&b.time_ms).unwrap());
 lines
}

fn parse_lrc_line(line: &str) -> Option<(f64, String)> {
 let re = regex::Regex::new(r"\[(\d+):(\d+\.?\d*)\](.+)").ok()?;
 let caps = re.captures(line)?;
 
 let mins: f64 = caps[1].parse().ok()?;
 let secs: f64 = caps[2].parse().ok()?;
 let text = caps[3].trim().to_string();
 
 if text.is_empty() { return None; }
 Some((mins * 60.0 * 1000.0 + secs * 1000.0, text))
}

#[derive(serde::Serialize, serde::Deserialize)]
struct PlaylistEntry {
 path: String,
 title: String,
 artist: String,
}

fn save_playlist(tracks: &[Track], name: &str) -> std::io::Result<()> {
 let playlist_dir = glib::home_dir().join(".config/volta-wave/playlists");
 std::fs::create_dir_all(&playlist_dir)?;
 
 let entries: Vec<PlaylistEntry> = tracks.iter().map(|t| PlaylistEntry {
 path: t.path.clone(),
 title: t.title.clone(),
 artist: t.artist.clone(),
 }).collect();
 
 let json = serde_json::to_string_pretty(&entries)?;
 let path = playlist_dir.join(format!("{}.json", name));
 std::fs::write(path, json)
}

fn load_playlist(name: &str) -> std::io::Result<Vec<Track>> {
 let path = glib::home_dir().join(".config/volta-wave/playlists").join(format!("{}.json", name));
 let content = std::fs::read_to_string(path)?;
 let entries: Vec<PlaylistEntry> = serde_json::from_str(&content)?;
 
 Ok(entries.into_iter().map(|e| Track {
 path: e.path,
 title: e.title,
 artist: e.artist,
 duration_secs: None,
 }).collect())
}

fn list_playlists() -> Vec<String> {
 let playlist_dir = glib::home_dir().join(".config/volta-wave/playlists");
 if !playlist_dir.exists() {
 return Vec::new();
 }
 
 std::fs::read_dir(&playlist_dir)
 .map(|entries| {
 entries.filter_map(|e| e.ok())
 .filter_map(|e| {
 let path = e.path();
 if path.extension()?.to_str()? == "json" {
 path.file_stem()?.to_str().map(|s| s.to_string())
 } else {
 None
 }
 })
 .collect()
 })
 .unwrap_or_default()
}
