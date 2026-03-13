use serde::{Deserialize, Serialize};

macro_rules! labeled_enum {
    ($name:ident { $($variant:ident => $label:literal),+ $(,)? }) => {
        #[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub enum $name {
            $($variant),+
        }

        impl $name {
            pub const ALL: &[Self] = &[
                $(Self::$variant),+
            ];

            pub const fn label(self) -> &'static str {
                match self {
                    $(Self::$variant => $label),+
                }
            }
        }
    };
}

labeled_enum!(Biome {
    Coastal => "coastal",
    Suburban => "suburban",
    Desert => "desert",
    Riverfront => "riverfront",
    Mountain => "mountain",
    Wetland => "wetland",
    Plains => "plains",
    Industrial => "industrial",
});

labeled_enum!(Economy {
    Trade => "trade",
    Logistics => "logistics",
    Manufacturing => "manufacturing",
    Tech => "tech",
    Tourism => "tourism",
    Healthcare => "healthcare",
    Finance => "finance",
    Shipping => "shipping",
});

labeled_enum!(Culture {
    Formal => "formal",
    StartupDriven => "startup-driven",
    LaidBack => "laid-back",
    StatusConscious => "status-conscious",
    Stoic => "stoic",
    CivicMinded => "civic-minded",
    ArtForward => "art-forward",
    NightlifeHeavy => "nightlife-heavy",
});

labeled_enum!(NpcArchetype {
    Gossip => "gossip",
    Fixer => "fixer",
    Scholar => "scholar",
    Watcher => "watcher",
    Networker => "networker",
    Creative => "creative",
    Contractor => "contractor",
    Organizer => "organizer",
});

labeled_enum!(Occupation {
    SoftwareEngineer => "software engineer",
    Barista => "barista",
    DeliveryDriver => "delivery driver",
    Journalist => "journalist",
    CityPlanner => "city planner",
    SecurityGuard => "security guard",
    BreweryManager => "brewery manager",
    RideshareDriver => "rideshare driver",
    RealEstateAgent => "real estate agent",
    Teacher => "teacher",
});

labeled_enum!(TraitTag {
    Warm => "warm",
    Suspicious => "suspicious",
    DryHumored => "dry-humored",
    Ambitious => "ambitious",
    Patient => "patient",
    Nervous => "nervous",
    Idealistic => "idealistic",
    Cunning => "cunning",
    Guarded => "guarded",
    Generous => "generous",
});

labeled_enum!(GoalTag {
    ProtectNeighborhood => "protect their neighborhood",
    SaveToMove => "save enough to move somewhere better",
    ExposeRecordsLeak => "find out who leaked private records",
    ImpressCityHall => "impress the right people at city hall",
    BuryDamagingStory => "keep a damaging story buried",
    RebuildFamilyBusiness => "rebuild a struggling family business",
    MapRegionalRoutes => "track the fastest route between regional hubs",
    VerifyOnlineRumor => "prove an online rumor is real",
});
