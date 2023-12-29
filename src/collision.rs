use bevy_rapier2d::geometry::Group;

pub const GROUP_WALL: Group = Group::from_bits_truncate(0b0001);
pub const GROUP_ACTOR: Group = Group::from_bits_truncate(0b0010);
pub const GROUP_PIT: Group = Group::from_bits_truncate(0b0100);
pub const GROUP_PIT_WALL: Group = Group::from_bits_truncate(0b1000);
pub const GROUP_ONLY_ALL: Group = Group::GROUP_32;
pub const FILTER_MAIN: Group = Group::from_bits_truncate(0b0011);
pub const FILTER_PITS: Group = Group::from_bits_truncate(0b0100);
// XXX should include pit walls but that's too buggy for now
pub const FILTER_WALLS: Group = Group::from_bits_truncate(0b0001);
