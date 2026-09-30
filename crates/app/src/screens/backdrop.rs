//! The menus' backdrops: a picture behind the start menu and an animated
//! one behind the game setup.
//!
//! The pictures are PNGs built into the binary (`backgrounds/`, made by
//! its `prepare.sh` from `Background/` at the root). They are decoded on a
//! thread of their own the first time a menu is up, so the window never
//! waits for them, and go up to the GPU as Bevy images egui is handed by
//! id — kept only on the GPU, and let go again once the menus are left
//! for the game.
//!
//! A picture is 3:2 and a window anything: it is drawn to **cover** the
//! window, scaled until neither side falls short and the overhang cut off
//! evenly, so it fills any window at any UI scale without being stretched.

use std::sync::Mutex;
use std::sync::mpsc::{Receiver, TryRecvError, channel};

use bevy::asset::RenderAssetUsages;
use bevy::image::{CompressedImageFormats, ImageSampler, ImageType};
use bevy::prelude::*;
use bevy_egui::{EguiTextureHandle, EguiUserTextures, egui};

use crate::Screen;

/// The start menu's picture.
const START: &[u8] = include_bytes!("../../backgrounds/start.png");

/// The game setup's pictures: the gif's distinct frames, in the order
/// `prepare.sh` writes them.
const SETUP: [&[u8]; 9] = [
    include_bytes!("../../backgrounds/setup_0.png"),
    include_bytes!("../../backgrounds/setup_1.png"),
    include_bytes!("../../backgrounds/setup_2.png"),
    include_bytes!("../../backgrounds/setup_3.png"),
    include_bytes!("../../backgrounds/setup_4.png"),
    include_bytes!("../../backgrounds/setup_5.png"),
    include_bytes!("../../backgrounds/setup_6.png"),
    include_bytes!("../../backgrounds/setup_7.png"),
    include_bytes!("../../backgrounds/setup_8.png"),
];

/// The game setup's animation as the gif plays it: which of [`SETUP`],
/// for how many hundredths of a second. It goes there and back twice, so
/// sixteen frames out of nine pictures.
const SETUP_FRAMES: [(usize, u32); 16] = [
    (0, 17),
    (1, 17),
    (2, 17),
    (3, 17),
    (4, 34),
    (3, 17),
    (2, 17),
    (1, 17),
    (0, 17),
    (5, 17),
    (6, 17),
    (7, 17),
    (8, 34),
    (7, 17),
    (6, 17),
    (5, 17),
];

/// Which picture is behind a screen.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Backdrop {
    Start,
    Setup,
}

/// The pictures, as far as they have got: none, decoding, or on the GPU.
/// Slot 0 is [`START`], the rest [`SETUP`] in order.
#[derive(Resource, Default)]
pub struct Backdrops {
    decoding: Option<Mutex<Receiver<(usize, Image)>>>,
    shown: Vec<Option<Shown>>,
}

struct Shown {
    image: Handle<Image>,
    texture: egui::TextureId,
    size: egui::Vec2,
}

pub struct BackdropPlugin;

impl Plugin for BackdropPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Backdrops>().add_systems(Update, keep);
    }
}

/// Whether the screen is one of the menus', which keep the pictures up:
/// the lobby too, since it lies between the menu and the menu again.
fn on_menus(screen: &Screen) -> bool {
    matches!(screen, Screen::Menu | Screen::Setup | Screen::Lobby)
}

/// Starts the decoding when a menu is first up, puts each picture on the
/// GPU as it comes, and lets them all go once the menus are left.
fn keep(
    mut backdrops: ResMut<Backdrops>,
    state: Res<State<Screen>>,
    mut images: ResMut<Assets<Image>>,
    mut textures: ResMut<EguiUserTextures>,
) {
    let wanted = on_menus(state.get());
    let backdrops = &mut *backdrops;
    if wanted && backdrops.decoding.is_none() && backdrops.shown.is_empty() {
        let (send, receive) = channel();
        std::thread::spawn(move || {
            for (i, bytes) in std::iter::once(START).chain(SETUP).enumerate() {
                if let Some(image) = decode(bytes)
                    && send.send((i, image)).is_err()
                {
                    return;
                }
            }
        });
        backdrops.decoding = Some(Mutex::new(receive));
        backdrops.shown = (0..=SETUP.len()).map(|_| None).collect();
    }
    if let Some(receive) = &backdrops.decoding {
        let receive = receive.lock().unwrap_or_else(|e| e.into_inner());
        loop {
            match receive.try_recv() {
                Ok((i, image)) => {
                    let size = image.size();
                    let image = images.add(image);
                    let texture = textures.add_image(EguiTextureHandle::Strong(image.clone()));
                    backdrops.shown[i] = Some(Shown {
                        image,
                        texture,
                        size: egui::vec2(size.x as f32, size.y as f32),
                    });
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    drop(receive);
                    backdrops.decoding = None;
                    break;
                }
            }
        }
    }
    if !wanted && !backdrops.shown.is_empty() {
        // Dropping the receiver ends a decoding still going at its next
        // picture; the handles going frees the GPU's copies.
        backdrops.decoding = None;
        for shown in backdrops.shown.drain(..).flatten() {
            textures.remove_image(&shown.image);
            images.remove(&shown.image);
        }
    }
}

/// A PNG as an image the GPU alone keeps.
fn decode(bytes: &[u8]) -> Option<Image> {
    Image::from_buffer(
        bytes,
        ImageType::Extension("png"),
        CompressedImageFormats::NONE,
        true,
        ImageSampler::linear(),
        RenderAssetUsages::RENDER_WORLD,
    )
    .ok()
}

/// Paints `which` over the whole window, behind everything else egui
/// draws this frame — to be called before the screen's panels, which
/// must leave their own fill off to let it show. Until the picture has
/// decoded it is the page's deepest panel colour.
pub fn paint(ctx: &egui::Context, backdrops: &Backdrops, which: Backdrop) {
    let screen = ctx.viewport_rect();
    let painter = ctx.layer_painter(egui::LayerId::background());
    let slot = match which {
        Backdrop::Start => 0,
        Backdrop::Setup => {
            let now = ctx.input(|i| i.time);
            1 + setup_frame(now)
        }
    };
    let Some(Some(shown)) = backdrops.shown.get(slot) else {
        painter.rect_filled(screen, 0.0, crate::theme::PANEL_DEEP);
        return;
    };
    painter.image(
        shown.texture,
        screen,
        cover(screen.size(), shown.size),
        egui::Color32::WHITE,
    );
    if which == Backdrop::Setup {
        // It moves: the next frame is due within a sixth of a second.
        ctx.request_repaint();
    }
}

/// Which of [`SETUP`] is showing `seconds` into the animation.
fn setup_frame(seconds: f64) -> usize {
    let whole: u32 = SETUP_FRAMES.iter().map(|&(_, d)| d).sum();
    let mut at = ((seconds * 100.0).max(0.0) as u64 % whole as u64) as u32;
    for &(picture, delay) in &SETUP_FRAMES {
        if at < delay {
            return picture;
        }
        at -= delay;
    }
    SETUP_FRAMES[0].0
}

/// The part of a picture `picture` big that covers a window `window` big
/// at one scale, in the picture's 0..1 coordinates: all of its narrower
/// way, and the middle of the other.
fn cover(window: egui::Vec2, picture: egui::Vec2) -> egui::Rect {
    if window.x <= 0.0 || window.y <= 0.0 || picture.x <= 0.0 || picture.y <= 0.0 {
        return egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0));
    }
    let scale = (window.x / picture.x).max(window.y / picture.y);
    let seen = egui::vec2(
        (window.x / (picture.x * scale)).min(1.0),
        (window.y / (picture.y * scale)).min(1.0),
    );
    let min = egui::pos2((1.0 - seen.x) / 2.0, (1.0 - seen.y) / 2.0);
    egui::Rect::from_min_size(min, seen)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A wide window sees the picture's whole width and the middle of its
    /// height; a tall one the other way round; one its own shape all of it.
    #[test]
    fn the_picture_covers_the_window_unstretched() {
        let picture = egui::vec2(1536.0, 1024.0);
        for window in [
            egui::vec2(1920.0, 1080.0),
            egui::vec2(800.0, 1200.0),
            egui::vec2(1536.0, 1024.0),
            egui::vec2(1280.0, 720.0) / 1.5,
        ] {
            let uv = cover(window, picture);
            assert!(uv.min.x >= 0.0 && uv.min.y >= 0.0);
            assert!(uv.max.x <= 1.0 + 1e-6 && uv.max.y <= 1.0 + 1e-6);
            assert!((uv.center().x - 0.5).abs() < 1e-6 && (uv.center().y - 0.5).abs() < 1e-6);
            assert!(uv.width() > 0.999 || uv.height() > 0.999);
            // The same scale both ways: the shown part has the window's
            // shape in picture pixels.
            let shown = egui::vec2(uv.width() * picture.x, uv.height() * picture.y);
            assert!((shown.x / shown.y - window.x / window.y).abs() < 1e-3);
        }
    }

    /// The animation walks the gif's order and loops.
    #[test]
    fn the_setup_animation_plays_the_gifs_order() {
        assert_eq!(setup_frame(0.0), 0);
        assert_eq!(setup_frame(0.17), 1);
        assert_eq!(setup_frame(0.70), 4);
        assert_eq!(setup_frame(0.95), 4);
        assert_eq!(setup_frame(1.03), 3);
        assert_eq!(setup_frame(3.06), 0);
        assert!(SETUP_FRAMES.iter().all(|&(p, _)| p < SETUP.len()));
    }
}
