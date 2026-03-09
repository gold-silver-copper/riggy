use rand::Rng;
use rand::prelude::IndexedRandom;
use rand_chacha::{ChaCha8Rng, rand_core::SeedableRng};
use serde::{Deserialize, Serialize};

pub type CityId = usize;
pub type NpcId = usize;
pub type RumorId = usize;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct World {
    pub seed: u64,
    pub cities: Vec<City>,
    pub npcs: Vec<Npc>,
    pub rumors: Vec<Rumor>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct City {
    pub id: CityId,
    pub name: String,
    pub biome: String,
    pub economy: String,
    pub culture: String,
    pub districts: Vec<District>,
    pub landmarks: Vec<String>,
    pub connected_city_ids: Vec<CityId>,
    pub npc_ids: Vec<NpcId>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct District {
    pub name: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Npc {
    pub id: NpcId,
    pub name: String,
    pub city_id: CityId,
    pub archetype: String,
    pub personality_traits: Vec<String>,
    pub goal: String,
    pub occupation: String,
    pub home_district: String,
    pub known_rumor_ids: Vec<RumorId>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Rumor {
    pub id: RumorId,
    pub city_id: CityId,
    pub source_npc_id: Option<NpcId>,
    pub text: String,
}

impl World {
    pub fn generate(seed: u64, city_count: usize) -> Self {
        let mut rng = ChaCha8Rng::seed_from_u64(seed);
        let target_cities = city_count.clamp(16, 24);

        let biomes = [
            "coastal", "forest", "desert", "river", "mountain", "marsh", "plains", "volcanic",
        ];
        let economies = [
            "trade",
            "fishing",
            "mining",
            "scholarship",
            "craftwork",
            "farming",
            "smuggling",
            "pilgrimage",
        ];
        let cultures = [
            "formal",
            "superstitious",
            "mercantile",
            "theatrical",
            "stoic",
            "devout",
            "scholarly",
            "festival-loving",
        ];
        let district_prefixes = [
            "Lantern", "North", "Market", "Copper", "Old", "Glass", "Lower", "Harbor", "River",
            "Temple",
        ];
        let district_suffixes = [
            "Ward", "Row", "Quarter", "Steps", "Circle", "Gate", "Hill", "Works",
        ];
        let landmark_kinds = [
            "bridge",
            "archive",
            "watchtower",
            "bazaar",
            "garden",
            "cathedral",
            "foundry",
            "amphitheater",
        ];
        let archetypes = [
            "gossip",
            "fixer",
            "scholar",
            "watcher",
            "merchant",
            "artisan",
            "drifter",
            "caretaker",
        ];
        let occupations = [
            "scribe",
            "dockworker",
            "innkeeper",
            "courier",
            "antiquarian",
            "guard",
            "brewer",
            "cartographer",
            "mason",
            "broker",
        ];
        let traits = [
            "warm",
            "suspicious",
            "dry-humored",
            "ambitious",
            "patient",
            "nervous",
            "idealistic",
            "cunning",
            "guarded",
            "generous",
        ];
        let goals = [
            "protect their neighborhood",
            "earn enough to leave the city",
            "find a missing ledger",
            "impress a local faction",
            "keep a dangerous secret buried",
            "rebuild a family business",
            "map hidden routes between cities",
            "prove an old rumor is true",
        ];
        let city_first = [
            "Ash", "Brae", "Cinder", "Dawn", "Elder", "Frost", "Glimmer", "High", "Iron",
            "Juniper", "Kings", "Low", "Moon", "North", "Oak", "Port", "Quartz", "Raven", "Stone",
            "Thorn", "Umber", "Vale", "West", "Yarrow",
        ];
        let city_second = [
            "haven", "ford", "mere", "crest", "point", "watch", "market", "cross", "fall", "reach",
            "gate", "harbor", "rest", "barrow", "field", "spire",
        ];
        let first_names = [
            "Ari", "Bryn", "Cato", "Dara", "Esme", "Finn", "Galen", "Hana", "Ivo", "Jora",
            "Kellan", "Lio", "Mara", "Niko", "Orin", "Pia", "Quin", "Rhea", "Soren", "Talia",
            "Una", "Vero", "Wren", "Yana",
        ];
        let last_names = [
            "Ashdown", "Briar", "Cask", "Dunfield", "Ember", "Farrow", "Gale", "Hearth", "Ives",
            "Jun", "Keene", "Lark", "Morrow", "Nettle", "Orchard", "Pell", "Quarry", "Reeve",
            "Sable", "Thorne", "Vale", "Wick",
        ];

        let mut cities = Vec::with_capacity(target_cities);
        for id in 0..target_cities {
            let name = format!(
                "{}{}",
                city_first[id % city_first.len()],
                city_second[rng.random_range(0..city_second.len())]
            );
            let mut districts = Vec::new();
            let district_count = rng.random_range(3..=4);
            for _ in 0..district_count {
                let district_name = format!(
                    "{} {}",
                    district_prefixes.choose(&mut rng).unwrap(),
                    district_suffixes.choose(&mut rng).unwrap()
                );
                districts.push(District {
                    name: district_name.clone(),
                    description: format!(
                        "{} is known for its {} mood and busy side streets.",
                        district_name,
                        traits.choose(&mut rng).unwrap()
                    ),
                });
            }

            let landmark_count = rng.random_range(2..=3);
            let mut landmarks = Vec::with_capacity(landmark_count);
            for _ in 0..landmark_count {
                landmarks.push(format!(
                    "the {} {}",
                    city_first.choose(&mut rng).unwrap().to_lowercase(),
                    landmark_kinds.choose(&mut rng).unwrap()
                ));
            }

            cities.push(City {
                id,
                name,
                biome: biomes.choose(&mut rng).unwrap().to_string(),
                economy: economies.choose(&mut rng).unwrap().to_string(),
                culture: cultures.choose(&mut rng).unwrap().to_string(),
                districts,
                landmarks,
                connected_city_ids: Vec::new(),
                npc_ids: Vec::new(),
            });
        }

        for city_id in 0..target_cities {
            let next = (city_id + 1) % target_cities;
            connect(&mut cities, city_id, next);
        }

        let extra_connections = target_cities / 2;
        for _ in 0..extra_connections {
            let a = rng.random_range(0..target_cities);
            let mut b = rng.random_range(0..target_cities);
            while b == a {
                b = rng.random_range(0..target_cities);
            }
            connect(&mut cities, a, b);
        }

        for city in &mut cities {
            city.connected_city_ids.sort_unstable();
            city.connected_city_ids.dedup();
        }

        let rumor_templates = [
            "A hidden ledger may be moving through {} under faction protection.",
            "Someone in {} swears an old tunnel still links the city to a neighboring capital.",
            "A broker in {} is quietly hiring outsiders for a dangerous favor.",
            "People say a landmark in {} opens only during storms.",
            "There is talk that a courier from {} vanished carrying names worth killing for.",
        ];

        let mut rumors = Vec::with_capacity(target_cities);
        for city in &cities {
            let template = rumor_templates.choose(&mut rng).unwrap();
            rumors.push(Rumor {
                id: rumors.len(),
                city_id: city.id,
                source_npc_id: None,
                text: template.replacen("{}", &city.name, 1),
            });
        }

        let mut npcs = Vec::new();
        for city in &mut cities {
            let npc_count = rng.random_range(3..=5);
            for _ in 0..npc_count {
                let name = format!(
                    "{} {}",
                    first_names.choose(&mut rng).unwrap(),
                    last_names.choose(&mut rng).unwrap()
                );
                let mut personality_traits = traits
                    .choose_multiple(&mut rng, 2)
                    .map(|value| (*value).to_string())
                    .collect::<Vec<_>>();
                personality_traits.sort();
                let home_district = city.districts.choose(&mut rng).unwrap().name.clone();
                let mut known_rumor_ids = vec![city.id];
                if rng.random_bool(0.35) {
                    known_rumor_ids.push(rng.random_range(0..rumors.len()));
                }
                known_rumor_ids.sort_unstable();
                known_rumor_ids.dedup();

                let npc = Npc {
                    id: npcs.len(),
                    name,
                    city_id: city.id,
                    archetype: archetypes.choose(&mut rng).unwrap().to_string(),
                    personality_traits,
                    goal: goals.choose(&mut rng).unwrap().to_string(),
                    occupation: occupations.choose(&mut rng).unwrap().to_string(),
                    home_district,
                    known_rumor_ids,
                };
                city.npc_ids.push(npc.id);
                npcs.push(npc);
            }
        }

        for rumor in &mut rumors {
            let city_npcs = &cities[rumor.city_id].npc_ids;
            rumor.source_npc_id = city_npcs.choose(&mut rng).copied();
        }

        Self {
            seed,
            cities,
            npcs,
            rumors,
        }
    }

    pub fn city(&self, id: CityId) -> &City {
        &self.cities[id]
    }

    pub fn npc(&self, id: NpcId) -> &Npc {
        &self.npcs[id]
    }

    pub fn rumor(&self, id: RumorId) -> &Rumor {
        &self.rumors[id]
    }
}

fn connect(cities: &mut [City], a: CityId, b: CityId) {
    if !cities[a].connected_city_ids.contains(&b) {
        cities[a].connected_city_ids.push(b);
    }
    if !cities[b].connected_city_ids.contains(&a) {
        cities[b].connected_city_ids.push(a);
    }
}

#[cfg(test)]
mod tests {
    use super::World;

    #[test]
    fn procgen_is_deterministic() {
        let a = World::generate(42, 18);
        let b = World::generate(42, 18);
        assert_eq!(a, b);
    }

    #[test]
    fn world_is_connected_and_in_bounds() {
        let world = World::generate(7, 24);
        assert_eq!(world.cities.len(), 24);

        let mut visited = std::collections::BTreeSet::new();
        let mut stack = vec![0usize];
        while let Some(city_id) = stack.pop() {
            if !visited.insert(city_id) {
                continue;
            }
            stack.extend(world.city(city_id).connected_city_ids.iter().copied());
        }

        assert_eq!(visited.len(), world.cities.len());
        assert!(world.npcs.len() >= 24 * 3);
    }
}
