use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct PlayerId(pub u8);

/// A game object. Per CR 400.7 an object that changes zones becomes a new object, so it
/// gets a new `ObjectId`; `CardId` stays with the physical card for its whole life.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ObjectId(pub u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct CardId(pub u32);

/// Timestamps order continuous effects and objects (CR 613.7).
pub type Timestamp = u64;
