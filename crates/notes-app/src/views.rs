//! Shared native workspace construction and drawing helpers.
use super::*;
#[derive(Clone, Copy, Debug)]
pub(super) struct Camera {
    pub zoom: f64,
    pub x: f64,
    pub y: f64,
}
impl Default for Camera {
    fn default() -> Self {
        Self {
            zoom: 1.0,
            x: 0.0,
            y: 0.0,
        }
    }
}
impl Camera {
    pub fn world(self, area: &gtk::DrawingArea, x: f64, y: f64) -> (f64, f64) {
        (
            (x - area.width() as f64 / 2.0 - self.x) / self.zoom,
            (y - area.height() as f64 / 2.0 - self.y) / self.zoom,
        )
    }
    pub fn transform(self, cr: &gtk::cairo::Context, w: i32, h: i32) {
        cr.translate(w as f64 / 2.0 + self.x, h as f64 / 2.0 + self.y);
        cr.scale(self.zoom, self.zoom);
    }
}
pub(super) fn color(cr: &gtk::cairo::Context, value: &str, alpha: f64) {
    if let Ok(c) = gdk::RGBA::parse(value) {
        cr.set_source_rgba(c.red() as f64, c.green() as f64, c.blue() as f64, alpha);
    }
}
pub(super) fn text(
    cr: &gtk::cairo::Context,
    value: &str,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    size: f64,
) {
    let layout = pangocairo::functions::create_layout(cr);
    layout.set_text(value);
    layout.set_width((width * gtk::pango::SCALE as f64) as i32);
    layout.set_height((height * gtk::pango::SCALE as f64) as i32);
    layout.set_wrap(gtk::pango::WrapMode::WordChar);
    layout.set_ellipsize(gtk::pango::EllipsizeMode::End);
    let mut font = gtk::pango::FontDescription::from_string("JetBrains Mono");
    font.set_absolute_size(size * gtk::pango::SCALE as f64);
    layout.set_font_description(Some(&font));
    cr.move_to(x, y);
    pangocairo::functions::show_layout(cr, &layout);
}
impl Ui {
    pub(super) fn workspace_window(
        self: &Rc<Self>,
        title: &str,
    ) -> (gtk::Window, gtk::Box, gtk::Box) {
        let window = gtk::Window::builder()
            .title(title)
            .transient_for(&self.window)
            .default_width(1050)
            .default_height(760)
            .build();
        if let Some(app) = self.window.application() {
            window.set_application(Some(&app));
        }
        window.add_css_class("notes-window");
        window.set_titlebar(Some(&widgets::hbox(0)));
        let root = widgets::vbox(0);
        let toolbar = widgets::hbox(6);
        toolbar.add_css_class("editor-bar");
        let heading = gtk::Label::new(None);
        heading.set_markup(&format!("<b>{}</b>", glib::markup_escape_text(title)));
        heading.add_css_class("crumb");
        heading.set_hexpand(true);
        heading.set_xalign(0.0);
        toolbar.append(&heading);
        root.append(&toolbar);
        // A tab, not a window: the window stays (hidden) for its dialogs and close
        // handlers; closing the tab closes it.
        let icon = match title {
            "Knowledge graph" => "nyx-notes-graph-symbolic",
            "Collections" => "nyx-notes-kanban-symbolic",
            "Writing projects" => "nyx-notes-book-symbolic",
            t if t.ends_with(".canvas") || t.starts_with("Canvas") => "nyx-notes-canvas-symbolic",
            _ => "nyx-notes-file-symbolic",
        };
        let tab = match title {
            "Knowledge graph" => "Graph",
            t => t,
        };
        let w = window.downgrade();
        self.view_tab(
            tab,
            icon,
            root.upcast_ref(),
            title == "Knowledge graph",
            Rc::new(move || {
                if let Some(w) = w.upgrade()
                    && !w.emit_by_name::<bool>("close-request", &[])
                {
                    w.destroy();
                }
            }),
        );
        self.shell.view_windows.borrow_mut().push((window.downgrade(), root.clone().upcast()));
        let (weak, page) = (Rc::downgrade(self), root.downgrade());
        window.connect_destroy(move |_| {
            if let (Some(ui), Some(page)) = (weak.upgrade(), page.upgrade()) {
                ui.close_view(page.upcast_ref());
            }
        });
        (window, root, toolbar)
    }
}
pub(super) fn pan_zoom(area: &gtk::DrawingArea, camera: Rc<Cell<Camera>>) {
    let drag = gtk::GestureDrag::new();
    drag.set_button(2);
    let origin = Rc::new(Cell::new(Camera::default()));
    let c = camera.clone();
    let o = origin.clone();
    drag.connect_drag_begin(move |_, _, _| o.set(c.get()));
    let c = camera.clone();
    let weak = area.downgrade();
    drag.connect_drag_update(move |_, x, y| {
        let initial = origin.get();
        c.set(Camera {
            x: initial.x + x,
            y: initial.y + y,
            ..initial
        });
        if let Some(area) = weak.upgrade() {
            area.queue_draw();
        }
    });
    area.add_controller(drag);
    let scroll = gtk::EventControllerScroll::new(gtk::EventControllerScrollFlags::VERTICAL);
    let weak = area.downgrade();
    scroll.connect_scroll(move |_, _, dy| {
        let mut c = camera.get();
        c.zoom = (c.zoom * (-dy * 0.15).exp()).clamp(0.05, 8.0);
        camera.set(c);
        if let Some(a) = weak.upgrade() {
            a.queue_draw();
        }
        glib::Propagation::Stop
    });
    area.add_controller(scroll);
}
