//! The fixtures the room only draws: the galley (worktop, hob, cold store,
//! dishwasher), the table and its chairs, the bunks, the broom locker, the
//! hydroponic bays and the fields, and the heads (the pan and the basin).
//!
//! Nothing uses them. They stay in a ship's part list as furniture, so the
//! room keeps where each one stands — every frame is a solid a body walks
//! round (`Room::solids`), and a bunk is where a crew member lands when it
//! comes aboard off the deck (`Game::adopt`) — and draws each one as it
//! stands in a fresh room: the drawer shut, the hob cold with an empty pot
//! on it, the cold store shut, the dishwasher empty and still, the broom in
//! its locker, the bunks made, the trays planted with nothing, the cistern
//! full and the tap off. The designer draws the same pictures through
//! `Room::draw_fixtures`, off the same types, so a part looks the same in
//! the yard as on the deck.

use crate::draw::{Color, DrawList};
use crate::math::{Rect, Vec2, lerp, vec2};
use crate::room::{GLOW, GLOW_DIM, PANEL, PANEL_EDGE, PANEL_LIT, STEEL};

const CAB: Color = PANEL;
const WORKTOP: Color = Color::rgb(0.52, 0.57, 0.63);
const WORKTOP_EDGE: Color = Color::rgb(0.33, 0.38, 0.44);
const DRAWER_FACE: Color = PANEL_LIT;
const HANDLE: Color = Color::rgb(0.66, 0.72, 0.78);
const BOARD: Color = Color::rgb(0.22, 0.26, 0.31);

const FRIDGE: Color = Color::rgb(0.56, 0.63, 0.70);
const FRIDGE_DOOR: Color = Color::rgb(0.69, 0.76, 0.82);
const FRIDGE_IN: Color = Color::rgb(0.09, 0.17, 0.21);

const STOVE_TOP: Color = Color::rgb(0.09, 0.11, 0.13);
const BURNER: Color = Color::rgb(0.16, 0.19, 0.22);
const KNOB: Color = Color::rgb(0.48, 0.54, 0.60);
const POT: Color = Color::rgb(0.38, 0.43, 0.48);
const POT_RIM: Color = Color::rgb(0.56, 0.63, 0.69);
/// How big the pot is drawn against the hob it stands on.
const POT_SIZE: f32 = 0.85;
/// The burner's radius on a double-width run, which the hob's scale is
/// measured against.
const BURNER_R: f32 = 46.0;

const TABLE: Color = PANEL_LIT;
const TABLE_EDGE: Color = PANEL;
const CHAIR: Color = Color::rgb(0.24, 0.28, 0.33);
/// The chair, which is drawn from a centre and a size rather than kept as
/// a rect. A chair fits inside one tile, backrest included.
pub const CHAIR_SIZE: Vec2 = vec2(48.0, 42.0);

const BED_FRAME: Color = PANEL;
const BED_BOARD: Color = PANEL_EDGE;
const BED_BOARD_EDGE: Color = Color::rgb(0.46, 0.53, 0.60);
const BED_SHADOW: Color = Color::rgba(0.0, 0.0, 0.0, 0.34);
const MATTRESS: Color = Color::rgb(0.72, 0.75, 0.79);
const MATTRESS_SEAM: Color = Color::rgb(0.40, 0.44, 0.49);
const PILLOW: Color = Color::rgb(0.88, 0.91, 0.94);
const BLANKET: Color = Color::rgb(0.19, 0.33, 0.47);
const BLANKET_FOLD: Color = Color::rgb(0.31, 0.51, 0.66);

const BROOM_POLE: Color = Color::rgb(0.55, 0.44, 0.31);
const BROOM_HEAD: Color = Color::rgb(0.72, 0.68, 0.58);

/// A field's ground: turned earth, warmer than a bay's trays so a strip
/// reads as a strip and not a bay with its lights out, and darker again
/// at the edge and along the furrows.
const EARTH: Color = Color::rgb(0.36, 0.27, 0.18);
const EARTH_EDGE: Color = Color::rgb(0.24, 0.17, 0.11);
const FURROW: Color = Color::rgb(0.27, 0.20, 0.13);

const BOWL: Color = Color::rgb(0.80, 0.84, 0.87);
const BOWL_RIM: Color = Color::rgb(0.62, 0.68, 0.72);
const WATER: Color = Color::rgb(0.24, 0.52, 0.66);
const BASIN: Color = Color::rgb(0.74, 0.79, 0.82);

/// Plate pips on a dishwasher's door: a slot each for what it holds.
const DISHWASHER_SLOTS: u32 = 10;

/// Trays in a bay or a field.
pub const TRAYS: usize = 6;

// --- the galley --------------------------------------------------------------

/// A run of worktop with a chopping board and a drawer on it.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Worktop {
    pub frame: Rect,
    pub board: Rect,
    pub drawer: Rect,
}

impl Worktop {
    pub fn new(frame: Rect, board: Rect, drawer: Rect) -> Worktop {
        Worktop {
            frame,
            board,
            drawer,
        }
    }

    /// The run, then the drawer's face and the board on top of it.
    pub fn draw(&self, list: &mut DrawList) {
        draw_worktop_run(list, self.frame);
        let d = self.drawer;
        list.rect(d.center(), d.size() + vec2(0.0, 4.0), 0.0, 3.0, DRAWER_FACE);
        list.rect(d.center(), vec2(d.width() * 0.5, 4.0), 0.0, 2.0, HANDLE);
        list.rect(
            d.center(),
            vec2(d.width() * 0.5 - 8.0, 1.5),
            0.0,
            1.0,
            GLOW.alpha(0.7),
        );
        let b = self.board;
        list.rect(b.center(), b.size(), 0.0, 4.0, BOARD);
        list.stroke_rect(b.center(), b.size(), 0.0, 4.0, 1.5, GLOW.alpha(0.35));
    }
}

/// A run of worktop alone: the cabinet, the lip, the top and the lit
/// fascia. A function of the rect and nothing else.
fn draw_worktop_run(list: &mut DrawList, c: Rect) {
    list.rect(c.center(), c.size(), 0.0, 3.0, CAB);
    // A worktop lip along the front edge reads as thickness from above.
    list.rect(
        vec2(c.center().x, c.max.y - 7.0),
        vec2(c.width(), 14.0),
        0.0,
        3.0,
        WORKTOP_EDGE,
    );
    list.rect(
        vec2(c.center().x, c.min.y + (c.height() - 14.0) * 0.5),
        vec2(c.width() - 6.0, c.height() - 14.0),
        0.0,
        2.0,
        WORKTOP,
    );
    // A light strip along the whole fascia, the way every powered surface
    // on the ship is edged.
    list.rect(
        vec2(c.center().x, c.max.y - 2.0),
        vec2(c.width() - 12.0, 3.0),
        0.0,
        1.5,
        GLOW_DIM,
    );
    // Service panels along the front, with a status lamp on each.
    let panels = 6;
    for i in 0..panels {
        let x = lerp(
            c.min.x + 34.0,
            c.max.x - 34.0,
            i as f32 / (panels - 1) as f32,
        );
        list.rect(vec2(x, c.max.y - 9.0), vec2(1.5, 9.0), 0.0, 0.0, PANEL_EDGE);
        list.circle(vec2(x, c.max.y - 11.0), 3.0, GLOW.alpha(0.55));
    }
}

/// A hob with its pot.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Hob {
    pub frame: Rect,
}

impl Hob {
    pub fn new(frame: Rect) -> Hob {
        Hob { frame }
    }

    /// The glass, the burner and its coils, the control, and the empty pot
    /// standing on it.
    pub fn draw(&self, list: &mut DrawList) {
        let s = self.frame;
        list.rect(s.center(), s.size() - vec2(0.0, 14.0), 0.0, 4.0, STOVE_TOP);
        let at = burner_of(s);
        let k = hob_scale_of(s);
        list.circle(at, BURNER_R * k, BURNER);
        // Induction coils, etched into the glass and lit low, so a cold hob
        // still looks like equipment.
        for ring in 0..3 {
            list.ring(at, (18.0 + ring as f32 * 11.0) * k, 1.5, GLOW.alpha(0.16));
        }
        list.ring(at, 38.0 * k, 2.0, STOVE_TOP);
        list.ring(at, 42.0 * k, 1.5, GLOW.alpha(0.30));

        // The touch control: a lit pip at the off end of its track.
        let knob = knob_of(s);
        list.rect(knob, vec2(34.0, 12.0), 0.0, 6.0, STOVE_TOP);
        list.stroke_rect(knob, vec2(34.0, 12.0), 0.0, 6.0, 1.0, PANEL_EDGE);
        let pip = knob + vec2(-9.0, 0.0);
        list.circle(pip, 15.0, GLOW.alpha(0.20));
        list.circle(pip, 8.0, GLOW);
        list.circle(pip, 3.0, KNOB.alpha(0.5));

        // The pot: a shade smaller than the hob it stands on, and empty.
        let at = burner_of(s);
        let k = POT_SIZE * hob_scale_of(s);
        list.circle(at, 50.0 * k, POT);
        list.ring(at, 46.0 * k, 4.0 * k, POT_RIM);
        let handle = vec2(14.0 * k, 7.0 * k);
        list.rect(at + vec2(-30.0 * k, 0.0), handle, 0.0, 3.0 * k, POT_RIM);
        list.rect(at + vec2(30.0 * k, 0.0), handle, 0.0, 3.0 * k, POT_RIM);
        list.circle(at, 40.0 * k, Color::rgb(0.10, 0.11, 0.12));
    }
}

/// The burner's middle: centred, since a ship's hob is one tile — except
/// on a double-width run, where it stands where the left of a pair would.
fn burner_of(stove: Rect) -> Vec2 {
    let c = stove.center();
    if stove.width() > 1.5 * stove.height() {
        c + vec2(-30.0, 0.0)
    } else {
        c
    }
}

/// How big the hob and the pot are drawn: 1 on a double-width run, and on
/// a one-tile hob whatever fits the burner inside the tile.
fn hob_scale_of(stove: Rect) -> f32 {
    if stove.width() > 1.5 * stove.height() {
        1.0
    } else {
        (stove.width().min(stove.height()) / (2.0 * BURNER_R)).min(1.0)
    }
}

/// The control knob, towards the near-left of the hob.
fn knob_of(stove: Rect) -> Vec2 {
    vec2(stove.min.x + 20.0, stove.max.y - 7.0)
}

/// A cold store, shut.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Fridge {
    pub frame: Rect,
}

impl Fridge {
    pub fn new(frame: Rect) -> Fridge {
        Fridge { frame }
    }

    /// The cabinet, its frosted panel and status bar, and the door shut
    /// across its front.
    pub fn draw(&self, list: &mut DrawList) {
        let f = self.frame;
        list.rect(f.center(), f.size(), 0.0, 4.0, FRIDGE);
        list.rect(
            f.center() + vec2(4.0, 0.0),
            f.size() - vec2(22.0, 20.0),
            0.0,
            3.0,
            FRIDGE_IN.alpha(0.35),
        );
        list.rect(
            vec2(f.min.x + 7.0, f.center().y),
            vec2(3.5, f.height() - 22.0),
            0.0,
            1.5,
            GLOW.alpha(0.45),
        );
        // The door is hinged at the front-left corner, shut along the front.
        let hinge = vec2(f.min.x, f.max.y);
        let dir = vec2(1.0, 0.0);
        let len = f.width();
        list.rect(
            hinge + dir * (len * 0.5),
            vec2(len, 9.0),
            0.0,
            3.0,
            FRIDGE_DOOR,
        );
        // Handle at the free end of the door.
        list.circle(hinge + dir * (len - 9.0), 8.0, HANDLE);
    }
}

/// A dishwasher: its door face, and the whole appliance where it stands
/// on its own tile.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Dishwasher {
    pub face: Rect,
    pub body: Option<Rect>,
}

impl Dishwasher {
    /// A dishwasher with its door face wherever a layout puts it.
    pub fn at(face: Rect) -> Dishwasher {
        Dishwasher { face, body: None }
    }

    /// The appliance, the door shut with its handle bar, and a dark pip
    /// for every empty slot.
    pub fn draw(&self, list: &mut DrawList) {
        let f = self.face;
        if let Some(b) = self.body {
            list.rect(b.center(), b.size(), 0.0, 3.0, PANEL);
            list.stroke_rect(b.center(), b.size(), 0.0, 3.0, 1.0, PANEL_EDGE.alpha(0.6));
        }
        let door = f.center();
        list.rect(door, f.size() + vec2(0.0, 4.0), 0.0, 3.0, PANEL_LIT);
        list.stroke_rect(door, f.size() + vec2(0.0, 4.0), 0.0, 3.0, 1.0, PANEL_EDGE);
        // Handle bar across the top of it.
        list.rect(
            door + vec2(0.0, -5.0),
            vec2(f.width() - 16.0, 3.0),
            0.0,
            1.5,
            STEEL,
        );
        for i in 0..DISHWASHER_SLOTS {
            let x = lerp(
                door.x - f.width() * 0.5 + 7.0,
                door.x + f.width() * 0.5 - 7.0,
                i as f32 / (DISHWASHER_SLOTS - 1) as f32,
            );
            list.circle(vec2(x, door.y + 4.5), 4.5, PANEL_EDGE.alpha(0.55));
        }
    }
}

/// The galley's three faces, off the worktop and the dishwasher's tile:
/// the board on the worktop and the drawer a face on its front, cut down
/// to what the worktop has room for; and the dishwasher's door, a face on
/// the front of its tile.
pub fn galley_faces(counter: Rect, dishwasher: Rect) -> (Rect, Rect, Rect) {
    let board = Rect::from_min_size(
        vec2(counter.min.x + 8.0, counter.min.y + 12.0),
        vec2(
            (counter.width() - 16.0).min(96.0).max(20.0),
            30.0f32.min(counter.height() - 20.0).max(10.0),
        ),
    );
    let drawer = Rect::from_min_size(
        vec2(counter.min.x + 8.0, counter.max.y - 18.0),
        vec2((counter.width() - 16.0).min(112.0).max(20.0), 16.0),
    );
    let dish_face = Rect::from_min_size(
        vec2(dishwasher.min.x + 4.0, dishwasher.max.y - 20.0),
        vec2((dishwasher.width() - 8.0).max(20.0), 18.0),
    );
    (board, drawer, dish_face)
}

// --- the table and the chairs ------------------------------------------------

/// The chairs round a table, each with its backrest on the side away from
/// the table.
pub fn draw_chairs(list: &mut DrawList, chairs: &[Vec2], table: Rect) {
    for &ch in chairs {
        let back = if ch.y < table.center().y { -1.0 } else { 1.0 };
        let spine = ch + vec2(0.0, back * 21.0);
        list.rect(ch, CHAIR_SIZE, 0.0, 8.0, CHAIR);
        list.stroke_rect(ch, vec2(40.0, 34.0), 0.0, 6.0, 1.5, GLOW.alpha(0.28));
        list.rect(spine, vec2(50.0, 8.0), 0.0, 4.0, TABLE_EDGE);
        list.rect(spine, vec2(32.0, 2.0), 0.0, 1.0, GLOW.alpha(0.5));
    }
}

/// A table top. A function of the rect alone, so a second table is drawn
/// the same way.
pub fn draw_table_top(list: &mut DrawList, t: Rect) {
    list.rect(t.center(), t.size(), 0.0, 14.0, TABLE_EDGE);
    list.rect(t.center(), t.size() - vec2(9.0, 9.0), 0.0, 11.0, TABLE);
    // An inlaid light around the top, and a seam across the middle where
    // the two halves of it fold.
    list.stroke_rect(
        t.center(),
        t.size() - vec2(24.0, 24.0),
        0.0,
        8.0,
        1.5,
        GLOW.alpha(0.40),
    );
    list.line(
        vec2(t.center().x, t.min.y + 12.0),
        vec2(t.center().x, t.max.y - 12.0),
        1.0,
        PANEL_EDGE.alpha(0.5),
    );
}

// --- the bunks ---------------------------------------------------------------

/// One bunk, and which side of it the deck is on: `side` is +1 with the
/// deck to the bunk's right and -1 to its left. A bunk lying along `x` is
/// the same bunk on its side, worked out in its own frame, `across` and
/// `along` from the head end.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Berth {
    pub frame: Rect,
    side: f32,
    lying: bool,
}

impl Berth {
    pub fn new(frame: Rect, side: f32) -> Berth {
        Berth {
            frame,
            side,
            lying: frame.width() > frame.height(),
        }
    }

    /// The bunk's own frame to the room's: `across` from its centre line,
    /// `along` from its head end.
    fn at(&self, across: f32, along: f32) -> Vec2 {
        if self.lying {
            vec2(self.frame.min.x + along, self.frame.center().y + across)
        } else {
            vec2(self.frame.center().x + across, self.frame.min.y + along)
        }
    }

    /// A size given across and along, as the room's width and height.
    fn size(&self, across: f32, along: f32) -> Vec2 {
        if self.lying {
            vec2(along, across)
        } else {
            vec2(across, along)
        }
    }

    fn breadth(&self) -> f32 {
        if self.lying {
            self.frame.height()
        } else {
            self.frame.width()
        }
    }

    fn length(&self) -> f32 {
        if self.lying {
            self.frame.width()
        } else {
            self.frame.height()
        }
    }

    /// The mattress: the frame less its rail all round.
    fn mattress(&self) -> Rect {
        Rect::from_min_size(
            self.frame.min + vec2(8.0, 8.0),
            self.frame.size() - vec2(16.0, 16.0),
        )
    }

    /// The deck beside the bunk, on whichever side faces the room, towards
    /// the foot: where a body coming aboard off the deck is stood
    /// (`Game::adopt`).
    pub fn station(&self) -> Vec2 {
        self.at(
            self.side * (self.breadth() / 2.0 + 28.0),
            self.length() - 56.0,
        )
    }

    /// A bunk seen from above: a frame with a headboard standing proud at
    /// the head end and a footboard at the other, a mattress in it, and a
    /// pillow. The bedding goes on in [`Berth::draw_bedding`], over whoever
    /// is lying on the deck beside it.
    pub fn draw(&self, list: &mut DrawList) {
        let f = self.frame;
        let m = self.mattress();
        let (b, l) = (self.breadth(), self.length());

        // The shadow it throws on the deck.
        list.rect(
            f.center() + vec2(3.0, 4.0),
            f.size() + vec2(7.0, 7.0),
            0.0,
            10.0,
            BED_SHADOW,
        );
        // Frame, then mattress, with a seam round the mattress where it
        // meets the rail.
        list.rect(f.center(), f.size(), 0.0, 8.0, BED_FRAME);
        list.rect(m.center(), m.size(), 0.0, 5.0, MATTRESS);
        list.stroke_rect(
            m.center(),
            m.size(),
            0.0,
            5.0,
            1.5,
            MATTRESS_SEAM.alpha(0.5),
        );

        // Headboard and footboard: a board across each end, a little wider
        // than the frame, the head one the taller, lit along the edge that
        // faces the room.
        for (along, depth) in [(5.0, 14.0), (l - 4.0, 9.0)] {
            list.rect(
                self.at(0.0, along),
                self.size(b + 6.0, depth),
                0.0,
                3.0,
                BED_BOARD,
            );
            let lit = if along < l / 2.0 {
                depth * 0.5 - 1.5
            } else {
                1.5 - depth * 0.5
            };
            list.rect(
                self.at(0.0, along + lit),
                self.size(b + 6.0, 2.5),
                0.0,
                1.0,
                BED_BOARD_EDGE,
            );
        }

        // Pillow at the head end. Its height gives way on a narrow bed
        // before its width does.
        let pillow = self.at(0.0, 34.0);
        let pillow_size = (b - 16.0 - 14.0, 32.0f32.min(0.22 * (l - 16.0)));
        list.rect(
            pillow,
            self.size(pillow_size.0, pillow_size.1),
            0.0,
            9.0,
            PILLOW,
        );
        list.line(
            self.at(0.0, 34.0 - 0.4 * pillow_size.1),
            self.at(0.0, 34.0 + 0.4 * pillow_size.1),
            1.5,
            MATTRESS_SEAM.alpha(0.35),
        );
    }

    /// The bedding, made: a duvet from just below the pillow to the foot of
    /// the mattress, a turned-down cuff along its top, and creases running
    /// down towards the foot.
    pub fn draw_bedding(&self, list: &mut DrawList) {
        let m_across = self.breadth() - 16.0;
        let head = 52.0;
        let foot = self.length() - 12.0;
        let mid = (head + foot) * 0.5;
        let width = m_across - 6.0;
        list.rect(
            self.at(0.0, mid),
            self.size(width, foot - head),
            0.0,
            6.0,
            BLANKET,
        );
        list.rect(
            self.at(0.0, head + 5.0),
            self.size(width, 11.0),
            0.0,
            5.0,
            BLANKET_FOLD,
        );
        for side in [-1.0f32, 1.0] {
            list.rect(
                self.at(side * 0.22 * m_across, mid + 8.0),
                self.size(3.5, 0.68 * (foot - head)),
                0.0,
                2.0,
                BED_SHADOW.alpha(0.12),
            );
        }
    }
}

// --- the broom locker --------------------------------------------------------

/// A broom locker: a shallow door, with the head of the broom showing
/// through the vent.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Locker {
    pub frame: Rect,
}

impl Locker {
    pub fn new(frame: Rect) -> Locker {
        Locker { frame }
    }

    pub fn draw(&self, list: &mut DrawList) {
        let l = self.frame;
        list.rect(l.center(), l.size(), 0.0, 3.0, PANEL);
        list.stroke_rect(l.center(), l.size(), 0.0, 3.0, 1.5, PANEL_EDGE);
        // A louvred vent down the door, which is what makes it a cupboard
        // rather than a panel.
        for i in 0..4 {
            let y = lerp(l.min.y + 14.0, l.max.y - 14.0, i as f32 / 3.0);
            list.rect(
                vec2(l.center().x, y),
                vec2(l.width() - 9.0, 2.0),
                0.0,
                1.0,
                PANEL_EDGE.alpha(0.55),
            );
        }
        // The broom behind it.
        list.rect(
            vec2(l.center().x, l.center().y),
            vec2(3.5, l.height() - 24.0),
            0.0,
            1.5,
            BROOM_POLE,
        );
        list.rect(
            vec2(l.center().x, l.max.y - 17.0),
            vec2(l.width() - 8.0, 12.0),
            0.0,
            2.0,
            BROOM_HEAD,
        );
        // The handle, on the deck side.
        list.rect(
            vec2(l.max.x - 3.0, l.center().y),
            vec2(2.5, 18.0),
            0.0,
            1.0,
            HANDLE,
        );
    }
}

// --- the bays and the fields -------------------------------------------------

/// A hydroponic bay — six trays along a frame, under grow lights — or a
/// field, the same trays laid on open ground.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Bay {
    /// The frame itself, which is furniture and gets walked round.
    pub frame: Rect,
    /// Which side it was worked from, as a unit step out of the frame: the
    /// trays run across that way.
    side: Vec2,
    trays: [Rect; TRAYS],
    /// A field rather than a bay.
    outdoor: bool,
}

impl Bay {
    /// A bay wherever a layout puts it. The trays run along the frame the
    /// long way: a bay lying east–west is six trays across, one standing
    /// north–south six trays down.
    pub fn at(frame: Rect, side: Vec2) -> Bay {
        let inner = frame.expand(-7.0);
        let across = side.x.abs() > side.y.abs();
        let trays = core::array::from_fn(|i| {
            if across {
                let height = inner.height() / TRAYS as f32;
                Rect::from_min_size(
                    vec2(inner.min.x, inner.min.y + i as f32 * height + 2.0),
                    vec2(inner.width(), height - 4.0),
                )
            } else {
                let width = inner.width() / TRAYS as f32;
                Rect::from_min_size(
                    vec2(inner.min.x + i as f32 * width + 2.0, inner.min.y),
                    vec2(width - 4.0, inner.height()),
                )
            }
        });
        Bay {
            frame,
            side,
            trays,
            outdoor: false,
        }
    }

    /// A field: a bay's trays laid on open ground.
    pub fn field(frame: Rect, side: Vec2) -> Bay {
        let mut bay = Bay::at(frame, side);
        bay.outdoor = true;
        bay
    }

    /// Whether this is a field rather than a bay.
    pub fn is_field(&self) -> bool {
        self.outdoor
    }

    /// The trays, along the run in order.
    pub fn trays(&self) -> &[Rect; TRAYS] {
        &self.trays
    }

    /// The frame, and a tray under a grow light for each spot, nothing
    /// planted in any of them. A field is turned earth with a furrow down
    /// each tray.
    pub fn draw(&self, list: &mut DrawList) {
        if self.outdoor {
            list.rect(self.frame.center(), self.frame.size(), 0.0, 0.0, EARTH);
            list.stroke_rect(
                self.frame.center(),
                self.frame.size(),
                0.0,
                0.0,
                1.5,
                EARTH_EDGE,
            );
            let across = self.side.x.abs() > self.side.y.abs();
            for tray in &self.trays {
                // The furrow runs the tray's long way.
                let furrow = if across {
                    vec2(tray.width() - 4.0, 1.5)
                } else {
                    vec2(1.5, tray.height() - 4.0)
                };
                list.rect(tray.center(), furrow, 0.0, 0.0, FURROW);
            }
            return;
        }
        list.rect(self.frame.center(), self.frame.size(), 0.0, 0.0, PANEL);
        list.stroke_rect(
            self.frame.center(),
            self.frame.size(),
            0.0,
            0.0,
            1.5,
            PANEL_EDGE,
        );
        for tray in &self.trays {
            list.rect(tray.center(), tray.size(), 0.0, 0.0, PANEL_LIT);
            // The grow light over each tray.
            list.rect(
                vec2(tray.center().x, tray.min.y + 3.0),
                vec2(tray.width() - 6.0, 3.0),
                0.0,
                0.0,
                GLOW,
            );
            list.rect(
                vec2(tray.center().x, tray.center().y),
                vec2(tray.width() - 8.0, tray.height() - 12.0),
                0.0,
                0.0,
                GLOW_DIM.alpha(0.10),
            );
        }
    }
}

// --- the heads ---------------------------------------------------------------

/// A heads: a pan and a basin standing on the deck where a ship design put
/// them, with no compartment of their own. `shell` is the two fixtures'
/// bounding box.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Heads {
    pub shell: Rect,
    pub toilet: Rect,
    pub sink: Rect,
}

impl Heads {
    pub fn new(toilet: Rect, sink: Rect) -> Heads {
        let shell = Rect::from_corners(
            vec2(toilet.min.x.min(sink.min.x), toilet.min.y.min(sink.min.y)),
            vec2(toilet.max.x.max(sink.max.x), toilet.max.y.max(sink.max.y)),
        );
        Heads {
            shell,
            toilet,
            sink,
        }
    }

    /// The pan and the basin, either without the other: a ship being built
    /// may have only one of them yet.
    pub fn draw(&self, list: &mut DrawList, toilet: bool, sink: bool) {
        if toilet {
            draw_toilet(list, self.toilet);
        }
        if sink {
            draw_basin(list, self.sink);
        }
    }
}

/// A pan seen from above: the cistern standing proud of it against the
/// wall, the seat a ring with a gap at the front, and water in the bowl.
fn draw_toilet(list: &mut DrawList, t: Rect) {
    let seat = t.center();
    let cistern = vec2(t.max.x - 16.0, t.center().y);
    list.rect(cistern, vec2(32.0, t.height()), 0.0, 6.0, PANEL_LIT);
    list.stroke_rect(cistern, vec2(32.0, t.height()), 0.0, 6.0, 1.0, PANEL_EDGE);
    // The flush plate on it, lit.
    list.rect(cistern, vec2(14.0, 26.0), 0.0, 3.0, GLOW.alpha(0.35));

    // Pan: a pedestal, wider at the front than where it meets the cistern.
    list.ellipse(seat + vec2(6.0, 0.0), vec2(44.0, 60.0), 0.0, BOWL_RIM);
    list.ellipse(seat, vec2(46.0, 52.0), 0.0, BOWL_RIM);
    list.ellipse(seat, vec2(40.0, 46.0), 0.0, BOWL);

    // The seat: a ring, with the gap at the front the way a seat is cut.
    list.stroke_ellipse(seat, vec2(31.0, 37.0), 0.0, 6.0, BOWL_RIM);
    list.rect(seat - vec2(16.0, 0.0), vec2(9.0, 13.0), 0.0, 2.0, BOWL);

    // Water, lit from under the rim.
    list.ellipse(seat, vec2(24.0, 30.0), 0.0, WATER);
}

/// A basin set into its shelf, with the tap reaching over it.
fn draw_basin(list: &mut DrawList, s: Rect) {
    list.rect(s.center(), s.size(), 0.0, 5.0, PANEL_LIT);
    list.rect(
        vec2(s.center().x, s.max.y - 2.0),
        vec2(s.width(), 4.0),
        0.0,
        2.0,
        PANEL_EDGE,
    );
    let basin = vec2(s.center().x, s.max.y - 13.0);
    list.ellipse(basin, vec2(44.0, 26.0), 0.0, BOWL_RIM);
    list.ellipse(basin, vec2(37.0, 20.0), 0.0, BASIN);
    list.ellipse(basin, vec2(12.0, 7.0), 0.0, BOWL_RIM);
    // Tap: a spout reaching over the basin from the wall side.
    let spout = vec2(basin.x, s.min.y + 8.0);
    list.rect(spout, vec2(9.0, 16.0), 0.0, 3.0, STEEL);
    list.circle(spout + vec2(0.0, 7.0), 8.0, STEEL);
}

// --- what the room only draws ------------------------------------------------

/// A kind of fixture drawn where it stands and nothing else: a second
/// table — every chair is seated already, and a table is what stands
/// between them — and a basin that is no toilet's. Solids the room draws
/// as themselves. See `Layout::extras` and [`Stills`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Still {
    Table,
    Basin,
}

/// The extras, ready to draw.
#[derive(Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Stills {
    pub tables: Vec<Rect>,
    pub basins: Vec<Rect>,
}

impl Stills {
    pub fn from_extras(extras: &[(Still, Rect)]) -> Stills {
        let mut stills = Stills::default();
        for &(kind, frame) in extras {
            match kind {
                Still::Table => stills.tables.push(frame),
                Still::Basin => stills.basins.push(frame),
            }
        }
        stills
    }

    pub fn draw(&self, list: &mut DrawList) {
        for &t in &self.tables {
            draw_table_top(list, t);
        }
        for &basin in &self.basins {
            draw_basin(list, basin);
        }
    }
}
