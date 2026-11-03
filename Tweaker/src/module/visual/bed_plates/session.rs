use std::time::{Duration, Instant};

const REFRESH: Duration = Duration::from_millis(500);
const START_SCAN_DELAY: Duration = Duration::from_secs(5);
const REJOIN_SCAN_DELAY: Duration = Duration::from_secs(10);
pub const OWN_BED_RADIUS: i32 = 25;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Bedwars {
    pub waiting: bool,
    pub game: bool,
}
impl Bedwars {
    pub fn playing(&self) -> bool {
        self.game && !self.waiting
    }

    pub fn read(title: &str, lines: &[String]) -> Self {
        let line = |text: &str| lines.iter().any(|l| l.contains(text));
        let any = title.contains("bed wars") || title.contains("bedwars");
        let practice = title.contains("bed wars practice");
        let lobby = any && !practice && line("level:");
        let waiting = any && !practice && (line("waiting...") || line("starting in"));
        let game = any && !practice && !lobby;
        Self { waiting, game }
    }
}

#[derive(Default)]
pub struct Session {
    refreshed: Option<Instant>,
    pub bedwars: Bedwars,
    scan_at: Option<Instant>,
    pub own_bed: Option<[[i32; 3]; 2]>,
}
impl Session {
    pub fn due(&self) -> bool {
        self.refreshed.is_none_or(|t| t.elapsed() >= REFRESH)
    }

    pub fn update(&mut self, read: Bedwars) {
        let (now, before) = (Instant::now(), self.bedwars);
        self.refreshed = Some(now);
        self.bedwars = read;
        if !read.game {
            self.own_bed = None;
            self.scan_at = None;
        } else if read.playing() && !before.playing() && self.own_bed.is_none() {
            self.scan_at = Some(now + if before.waiting { START_SCAN_DELAY } else { REJOIN_SCAN_DELAY });
        }
    }

    pub fn take_scan(&mut self) -> bool {
        let due = self.scan_at.is_some_and(|t| Instant::now() >= t);
        if due {
            self.scan_at = None;
        }
        due
    }

    pub fn is_own_bed(&self, block: [i32; 3]) -> bool {
        self.own_bed.is_some_and(|halves| halves.contains(&block))
    }
}

pub fn nearest_bed(beds: &[([i32; 3], bool)], [px, py, pz]: [i32; 3]) -> Option<[[i32; 3]; 2]> {
    let distance = |b: [i32; 3]| {
        let (dx, dy, dz) = ((b[0] - px) as i64, (b[1] - py) as i64, (b[2] - pz) as i64);
        dx * dx + dy * dy + dz * dz
    };
    let &(nearest, head) = beds.iter().min_by_key(|(b, _)| distance(*b))?;
    let other = beds
        .iter()
        .map(|&(b, _)| b)
        .find(|b| b[1] == nearest[1] && (b[0] - nearest[0]).abs() + (b[2] - nearest[2]).abs() == 1)
        .unwrap_or(nearest);
    Some(if head { [nearest, other] } else { [other, nearest] })
}
