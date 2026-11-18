//! Port of `Trackers.java` — the shared holder of surface trackers across
//! all placed models. Fixed capacity of 500 like the original arrays; `nt`
//! is the count in use.

pub struct Trackers {
    pub x: Vec<i32>,
    pub y: Vec<i32>,
    pub z: Vec<i32>,
    pub xy: Vec<i32>,
    pub zy: Vec<i32>,
    pub skd: Vec<i32>,
    pub dam: Vec<i32>,
    pub c: Vec<[i32; 3]>,
    pub radx: Vec<i32>,
    pub radz: Vec<i32>,
    pub rady: Vec<i32>,
    /// Count in use (original field `nt`).
    pub nt: usize,
}

pub const TRACKER_CAP: usize = 500;

impl Default for Trackers {
    fn default() -> Self {
        Self::new()
    }
}

impl Trackers {
    pub fn new() -> Trackers {
        Trackers {
            x: vec![0; TRACKER_CAP],
            y: vec![0; TRACKER_CAP],
            z: vec![0; TRACKER_CAP],
            xy: vec![0; TRACKER_CAP],
            zy: vec![0; TRACKER_CAP],
            skd: vec![0; TRACKER_CAP],
            dam: vec![0; TRACKER_CAP],
            c: vec![[0; 3]; TRACKER_CAP],
            radx: vec![0; TRACKER_CAP],
            radz: vec![0; TRACKER_CAP],
            rady: vec![0; TRACKER_CAP],
            nt: 0,
        }
    }
}
