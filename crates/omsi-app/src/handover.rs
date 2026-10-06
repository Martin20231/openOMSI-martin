//! The change of driver in multiplayer: the driver hands his bus to a friend who rides in it.
//!
//! A bus is worked out only in its driver's game; the others see its pose. So the bus
//! changes games: the driver offers the wheel to a player aboard (`drive offer`), the player
//! takes it (`drive take`), the driver's game takes its bus out of the world and puts the
//! driver beside its door (`drive go`), and the friend's game puts the same bus - the same
//! vehicle, livery, place, line and tour, as the friend last saw it - into its own world,
//! started up, with the friend at the wheel. His own bus stays where he left it.
//!
//! What the bus held that its pose does not say stays behind: its riders get out, its
//! switches start again from a running bus.

use crate::app::App;
use glam::DVec3;

/// The bus as the friend saw it when he took the wheel.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Snapshot {
    pub bus: String,
    pub paint: String,
    pub pos: DVec3,
    pub heading: f64,
    pub line: String,
    pub tour: String,
}

/// The change of driver, in both games.
#[derive(Clone, Debug, Default)]
pub(crate) struct Handover {
    /// (the friend) A driver offers us his bus: his id and name.
    pub offer: Option<(u32, String)>,
    /// (the friend) We took it: the driver's id, the bus as we saw it, seconds left to wait.
    pub taking: Option<(u32, Snapshot, f32)>,
    /// (the driver) We offered our bus to this player: id, seconds left to wait.
    pub offered: Option<(u32, f32)>,
}

/// Seconds an offer, and the wait for the bus, last.
const WAIT: f32 = 30.0;
/// The bus must stand: at most this fast (km/h).
pub(crate) const STAND_KMH: f32 = 2.0;

/// The `spawn` value for the bus at `s`: x,y,heading,z.
pub(crate) fn spawn_of(s: &Snapshot) -> String {
    format!("{:.3},{:.3},{:.2},{:.3}", s.pos.x, s.pos.y, s.heading, s.pos.z)
}

/// The players of `riders` (id, name, aboard bus owner) who ride in `my_id`'s bus.
pub(crate) fn riders_of(my_id: u32, riders: &[(u32, String, Option<u32>)]) -> Vec<(u32, String)> {
    riders.iter().filter(|r| r.2 == Some(my_id) && r.0 != my_id).map(|r| (r.0, r.1.clone())).collect()
}

impl App {
    fn hand_my_id(&self) -> Option<u32> {
        self.lan.as_ref().map(|l| l.my_id).filter(|i| *i != 0)
    }

    fn hand_send(&mut self, to: u32, text: &str) {
        if let Some(l) = self.lan.as_mut() {
            l.command(to, text);
        }
    }

    fn hand_name(&self, id: u32) -> String {
        self.lan
            .as_ref()
            .and_then(|l| l.peers().find(|p| p.pose.id == id).map(|p| p.pose.name.trim().to_string()))
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| format!("{} {id}", omsi_ui::tr("Player")))
    }

    /// The players riding in the bus we drive (id, name).
    pub(crate) fn riders_in_my_bus(&self) -> Vec<(u32, String)> {
        let (Some(l), Some(me)) = (self.lan.as_ref(), self.hand_my_id()) else { return Vec::new() };
        let all: Vec<(u32, String, Option<u32>)> = l
            .peers()
            .map(|p| {
                let name = if p.pose.name.trim().is_empty() { format!("{} {}", omsi_ui::tr("Player"), p.pose.id) } else { p.pose.name.trim().to_string() };
                (p.pose.id, name, p.pose.walker.as_ref().and_then(|w| w.aboard).map(|a| a.owner))
            })
            .collect();
        riders_of(me, &all)
    }

    /// Our bus stands still (the wheel may change hands).
    pub(crate) fn bus_stands(&self) -> bool {
        self.player.as_ref().is_some_and(|p| p.vehicle.physics.velocity_kmh().abs() <= STAND_KMH)
    }

    /// (the driver) Offer the wheel to player `id`, who rides in our bus.
    pub(crate) fn offer_wheel(&mut self, id: u32) {
        if !self.bus_stands() {
            self.service_msg = Some((omsi_ui::tr("Stop the bus first: the wheel changes hands at a standstill").into_owned(), 5.0));
            return;
        }
        if !self.riders_in_my_bus().iter().any(|r| r.0 == id) {
            return;
        }
        self.handover.offered = Some((id, WAIT));
        self.hand_send(id, "drive offer");
        let who = self.hand_name(id);
        self.service_msg = Some((omsi_ui::tr("You offer {who} the wheel").replace("{who}", &who), 5.0));
    }

    /// (the friend) Our answer to the offer.
    pub(crate) fn answer_wheel(&mut self, yes: bool) {
        let Some((from, _)) = self.handover.offer.take() else { return };
        if !yes {
            self.hand_send(from, "drive no");
            return;
        }
        // the bus as we see it now: that is the one we shall drive
        let snap = self.lan.as_ref().and_then(|l| l.peers().find(|p| p.pose.id == from)).filter(|p| p.pose.has_vehicle()).map(|p| Snapshot {
            bus: p.pose.bus.clone(),
            paint: p.pose.paint.clone(),
            pos: DVec3::new(p.pose.x, p.pose.y, p.pose.z),
            heading: p.pose.heading as f64,
            line: p.pose.line.trim().to_string(),
            tour: p.pose.tour.trim().to_string(),
        });
        match snap {
            Some(s) => {
                self.handover.taking = Some((from, s, WAIT));
                self.hand_send(from, "drive take");
                self.service_msg = Some((omsi_ui::tr("The driver gets up…").into_owned(), 4.0));
            }
            None => {
                self.hand_send(from, "drive no");
                self.service_msg = Some((omsi_ui::tr("That bus is not to be seen any more").into_owned(), 4.0));
            }
        }
    }

    /// A `drive …` command from player `from`.
    pub(crate) fn handover_command(&mut self, from: u32, text: &str) {
        match text.trim() {
            // (driver → us)
            "offer" => {
                let who = self.hand_name(from);
                self.handover.offer = Some((from, who.clone()));
                self.service_msg = Some((omsi_ui::tr("{who} offers you the wheel: the tablet (F8) asks you").replace("{who}", &who), 10.0));
            }
            "go" => {
                if let Some((owner, snap, _)) = self.handover.taking.take().filter(|t| t.0 == from) {
                    self.take_wheel(owner, snap);
                }
            }
            "cancel" => {
                if self.handover.taking.as_ref().is_some_and(|t| t.0 == from) {
                    self.handover.taking = None;
                    self.service_msg = Some((omsi_ui::tr("The driver kept the wheel").into_owned(), 5.0));
                }
                if self.handover.offer.as_ref().is_some_and(|o| o.0 == from) {
                    self.handover.offer = None;
                }
            }
            // (friend → us, the driver)
            "take" => {
                let asked = self.handover.offered.take().is_some_and(|o| o.0 == from);
                let aboard = self.riders_in_my_bus().iter().any(|r| r.0 == from);
                if !(asked && aboard && self.bus_stands() && self.player.is_some()) {
                    self.hand_send(from, "drive cancel");
                    return;
                }
                let who = self.hand_name(from);
                self.remove_driven_vehicle();
                self.hand_send(from, "drive go");
                self.service_msg = Some((omsi_ui::tr("{who} drives your bus now").replace("{who}", &who), 6.0));
            }
            "no" => {
                if self.handover.offered.take().is_some() {
                    let who = self.hand_name(from);
                    self.service_msg = Some((omsi_ui::tr("{who} does not take the wheel").replace("{who}", &who), 5.0));
                }
            }
            other => log::info!("handover: '{other}' from player {from} not taken"),
        }
    }

    /// (the friend) The driver got up: the bus is ours now.
    fn take_wheel(&mut self, owner: u32, snap: Snapshot) {
        let file = match crate::lan::remote_bus_file(&self.args, &snap.bus) {
            Ok(f) => f,
            Err(e) => {
                log::warn!("handover: bus {} of player {owner}: {e}", snap.bus);
                self.service_msg = Some((omsi_ui::tr("This bus is not installed here: the change of driver failed").into_owned(), 8.0));
                return;
            }
        };
        let one = crate::Args {
            bus: Some(file.to_string_lossy().to_string()),
            spawn: Some(spawn_of(&snap)),
            paint: Some(snap.paint.clone()).filter(|p| !p.trim().is_empty()),
            plate: None,
            number: None,
            situation_vars: Vec::new(),
            situation_strvars: Vec::new(),
            situation_others: Vec::new(),
            line: None,
            tour: None,
            trip: None,
            autostart: false,
            situation_next_stop: None,
            ..self.args.clone()
        };
        let spawned = match (self.world.clone(), self.renderer.as_ref(), self.scene.as_mut()) {
            (Some(w), Some(r), Some(scene)) => crate::spawn_player(&one, &w, r, scene),
            _ => return,
        };
        match spawned {
            Ok(Some(q)) => {
                self.placed.push(q);
                let k = self.placed.len() - 1;
                self.take_placed(k);
                if let Some(p) = self.player.as_mut() {
                    let msg = p.start_up();
                    log::info!("handover: took the wheel of player {owner}'s bus ({msg})");
                }
                // the duty the driver was on, when our timetable knows it
                let known = self.schedule.as_ref().is_some_and(|s| s.data.lines.iter().any(|l| l.name.eq_ignore_ascii_case(&snap.line) && l.tours.iter().any(|t| t.number.trim().eq_ignore_ascii_case(&snap.tour))));
                if known {
                    crate::game_lists::start_duty(self, &snap.line, &snap.tour);
                }
                self.service_msg = Some((omsi_ui::tr("You drive the bus now").into_owned(), 5.0));
            }
            Ok(None) => {}
            Err(e) => {
                log::warn!("handover: {e:#}");
                self.service_msg = Some((omsi_ui::tr("This bus is not installed here: the change of driver failed").into_owned(), 8.0));
            }
        }
    }

    /// Every frame: offers and waits run out.
    pub(crate) fn handover_tick(&mut self, dt: f32) {
        if let Some((id, t)) = self.handover.offered.as_mut() {
            *t -= dt;
            if *t <= 0.0 {
                let id = *id;
                self.handover.offered = None;
                self.hand_send(id, "drive cancel");
            }
        }
        if let Some((_, _, t)) = self.handover.taking.as_mut() {
            *t -= dt;
            if *t <= 0.0 {
                self.handover.taking = None;
                self.service_msg = Some((omsi_ui::tr("The driver kept the wheel").into_owned(), 5.0));
            }
        }
        if self.lan.is_none() {
            self.handover = Handover::default();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_those_aboard_this_bus_may_take_the_wheel() {
        let all = vec![(2, "Kev".to_string(), Some(1)), (3, "Lena".to_string(), Some(4)), (4, "Tobi".to_string(), None), (1, "Me".to_string(), Some(1))];
        assert_eq!(riders_of(1, &all), vec![(2, "Kev".to_string())]);
        assert!(riders_of(5, &all).is_empty());
    }

    #[test]
    fn the_bus_comes_where_it_stood() {
        let s = Snapshot { bus: "Vehicles/x/x.bus".into(), paint: String::new(), pos: DVec3::new(10.5, -3.25, 41.0), heading: 92.5, line: "154".into(), tour: "4".into() };
        assert_eq!(spawn_of(&s), "10.500,-3.250,92.50,41.000");
    }
}
