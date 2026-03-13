use bfo::BfoClass;
use serde::{Deserialize, Serialize};

use crate::time::TimeDelta;
use crate::vocab::{GoalTag, NpcArchetype, Occupation, TraitTag};

macro_rules! labeled_enum {
    ($name:ident { $($variant:ident => $label:literal),+ $(,)? }) => {
        #[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub enum $name {
            $($variant),+
        }

        impl $name {
            pub const fn label(self) -> &'static str {
                match self {
                    $(Self::$variant => $label),+
                }
            }
        }
    };
}

labeled_enum!(RouteKind {
    Hallway => "hallway",
    Stairwell => "stairwell",
    Crosswalk => "crosswalk",
    SideStreet => "side street",
    LocalRoad => "local roads",
    ArterialRoad => "arterial road",
    Highway => "highway",
});

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TravelRoute {
    pub kind: RouteKind,
    pub travel_time: TimeDelta,
}

labeled_enum!(PlaceKind {
    BuildingInterior => "building interior",
    ApartmentLobby => "apartment lobby",
    ApartmentRoom => "apartment room",
    RoadLane => "road lane",
    SidewalkLeft => "left sidewalk",
    SidewalkRight => "right sidewalk",
    StationConcourse => "station concourse",
    StationPlatform => "station platform",
});

impl PlaceKind {
    pub fn supports_people(self) -> bool {
        matches!(
            self,
            Self::BuildingInterior
                | Self::ApartmentLobby
                | Self::ApartmentRoom
                | Self::SidewalkLeft
                | Self::SidewalkRight
                | Self::StationConcourse
                | Self::StationPlatform
        )
    }
}

labeled_enum!(EntityKind {
    Gun => "gun",
    Knife => "knife",
    Bag => "bag",
});

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum RoleKind {
    Occupation(Occupation),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum DispositionKind {
    Trait(TraitTag),
    Goal(GoalTag),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum QualityKind {
    Archetype(NpcArchetype),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RoleNode {
    pub kind: RoleKind,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DispositionNode {
    pub kind: DispositionKind,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct QualityNode {
    pub kind: QualityKind,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum DependentContinuant {
    Role(RoleNode),
    Disposition(DispositionNode),
    Quality(QualityNode),
}

impl DependentContinuant {
    pub const fn bfo_class(&self) -> BfoClass {
        match self {
            Self::Role(_) => BfoClass::Role,
            Self::Disposition(_) => BfoClass::Disposition,
            Self::Quality(_) => BfoClass::Quality,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum OccurrentKind {
    Dialogue,
    Travel { duration: TimeDelta },
    Waiting { duration: TimeDelta },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum RiggyUniversal {
    City,
    District,
    Landmark,
    Place(PlaceKind),
    Npc,
    Player,
    Entity(EntityKind),
    Role(RoleKind),
    Disposition(DispositionKind),
    Quality(QualityKind),
    ConversationMemory,
    DialogueRecord,
    ContextRecord,
    CityKnowledge,
    DialogueProcess,
    TravelProcess,
    WaitingProcess,
    TemporalClock,
}

impl RiggyUniversal {
    pub const fn bfo_class(&self) -> BfoClass {
        match self {
            Self::City | Self::District | Self::Landmark | Self::Place(_) => BfoClass::Site,
            Self::Npc | Self::Player | Self::Entity(_) => BfoClass::Object,
            Self::Role(_) => BfoClass::Role,
            Self::Disposition(_) => BfoClass::Disposition,
            Self::Quality(_) => BfoClass::Quality,
            Self::ConversationMemory
            | Self::DialogueRecord
            | Self::ContextRecord
            | Self::CityKnowledge => BfoClass::GenericallyDependentContinuant,
            Self::DialogueProcess | Self::TravelProcess | Self::WaitingProcess => BfoClass::Process,
            Self::TemporalClock => BfoClass::TemporalRegion,
        }
    }
}
