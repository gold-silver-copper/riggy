use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Biome {
    Coastal,
    Suburban,
    Desert,
    Riverfront,
    Mountain,
    Wetland,
    Plains,
    Industrial,
}

impl Biome {
    pub const ALL: [Self; 8] = [
        Self::Coastal,
        Self::Suburban,
        Self::Desert,
        Self::Riverfront,
        Self::Mountain,
        Self::Wetland,
        Self::Plains,
        Self::Industrial,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Coastal => "coastal",
            Self::Suburban => "suburban",
            Self::Desert => "desert",
            Self::Riverfront => "riverfront",
            Self::Mountain => "mountain",
            Self::Wetland => "wetland",
            Self::Plains => "plains",
            Self::Industrial => "industrial",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Economy {
    Trade,
    Logistics,
    Manufacturing,
    Tech,
    Tourism,
    Healthcare,
    Finance,
    Shipping,
}

impl Economy {
    pub const ALL: [Self; 8] = [
        Self::Trade,
        Self::Logistics,
        Self::Manufacturing,
        Self::Tech,
        Self::Tourism,
        Self::Healthcare,
        Self::Finance,
        Self::Shipping,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Trade => "trade",
            Self::Logistics => "logistics",
            Self::Manufacturing => "manufacturing",
            Self::Tech => "tech",
            Self::Tourism => "tourism",
            Self::Healthcare => "healthcare",
            Self::Finance => "finance",
            Self::Shipping => "shipping",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Culture {
    Formal,
    StartupDriven,
    LaidBack,
    StatusConscious,
    Stoic,
    CivicMinded,
    ArtForward,
    NightlifeHeavy,
}

impl Culture {
    pub const ALL: [Self; 8] = [
        Self::Formal,
        Self::StartupDriven,
        Self::LaidBack,
        Self::StatusConscious,
        Self::Stoic,
        Self::CivicMinded,
        Self::ArtForward,
        Self::NightlifeHeavy,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Formal => "formal",
            Self::StartupDriven => "startup-driven",
            Self::LaidBack => "laid-back",
            Self::StatusConscious => "status-conscious",
            Self::Stoic => "stoic",
            Self::CivicMinded => "civic-minded",
            Self::ArtForward => "art-forward",
            Self::NightlifeHeavy => "nightlife-heavy",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum NpcArchetype {
    Gossip,
    Fixer,
    Scholar,
    Watcher,
    Networker,
    Creative,
    Contractor,
    Organizer,
}

impl NpcArchetype {
    pub const ALL: [Self; 8] = [
        Self::Gossip,
        Self::Fixer,
        Self::Scholar,
        Self::Watcher,
        Self::Networker,
        Self::Creative,
        Self::Contractor,
        Self::Organizer,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Gossip => "gossip",
            Self::Fixer => "fixer",
            Self::Scholar => "scholar",
            Self::Watcher => "watcher",
            Self::Networker => "networker",
            Self::Creative => "creative",
            Self::Contractor => "contractor",
            Self::Organizer => "organizer",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Occupation {
    SoftwareEngineer,
    Barista,
    DeliveryDriver,
    Journalist,
    CityPlanner,
    SecurityGuard,
    BreweryManager,
    RideshareDriver,
    RealEstateAgent,
    Teacher,
}

impl Occupation {
    pub const ALL: [Self; 10] = [
        Self::SoftwareEngineer,
        Self::Barista,
        Self::DeliveryDriver,
        Self::Journalist,
        Self::CityPlanner,
        Self::SecurityGuard,
        Self::BreweryManager,
        Self::RideshareDriver,
        Self::RealEstateAgent,
        Self::Teacher,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::SoftwareEngineer => "software engineer",
            Self::Barista => "barista",
            Self::DeliveryDriver => "delivery driver",
            Self::Journalist => "journalist",
            Self::CityPlanner => "city planner",
            Self::SecurityGuard => "security guard",
            Self::BreweryManager => "brewery manager",
            Self::RideshareDriver => "rideshare driver",
            Self::RealEstateAgent => "real estate agent",
            Self::Teacher => "teacher",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TraitTag {
    Warm,
    Suspicious,
    DryHumored,
    Ambitious,
    Patient,
    Nervous,
    Idealistic,
    Cunning,
    Guarded,
    Generous,
}

impl TraitTag {
    pub const ALL: [Self; 10] = [
        Self::Warm,
        Self::Suspicious,
        Self::DryHumored,
        Self::Ambitious,
        Self::Patient,
        Self::Nervous,
        Self::Idealistic,
        Self::Cunning,
        Self::Guarded,
        Self::Generous,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Warm => "warm",
            Self::Suspicious => "suspicious",
            Self::DryHumored => "dry-humored",
            Self::Ambitious => "ambitious",
            Self::Patient => "patient",
            Self::Nervous => "nervous",
            Self::Idealistic => "idealistic",
            Self::Cunning => "cunning",
            Self::Guarded => "guarded",
            Self::Generous => "generous",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum GoalTag {
    ProtectNeighborhood,
    SaveToMove,
    ExposeRecordsLeak,
    ImpressCityHall,
    BuryDamagingStory,
    RebuildFamilyBusiness,
    MapRegionalRoutes,
    VerifyOnlineRumor,
}

impl GoalTag {
    pub const ALL: [Self; 8] = [
        Self::ProtectNeighborhood,
        Self::SaveToMove,
        Self::ExposeRecordsLeak,
        Self::ImpressCityHall,
        Self::BuryDamagingStory,
        Self::RebuildFamilyBusiness,
        Self::MapRegionalRoutes,
        Self::VerifyOnlineRumor,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::ProtectNeighborhood => "protect their neighborhood",
            Self::SaveToMove => "save enough to move somewhere better",
            Self::ExposeRecordsLeak => "find out who leaked private records",
            Self::ImpressCityHall => "impress the right people at city hall",
            Self::BuryDamagingStory => "keep a damaging story buried",
            Self::RebuildFamilyBusiness => "rebuild a struggling family business",
            Self::MapRegionalRoutes => "track the fastest route between regional hubs",
            Self::VerifyOnlineRumor => "prove an online rumor is real",
        }
    }
}
