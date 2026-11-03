use std::collections::{HashMap, HashSet, VecDeque};
use std::time::{Duration, Instant};

pub const BED: u16 = 26;
pub const RANGE: f64 = 100.0;
const SCAN_INTERVAL: Duration = Duration::from_secs(2);
const SCAN_BUDGET: Duration = Duration::from_micros(1000);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Bed {
    pub head: [i32; 3],
    pub foot: [i32; 3],
}
impl Bed {
    pub fn blocks(&self) -> [[i32; 3]; 2] {
        [self.head, self.foot]
    }
    pub fn centre(&self) -> [f64; 3] {
        [
            (self.head[0] + self.foot[0]) as f64 * 0.5 + 0.5,
            self.head[1] as f64,
            (self.head[2] + self.foot[2]) as f64 * 0.5 + 0.5,
        ]
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Defense {
    pub bed: Bed,
    pub icons: Vec<u16>,
    pub status: Option<&'static str>,
}

pub fn position(chunk_x: i32, section_y: i32, chunk_z: i32, i: usize) -> [i32; 3] {
    [
        chunk_x * 16 + (i & 15) as i32,
        section_y * 16 + (i >> 8) as i32,
        chunk_z * 16 + ((i >> 4) & 15) as i32,
    ]
}

pub fn section_beds(states: &[u16; 4096], chunk_x: i32, section_y: i32, chunk_z: i32) -> Vec<Bed> {
    states
        .iter()
        .enumerate()
        .filter(|&(_, &state)| state >> 4 == BED && state & 8 != 0)
        .map(|(i, &state)| {
            let head = position(chunk_x, section_y, chunk_z, i);
            let (dx, dz) = match state & 3 {
                0 => (0, 1),
                1 => (-1, 0),
                2 => (0, -1),
                _ => (1, 0),
            };
            Bed {
                head,
                foot: [head[0] - dx, head[1], head[2] - dz],
            }
        })
        .collect()
}

pub fn icon_state(state: u16) -> u16 {
    let (id, meta) = (state >> 4, state & 15);
    let meta = match id {
        1 | 5 | 24 | 35 | 95 | 98 | 159 | 160 | 171 => meta,
        17 | 162 => meta & 3,
        _ => 0,
    };
    id << 4 | meta
}

fn defends(state: u16) -> bool {
    !matches!(state >> 4, 0 | 8 | 9 | 10 | 11 | 26 | 51)
}

pub fn defense(bed: Bed, layers: i32, mut state: impl FnMut([i32; 3]) -> u16) -> Defense {
    let layers = layers.max(1);
    let head = bed.head;
    let toward_foot = [bed.foot[0] - head[0], bed.foot[2] - head[2]];
    let mut rings: Vec<Vec<(u16, u32)>> = vec![Vec::new(); layers as usize];
    for n in 1..=layers {
        let (x0, x1) = (-n + toward_foot[0].min(0), n + toward_foot[0].max(0));
        let (z0, z1) = (-n + toward_foot[1].min(0), n + toward_foot[1].max(0));
        for dy in 0..=n {
            for dx in x0..=x1 {
                for dz in z0..=z1 {
                    let edge = dx == x0 || dx == x1 || dz == z0 || dz == z1;
                    if !edge && dy != n {
                        continue;
                    }
                    let reach = |ox: i32, oz: i32| {
                        (((dx - ox) * (dx - ox) + (dz - oz) * (dz - oz)) as f64).sqrt() + dy as f64
                    };
                    let far = reach(0, 0) > n as f64 && reach(toward_foot[0], toward_foot[1]) > n as f64;
                    let ring = if far { n + 1 } else { n };
                    if ring > layers {
                        continue;
                    }
                    let block = state([head[0] + dx, head[1] + dy, head[2] + dz]);
                    let key = if defends(block) { icon_state(block) } else { 0 };
                    let tally = &mut rings[ring as usize - 1];
                    match tally.iter_mut().find(|t| t.0 == key) {
                        Some(t) => t.1 += 1,
                        None => tally.push((key, 1)),
                    }
                }
            }
        }
    }
    for tally in &mut rings {
        tally.sort_by_key(|t| std::cmp::Reverse(t.1));
    }
    let open = rings[0].iter().find(|t| t.0 == 0).map_or(0, |t| t.1);
    let solid: u32 = rings[0].iter().filter(|t| t.0 != 0).map(|t| t.1).sum();
    let status = if solid == 0 {
        Some("Not Protected")
    } else if open > 0 {
        Some("Not Full Protected")
    } else {
        None
    };
    let mut icons = Vec::new();
    for tally in &rings {
        let mut any = false;
        for &(key, _) in tally.iter().filter(|t| t.0 != 0) {
            any = true;
            if !icons.contains(&key) {
                icons.push(key);
            }
        }
        if !any {
            break;
        }
    }
    Defense { bed, icons, status }
}

#[derive(Default)]
pub struct Scanner {
    beds: HashMap<(i32, i32), Vec<Bed>>,
    pending: VecDeque<(i32, i32)>,
    queued: HashSet<(i32, i32)>,
    last_scan: Option<Instant>,
}
impl Scanner {
    pub fn tick<E>(
        &mut self,
        (pcx, pcz): (i32, i32),
        mut scan: impl FnMut(i32, i32) -> Result<Vec<Bed>, E>,
    ) -> Result<(), E> {
        let radius = (RANGE / 16.0).ceil() as i32;
        if self.pending.is_empty() && self.last_scan.is_none_or(|t| t.elapsed() >= SCAN_INTERVAL) {
            self.last_scan = Some(Instant::now());
            for dx in -radius..=radius {
                for dz in -radius..=radius {
                    let chunk = (pcx + dx, pcz + dz);
                    if self.queued.insert(chunk) {
                        self.pending.push_back(chunk);
                    }
                }
            }
            self.beds
                .retain(|&(x, z), _| (x - pcx).abs() <= radius + 1 && (z - pcz).abs() <= radius + 1);
        }
        let start = Instant::now();
        while let Some(chunk) = self.pending.pop_front() {
            self.queued.remove(&chunk);
            let found = scan(chunk.0, chunk.1)?;
            if found.is_empty() {
                self.beds.remove(&chunk);
            } else {
                self.beds.insert(chunk, found);
            }
            if start.elapsed() >= SCAN_BUDGET {
                break;
            }
        }
        Ok(())
    }

    pub fn beds(&self) -> impl Iterator<Item = &Bed> {
        self.beds.values().flatten()
    }
}
