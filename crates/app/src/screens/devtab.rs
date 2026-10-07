//! The game setup's **Dev** tab (October 2026, the player's: "put in the
//! gamesetup a tab called dev, where i can buy items weapons choose what
//! mission type and so on"): what a playtest run sets out with — which
//! mission every fight of its kind is, the class's level, a weapon and
//! armour, the four item slots, the relics held and a full armory.
//!
//! The host's to pick and dealt with the rest of the settings
//! (`SettingsWire::dev`), so every machine builds the same run; every
//! player's Bim gets the kit. [`apply`] lays it onto the session
//! `designer::build_run` stood up, through the world's probes. The
//! default is nothing at all: the run as it plays.

use bevy_egui::egui;
use bims::combat::{Tier, Weapon, WeaponKind};
use bims::module::{Module, ModuleKind};
use world::run::Mission;

use super::builder::section;
use crate::names::*;
use crate::theme;

/// What the Dev tab picked. `Copy`, so it crosses with the settings.
#[derive(Clone, Copy, PartialEq, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct DevSetup {
    /// Every fight of the mission's kind made it (`World::set_mission_for_probe`);
    /// `None` is the roll.
    pub mission: Option<Mission>,
    /// The class's level at the start; nought or one is the first.
    pub level: u32,
    /// The weapon in every player's Bim's hand; `None` the class's own.
    pub weapon: Option<Weapon>,
    /// Armour at this tier on every player's Bim; `None` the class's own.
    pub armour: Option<Tier>,
    /// The item slots, filled in order.
    pub items: [Option<Module>; 4],
    /// The relics the crew hold from the start, a bit by code.
    pub relics: u32,
    /// Every weapon and armour at every tier in each player's armory.
    pub armory: bool,
}

impl DevSetup {
    /// Whether anything is picked: the footer says so.
    pub fn any(&self) -> bool {
        *self != DevSetup::default()
    }

    fn has_relic(&self, relic: world::Relic) -> bool {
        self.relics & (1 << relic.code()) != 0
    }
}

/// The picks laid onto a session just stood up — after the classes and
/// the relic dials, before the first step.
pub fn apply(dev: &DevSetup, session: &mut ship::Session) {
    if !dev.any() {
        return;
    }
    let Some(game) = session.game.as_mut() else {
        return;
    };
    let world = &mut game.world;
    if let Some(mission) = dev.mission {
        world.set_mission_for_probe(Some(mission));
    }
    let items: Vec<Module> = dev.items.iter().flatten().copied().collect();
    for slot in 0..world.players() {
        if dev.level > 1 && world.class_of(slot) != world::Class::None {
            let want = world::class::LEVEL_XP
                .get(dev.level as usize - 1)
                .copied()
                .unwrap_or(0);
            let mut events = Vec::new();
            world.award(slot as usize, want, &mut events);
        }
        world.kit_out_for_probe(slot, dev.weapon, dev.armour);
        if !items.is_empty() {
            world.give_items_for_probe(slot, &items);
        }
    }
    for relic in world::Relic::ALL {
        if dev.has_relic(relic) {
            world.give_relic_for_probe(relic);
        }
    }
    if dev.armory {
        world.stock_every_thing_for_probe();
    }
}

/// A small toggle: lit while `on`, dead where the setup is not ours.
fn pick(
    ui: &mut egui::Ui,
    label: impl Into<egui::WidgetText>,
    on: bool,
    editable: bool,
) -> egui::Response {
    let button = egui::Button::new(label).min_size(egui::vec2(0.0, 24.0));
    let button = if on {
        button
            .fill(theme::RAISED_ON)
            .stroke(egui::Stroke::new(1.0, theme::ACCENT))
    } else {
        button
    };
    ui.add_enabled(editable, button)
}

/// A tier's colour, as the floor map and the trader draw it.
fn tier_colour(tier: Tier) -> egui::Color32 {
    super::floormap::tier_colour(tier)
}

/// A tier picker: one, two and three, each in its colour.
fn tier_row(ui: &mut egui::Ui, tier: &mut Tier, editable: bool, made: impl Fn(Tier) -> bool) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(DEV_TIER).color(theme::MUTED));
        for t in Tier::ALL {
            let text = egui::RichText::new(t.code().to_string())
                .strong()
                .color(tier_colour(t));
            if ui
                .add_enabled_ui(made(t), |ui| pick(ui, text, *tier == t, editable))
                .inner
                .clicked()
            {
                *tier = t;
            }
        }
    });
}

/// The Dev tab: the mission and the level on the left, the kit on the
/// right — or one under the other on a narrow card.
pub fn dev_page(ui: &mut egui::Ui, dev: &mut DevSetup, editable: bool) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(DEV_NOTE).color(theme::MUTED));
        if dev.any()
            && ui
                .add_enabled(editable, egui::Button::new(DEV_RESET))
                .clicked()
        {
            *dev = DevSetup::default();
        }
    });
    ui.add_space(8.0);
    let left = |ui: &mut egui::Ui, dev: &mut DevSetup| {
        section(ui, DEV_MISSION, Some(DEV_MISSION_TIP), |ui| {
            mission_rows(ui, dev, editable);
        });
        section(ui, DEV_LEVEL, Some(DEV_LEVEL_TIP), |ui| {
            let mut level = dev.level.max(1);
            ui.add_enabled(
                editable,
                egui::Slider::new(&mut level, 1..=u32::from(world::class::LEVELS)),
            );
            dev.level = if level > 1 { level } else { 0 };
        });
        section(ui, DEV_RELICS, Some(DEV_RELICS_TIP), |ui| {
            relic_rows(ui, dev, editable);
        });
        section(ui, DEV_ARMORY, Some(DEV_ARMORY_TIP), |ui| {
            ui.horizontal(|ui| {
                if pick(ui, DEV_ARMORY_EMPTY, !dev.armory, editable).clicked() {
                    dev.armory = false;
                }
                if pick(ui, DEV_ARMORY_FULL, dev.armory, editable).clicked() {
                    dev.armory = true;
                }
            });
        });
    };
    let right = |ui: &mut egui::Ui, dev: &mut DevSetup| {
        section(ui, DEV_WEAPON, None, |ui| weapon_rows(ui, dev, editable));
        section(ui, DEV_ARMOUR, None, |ui| {
            ui.horizontal(|ui| {
                if pick(ui, DEV_CLASS_KIT, dev.armour.is_none(), editable).clicked() {
                    dev.armour = None;
                }
                for t in Tier::ALL {
                    let text = egui::RichText::new(format!("{DEV_TIER} {}", t.code()))
                        .color(tier_colour(t));
                    if pick(ui, text, dev.armour == Some(t), editable).clicked() {
                        dev.armour = Some(t);
                    }
                }
            });
        });
        section(ui, DEV_ITEMS, Some(DEV_ITEMS_TIP), |ui| {
            item_rows(ui, dev, editable)
        });
    };
    if ui.available_width() >= 660.0 {
        ui.columns(2, |columns| {
            let (l, r) = columns.split_at_mut(1);
            left(&mut l[0], dev);
            right(&mut r[0], dev);
        });
    } else {
        left(ui, dev);
        right(ui, dev);
    }
}

/// The mission picks: the roll, plain, and the four, each with where it
/// goes.
fn mission_rows(ui: &mut egui::Ui, dev: &mut DevSetup, editable: bool) {
    let options: [(Option<Mission>, &str, &str); 6] = [
        (None, DEV_ROLLED, DEV_AS_PLAYED),
        (
            Some(Mission::Plain),
            dev_mission_name(Mission::Plain),
            DEV_NO_MISSION,
        ),
        (
            Some(Mission::Breaches),
            dev_mission_name(Mission::Breaches),
            DEV_EVERY_DEFENCE,
        ),
        (
            Some(Mission::Evacuation),
            dev_mission_name(Mission::Evacuation),
            DEV_EVERY_DEFENCE,
        ),
        (
            Some(Mission::Sabotage),
            dev_mission_name(Mission::Sabotage),
            DEV_EVERY_ATTACK,
        ),
        (
            Some(Mission::Nests),
            dev_mission_name(Mission::Nests),
            DEV_EVERY_ATTACK,
        ),
    ];
    ui.horizontal_wrapped(|ui| {
        for (value, name, sub) in options {
            let text = egui::text::LayoutJob::simple_singleline(
                format!("{name} · {sub}"),
                egui::FontId::proportional(14.0),
                theme::INK,
            );
            if pick(ui, text, dev.mission == value, editable).clicked() {
                dev.mission = value;
            }
        }
    });
}

/// The weapon picks: the class's own, or a kind, and its tier.
fn weapon_rows(ui: &mut egui::Ui, dev: &mut DevSetup, editable: bool) {
    ui.horizontal_wrapped(|ui| {
        if pick(ui, DEV_CLASS_KIT, dev.weapon.is_none(), editable).clicked() {
            dev.weapon = None;
        }
        for kind in WeaponKind::ALL {
            let on = dev.weapon.is_some_and(|w| w.kind == kind);
            if pick(ui, weapon_name(Some(kind)), on, editable).clicked() {
                let tier = dev.weapon.map_or(Tier::One, |w| w.tier);
                dev.weapon = Some(kind.at(tier.max(kind.min_tier())));
            }
        }
    });
    if let Some(weapon) = dev.weapon.as_mut() {
        let kind = weapon.kind;
        let mut tier = weapon.tier;
        tier_row(ui, &mut tier, editable, |t| kind.made_at(t));
        if tier != weapon.tier {
            *weapon = kind.at(tier);
        }
    }
}

/// The four item slots — a click empties one — and the shop under them:
/// every item at the tier picked, a click buying it into the next free
/// slot.
fn item_rows(ui: &mut egui::Ui, dev: &mut DevSetup, editable: bool) {
    ui.horizontal(|ui| {
        let gap = ui.spacing().item_spacing.x;
        let width = ((ui.available_width() - 3.0 * gap) / 4.0).max(60.0);
        for slot in dev.items.iter_mut() {
            let (text, colour) = match slot {
                Some(item) => (
                    format!("{} {}", item_name(item.kind), item.tier.code()),
                    tier_colour(item.tier),
                ),
                None => (DEV_SLOT_EMPTY.to_string(), theme::MUTED),
            };
            let button = egui::Button::new(egui::RichText::new(text).color(colour))
                .min_size(egui::vec2(width, 32.0))
                .truncate()
                .stroke(egui::Stroke::new(1.0, colour.gamma_multiply(0.6)));
            let response = ui.add_enabled(editable, button);
            let response = match slot {
                Some(item) => response.on_hover_text(item_line(*item)),
                None => response,
            };
            if response.clicked() {
                *slot = None;
            }
        }
    });
    ui.add_space(6.0);
    // The tier the shop sells at, kept on the egui memory for the tab.
    let id = egui::Id::new("dev-item-tier");
    let mut tier = ui
        .data(|d| d.get_temp::<u32>(id))
        .and_then(Tier::from_code)
        .unwrap_or(Tier::One);
    tier_row(ui, &mut tier, editable, |_| true);
    ui.data_mut(|d| d.insert_temp(id, tier.code()));
    let full = dev.items.iter().all(Option::is_some);
    ui.horizontal_wrapped(|ui| {
        for kind in ModuleKind::ALL {
            let at = if kind.made_at(tier) {
                tier
            } else {
                kind.min_tier()
            };
            let item = kind.at(at);
            let text = egui::RichText::new(item_name(kind)).color(tier_colour(at));
            let response = ui
                .add_enabled(editable && !full, egui::Button::new(text))
                .on_hover_text(item_line(item));
            if response.clicked()
                && let Some(free) = dev.items.iter_mut().find(|s| s.is_none())
            {
                *free = Some(item);
            }
        }
    });
}

/// The relics, a toggle each, the boon and the price on the tooltip.
fn relic_rows(ui: &mut egui::Ui, dev: &mut DevSetup, editable: bool) {
    ui.horizontal_wrapped(|ui| {
        for relic in world::Relic::ALL {
            let on = dev.has_relic(relic);
            let tip = relic_lines(relic)
                .into_iter()
                .map(|(line, _)| line)
                .collect::<Vec<_>>()
                .join("\n");
            if pick(ui, relic_name(relic), on, editable)
                .on_hover_text(tip)
                .clicked()
            {
                dev.relics ^= 1 << relic.code();
            }
        }
    });
}
