# BFO 2020 to CCO Inheritance Map

This file maps explicit inheritance edges from the BFO 2020 OWL spec into the Common Core Ontologies (CCO) spec. It uses the non-merged CCO module and extension TTL files so each declaration is counted once.

## Sources

- `bfo/BFO-2020-master/21838-2/owl/bfo-core.ttl`
- `bfo/CommonCoreOntologies-develop/src/cco-modules/*.ttl`
- `bfo/CommonCoreOntologies-develop/src/cco-extensions/*.ttl`

## Notes

- `Additional CCO descendants` counts lower CCO terms reachable through same-kind inheritance beneath the direct CCO child.
- A small number of BFO labels used by CCO, such as `obo:BFO_0000144`, are redeclared in CCO module files; those labels are used here when the BFO core file does not carry them.

## Summary By BFO Class Root

| BFO class | Label | Direct CCO subclasses | Total CCO descendants |
| --- | --- | ---: | ---: |
| `obo:BFO_0000002` | continuant | 1 | 1 |
| `obo:BFO_0000016` | disposition | 5 | 46 |
| `obo:BFO_0000142` | fiat line | 1 | 8 |
| `obo:BFO_0000024` | fiat object part | 6 | 16 |
| `obo:BFO_0000147` | fiat point | 2 | 3 |
| `obo:BFO_0000146` | fiat surface | 1 | 1 |
| `obo:BFO_0000034` | function | 1 | 138 |
| `obo:BFO_0000031` | generically dependent continuant | 1 | 175 |
| `obo:BFO_0000182` | history | 1 | 1 |
| `obo:BFO_0000040` | material entity | 8 | 514 |
| `obo:BFO_0000030` | object | 2 | 11 |
| `obo:BFO_0000027` | object aggregate | 1 | 24 |
| `obo:BFO_0000026` | one-dimensional spatial region | 10 | 17 |
| `obo:BFO_0000038` | one-dimensional temporal region | 7 | 7 |
| `obo:BFO_0000015` | process | 7 | 244 |
| `obo:BFO_0000035` | process boundary | 2 | 2 |
| `obo:BFO_0000144` | Process Profile | 12 | 74 |
| `obo:BFO_0000019` | quality | 20 | 68 |
| `obo:BFO_0000017` | realizable entity | 4 | 7 |
| `obo:BFO_0000145` | relational quality | 2 | 6 |
| `obo:BFO_0000023` | role | 16 | 19 |
| `obo:BFO_0000029` | site | 2 | 21 |
| `obo:BFO_0000203` | temporal instant | 3 | 5 |
| `obo:BFO_0000202` | temporal interval | 13 | 21 |
| `obo:BFO_0000028` | three-dimensional spatial region | 2 | 2 |
| `obo:BFO_0000018` | zero-dimensional spatial region | 2 | 3 |

## Summary By BFO Relation Root

| BFO relation | Label | Direct CCO subproperties | Total CCO descendants |
| --- | --- | ---: | ---: |
| `obo:BFO_0000196` | bearer of | 1 | 1 |
| `obo:BFO_0000176` | continuant part of | 1 | 3 |
| `obo:BFO_0000183` | environs | 1 | 1 |
| `obo:BFO_0000084` | generically depends on | 3 | 4 |
| `obo:BFO_0000178` | has continuant part | 1 | 3 |
| `obo:BFO_0000117` | has occurrent part | 1 | 1 |
| `obo:BFO_0000057` | has participant | 9 | 9 |
| `obo:BFO_0000197` | inheres in | 1 | 1 |
| `obo:BFO_0000101` | is carrier of | 3 | 4 |
| `obo:BFO_0000132` | occurrent part of | 1 | 1 |
| `obo:BFO_0000066` | occurs in | 1 | 1 |
| `obo:BFO_0000056` | participates in | 9 | 9 |

## Direct CCO Class To BFO Class Links

| BFO class | BFO label | CCO class | CCO label | Source file | Additional CCO descendants |
| --- | --- | --- | --- | --- | ---: |
| `obo:BFO_0000002` | continuant | `cco:ont00000740` | Resource | `bfo/CommonCoreOntologies-develop/src/cco-modules/ArtifactOntology.ttl` | 0 |
| `obo:BFO_0000016` | disposition | `cco:ont00000318` | Disease | `bfo/CommonCoreOntologies-develop/src/cco-modules/AgentOntology.ttl` | 0 |
| `obo:BFO_0000016` | disposition | `cco:ont00000628` | Disposition to Interact with Electromagnetic Radiation | `bfo/CommonCoreOntologies-develop/src/cco-modules/QualityOntology.ttl` | 40 |
| `obo:BFO_0000016` | disposition | `cco:ont00000997` | Disrupting Disposition | `bfo/CommonCoreOntologies-develop/src/cco-modules/QualityOntology.ttl` | 1 |
| `obo:BFO_0000016` | disposition | `cco:ont00000632` | Magnetism | `bfo/CommonCoreOntologies-develop/src/cco-modules/QualityOntology.ttl` | 0 |
| `obo:BFO_0000016` | disposition | `cco:ont00001118` | Surface Tension | `bfo/CommonCoreOntologies-develop/src/cco-modules/QualityOntology.ttl` | 0 |
| `obo:BFO_0000142` | fiat line | `cco:ont00000207` | One-Dimensional Geospatial Boundary | `bfo/CommonCoreOntologies-develop/src/cco-modules/GeospatialOntology.ttl` | 7 |
| `obo:BFO_0000024` | fiat object part | `cco:ont00000084` | Bodily Component | `bfo/CommonCoreOntologies-develop/src/cco-modules/AgentOntology.ttl` | 8 |
| `obo:BFO_0000024` | fiat object part | `cco:ont00001259` | Portion of Atmosphere | `bfo/CommonCoreOntologies-develop/src/cco-modules/GeospatialOntology.ttl` | 0 |
| `obo:BFO_0000024` | fiat object part | `cco:ont00000779` | Portion of Cryosphere | `bfo/CommonCoreOntologies-develop/src/cco-modules/GeospatialOntology.ttl` | 0 |
| `obo:BFO_0000024` | fiat object part | `cco:ont00001341` | Portion of Geosphere | `bfo/CommonCoreOntologies-develop/src/cco-modules/GeospatialOntology.ttl` | 2 |
| `obo:BFO_0000024` | fiat object part | `cco:ont00000297` | Portion of Hydrosphere | `bfo/CommonCoreOntologies-develop/src/cco-modules/GeospatialOntology.ttl` | 0 |
| `obo:BFO_0000024` | fiat object part | `cco:ont00001016` | Portion of Lithosphere | `bfo/CommonCoreOntologies-develop/src/cco-modules/GeospatialOntology.ttl` | 0 |
| `obo:BFO_0000147` | fiat point | `cco:ont00002000` | Center of Mass | `bfo/CommonCoreOntologies-develop/src/cco-modules/GeospatialOntology.ttl` | 0 |
| `obo:BFO_0000147` | fiat point | `cco:ont00000373` | Geospatial Position | `bfo/CommonCoreOntologies-develop/src/cco-modules/GeospatialOntology.ttl` | 1 |
| `obo:BFO_0000146` | fiat surface | `cco:ont00000722` | Sea Level | `bfo/CommonCoreOntologies-develop/src/cco-modules/GeospatialOntology.ttl` | 0 |
| `obo:BFO_0000034` | function | `cco:ont00000323` | Artifact Function | `bfo/CommonCoreOntologies-develop/src/cco-modules/ArtifactOntology.ttl` | 137 |
| `obo:BFO_0000031` | generically dependent continuant | `cco:ont00000958` | Information Content Entity | `bfo/CommonCoreOntologies-develop/src/cco-extensions/ModalRelationOntology.ttl` | 174 |
| `obo:BFO_0000182` | history | `cco:ont00000856` | Artifact History | `bfo/CommonCoreOntologies-develop/src/cco-modules/ArtifactOntology.ttl` | 0 |
| `obo:BFO_0000040` | material entity | `cco:ont00001017` | Agent | `bfo/CommonCoreOntologies-develop/src/cco-extensions/ModalRelationOntology.ttl` | 0 |
| `obo:BFO_0000040` | material entity | `cco:ont00000574` | Environmental Feature | `bfo/CommonCoreOntologies-develop/src/cco-modules/GeospatialOntology.ttl` | 10 |
| `obo:BFO_0000040` | material entity | `cco:ont00000627` | Infrastructure Element | `bfo/CommonCoreOntologies-develop/src/cco-modules/ArtifactOntology.ttl` | 0 |
| `obo:BFO_0000040` | material entity | `cco:ont00000870` | Infrastructure System | `bfo/CommonCoreOntologies-develop/src/cco-modules/ArtifactOntology.ttl` | 2 |
| `obo:BFO_0000040` | material entity | `cco:ont00000995` | Material Artifact | `bfo/CommonCoreOntologies-develop/src/cco-modules/ArtifactOntology.ttl` | 494 |
| `obo:BFO_0000040` | material entity | `cco:ont00000556` | Payload | `bfo/CommonCoreOntologies-develop/src/cco-modules/ArtifactOntology.ttl` | 0 |
| `obo:BFO_0000040` | material entity | `cco:ont00001168` | Reaction Mass | `bfo/CommonCoreOntologies-develop/src/cco-modules/ArtifactOntology.ttl` | 0 |
| `obo:BFO_0000040` | material entity | `cco:ont00000544` | Target | `bfo/CommonCoreOntologies-develop/src/cco-modules/EventOntology.ttl` | 0 |
| `obo:BFO_0000030` | object | `cco:ont00000253` | Information Bearing Entity | `bfo/CommonCoreOntologies-develop/src/cco-extensions/ModalRelationOntology.ttl` | 0 |
| `obo:BFO_0000030` | object | `cco:ont00000551` | Organism | `bfo/CommonCoreOntologies-develop/src/cco-modules/AgentOntology.ttl` | 9 |
| `obo:BFO_0000027` | object aggregate | `cco:ont00000300` | Group of Agents | `bfo/CommonCoreOntologies-develop/src/cco-extensions/ModalRelationOntology.ttl` | 23 |
| `obo:BFO_0000026` | one-dimensional spatial region | `cco:ont00000387` | Axis of Rotation | `bfo/CommonCoreOntologies-develop/src/cco-modules/GeospatialOntology.ttl` | 3 |
| `obo:BFO_0000026` | one-dimensional spatial region | `cco:ont00000161` | Coordinate System Axis | `bfo/CommonCoreOntologies-develop/src/cco-modules/GeospatialOntology.ttl` | 3 |
| `obo:BFO_0000026` | one-dimensional spatial region | `cco:ont00000188` | Ground Track | `bfo/CommonCoreOntologies-develop/src/cco-modules/GeospatialOntology.ttl` | 0 |
| `obo:BFO_0000026` | one-dimensional spatial region | `cco:ont00000218` | Major Axis | `bfo/CommonCoreOntologies-develop/src/cco-modules/GeospatialOntology.ttl` | 0 |
| `obo:BFO_0000026` | one-dimensional spatial region | `cco:ont00000017` | Minor Axis | `bfo/CommonCoreOntologies-develop/src/cco-modules/GeospatialOntology.ttl` | 0 |
| `obo:BFO_0000026` | one-dimensional spatial region | `cco:ont00001040` | Nadir | `bfo/CommonCoreOntologies-develop/src/cco-modules/GeospatialOntology.ttl` | 0 |
| `obo:BFO_0000026` | one-dimensional spatial region | `cco:ont00000205` | Object Track | `bfo/CommonCoreOntologies-develop/src/cco-modules/GeospatialOntology.ttl` | 1 |
| `obo:BFO_0000026` | one-dimensional spatial region | `cco:ont00000911` | Semi-Major Axis | `bfo/CommonCoreOntologies-develop/src/cco-modules/GeospatialOntology.ttl` | 0 |
| `obo:BFO_0000026` | one-dimensional spatial region | `cco:ont00000221` | Semi-Minor Axis | `bfo/CommonCoreOntologies-develop/src/cco-modules/GeospatialOntology.ttl` | 0 |
| `obo:BFO_0000026` | one-dimensional spatial region | `cco:ont00000755` | Zenith | `bfo/CommonCoreOntologies-develop/src/cco-modules/GeospatialOntology.ttl` | 0 |
| `obo:BFO_0000038` | one-dimensional temporal region | `cco:ont00000211` | Multi-Day Temporal Interval | `bfo/CommonCoreOntologies-develop/src/cco-modules/TimeOntology.ttl` | 0 |
| `obo:BFO_0000038` | one-dimensional temporal region | `cco:ont00000063` | Multi-Hour Temporal Interval | `bfo/CommonCoreOntologies-develop/src/cco-modules/TimeOntology.ttl` | 0 |
| `obo:BFO_0000038` | one-dimensional temporal region | `cco:ont00001166` | Multi-Minute Temporal Interval | `bfo/CommonCoreOntologies-develop/src/cco-modules/TimeOntology.ttl` | 0 |
| `obo:BFO_0000038` | one-dimensional temporal region | `cco:ont00000329` | Multi-Month Temporal Interval | `bfo/CommonCoreOntologies-develop/src/cco-modules/TimeOntology.ttl` | 0 |
| `obo:BFO_0000038` | one-dimensional temporal region | `cco:ont00001154` | Multi-Second Temporal Interval | `bfo/CommonCoreOntologies-develop/src/cco-modules/TimeOntology.ttl` | 0 |
| `obo:BFO_0000038` | one-dimensional temporal region | `cco:ont00000810` | Multi-Week Temporal Interval | `bfo/CommonCoreOntologies-develop/src/cco-modules/TimeOntology.ttl` | 0 |
| `obo:BFO_0000038` | one-dimensional temporal region | `cco:ont00001206` | Multi-Year Temporal Interval | `bfo/CommonCoreOntologies-develop/src/cco-modules/TimeOntology.ttl` | 0 |
| `obo:BFO_0000015` | process | `cco:ont00000005` | Act | `bfo/CommonCoreOntologies-develop/src/cco-modules/EventOntology.ttl` | 148 |
| `obo:BFO_0000015` | process | `cco:ont00000978` | Cause | `bfo/CommonCoreOntologies-develop/src/cco-modules/EventOntology.ttl` | 0 |
| `obo:BFO_0000015` | process | `cco:ont00000004` | Change | `bfo/CommonCoreOntologies-develop/src/cco-modules/EventOntology.ttl` | 33 |
| `obo:BFO_0000015` | process | `cco:ont00000660` | Effect | `bfo/CommonCoreOntologies-develop/src/cco-modules/EventOntology.ttl` | 1 |
| `obo:BFO_0000015` | process | `cco:ont00000110` | Mechanical Process | `bfo/CommonCoreOntologies-develop/src/cco-modules/EventOntology.ttl` | 1 |
| `obo:BFO_0000015` | process | `cco:ont00000007` | Natural Process | `bfo/CommonCoreOntologies-develop/src/cco-modules/EventOntology.ttl` | 27 |
| `obo:BFO_0000015` | process | `cco:ont00000819` | Stasis | `bfo/CommonCoreOntologies-develop/src/cco-modules/EventOntology.ttl` | 27 |
| `obo:BFO_0000035` | process boundary | `cco:ont00000197` | Process Beginning | `bfo/CommonCoreOntologies-develop/src/cco-modules/EventOntology.ttl` | 0 |
| `obo:BFO_0000035` | process boundary | `cco:ont00000083` | Process Ending | `bfo/CommonCoreOntologies-develop/src/cco-modules/EventOntology.ttl` | 0 |
| `obo:BFO_0000144` | Process Profile | `cco:ont00000712` | Acceleration | `bfo/CommonCoreOntologies-develop/src/cco-modules/EventOntology.ttl` | 1 |
| `obo:BFO_0000144` | Process Profile | `cco:ont00000910` | Amplitude | `bfo/CommonCoreOntologies-develop/src/cco-modules/EventOntology.ttl` | 0 |
| `obo:BFO_0000144` | Process Profile | `cco:ont00001219` | Delta-v | `bfo/CommonCoreOntologies-develop/src/cco-modules/EventOntology.ttl` | 0 |
| `obo:BFO_0000144` | Process Profile | `cco:ont00000570` | Force | `bfo/CommonCoreOntologies-develop/src/cco-modules/EventOntology.ttl` | 4 |
| `obo:BFO_0000144` | Process Profile | `cco:ont00001047` | Frequency | `bfo/CommonCoreOntologies-develop/src/cco-modules/EventOntology.ttl` | 40 |
| `obo:BFO_0000144` | Process Profile | `cco:ont00000772` | Impulsive Force | `bfo/CommonCoreOntologies-develop/src/cco-modules/EventOntology.ttl` | 0 |
| `obo:BFO_0000144` | Process Profile | `cco:ont00000278` | Momentum | `bfo/CommonCoreOntologies-develop/src/cco-modules/EventOntology.ttl` | 1 |
| `obo:BFO_0000144` | Process Profile | `cco:ont00000503` | Power | `bfo/CommonCoreOntologies-develop/src/cco-modules/EventOntology.ttl` | 1 |
| `obo:BFO_0000144` | Process Profile | `cco:ont00000862` | Sound Process Profile | `bfo/CommonCoreOntologies-develop/src/cco-modules/EventOntology.ttl` | 3 |
| `obo:BFO_0000144` | Process Profile | `cco:ont00000830` | Speed | `bfo/CommonCoreOntologies-develop/src/cco-modules/EventOntology.ttl` | 0 |
| `obo:BFO_0000144` | Process Profile | `cco:ont00000763` | Velocity | `bfo/CommonCoreOntologies-develop/src/cco-modules/EventOntology.ttl` | 1 |
| `obo:BFO_0000144` | Process Profile | `cco:ont00000752` | Wave Process Profile | `bfo/CommonCoreOntologies-develop/src/cco-modules/EventOntology.ttl` | 11 |
| `obo:BFO_0000019` | quality | `cco:ont00000768` | Amount | `bfo/CommonCoreOntologies-develop/src/cco-modules/QualityOntology.ttl` | 0 |
| `obo:BFO_0000019` | quality | `cco:ont00001033` | Biological Sex | `bfo/CommonCoreOntologies-develop/src/cco-modules/AgentOntology.ttl` | 2 |
| `obo:BFO_0000019` | quality | `cco:ont00000979` | Closure | `bfo/CommonCoreOntologies-develop/src/cco-modules/QualityOntology.ttl` | 0 |
| `obo:BFO_0000019` | quality | `cco:ont00000377` | Disability | `bfo/CommonCoreOntologies-develop/src/cco-modules/AgentOntology.ttl` | 0 |
| `obo:BFO_0000019` | quality | `cco:ont00000780` | Ethnicity | `bfo/CommonCoreOntologies-develop/src/cco-modules/AgentOntology.ttl` | 0 |
| `obo:BFO_0000019` | quality | `cco:ont00000044` | Eye Color | `bfo/CommonCoreOntologies-develop/src/cco-modules/AgentOntology.ttl` | 0 |
| `obo:BFO_0000019` | quality | `cco:ont00000608` | Financial Value | `bfo/CommonCoreOntologies-develop/src/cco-modules/AgentOntology.ttl` | 1 |
| `obo:BFO_0000019` | quality | `cco:ont00000026` | Hair Color | `bfo/CommonCoreOntologies-develop/src/cco-modules/AgentOntology.ttl` | 0 |
| `obo:BFO_0000019` | quality | `cco:ont00000766` | Hardness | `bfo/CommonCoreOntologies-develop/src/cco-modules/QualityOntology.ttl` | 0 |
| `obo:BFO_0000019` | quality | `cco:ont00000314` | Information Quality Entity | `bfo/CommonCoreOntologies-develop/src/cco-modules/InformationEntityOntology.ttl` | 0 |
| `obo:BFO_0000019` | quality | `cco:ont00000614` | Mass | `bfo/CommonCoreOntologies-develop/src/cco-modules/QualityOntology.ttl` | 0 |
| `obo:BFO_0000019` | quality | `cco:ont00000009` | Mass Density | `bfo/CommonCoreOntologies-develop/src/cco-modules/QualityOntology.ttl` | 0 |
| `obo:BFO_0000019` | quality | `cco:ont00000442` | Radioactive | `bfo/CommonCoreOntologies-develop/src/cco-modules/QualityOntology.ttl` | 0 |
| `obo:BFO_0000019` | quality | `cco:ont00001059` | Shape Quality | `bfo/CommonCoreOntologies-develop/src/cco-modules/QualityOntology.ttl` | 33 |
| `obo:BFO_0000019` | quality | `cco:ont00001202` | Size Quality | `bfo/CommonCoreOntologies-develop/src/cco-modules/QualityOntology.ttl` | 12 |
| `obo:BFO_0000019` | quality | `cco:ont00000102` | Skin Type | `bfo/CommonCoreOntologies-develop/src/cco-modules/AgentOntology.ttl` | 0 |
| `obo:BFO_0000019` | quality | `cco:ont00000441` | Temperature | `bfo/CommonCoreOntologies-develop/src/cco-modules/QualityOntology.ttl` | 0 |
| `obo:BFO_0000019` | quality | `cco:ont00000327` | Texture | `bfo/CommonCoreOntologies-develop/src/cco-modules/QualityOntology.ttl` | 0 |
| `obo:BFO_0000019` | quality | `cco:ont00000633` | Weight | `bfo/CommonCoreOntologies-develop/src/cco-modules/QualityOntology.ttl` | 0 |
| `obo:BFO_0000019` | quality | `cco:ont00000295` | Wetness | `bfo/CommonCoreOntologies-develop/src/cco-modules/QualityOntology.ttl` | 0 |
| `obo:BFO_0000017` | realizable entity | `cco:ont00000177` | Affordance | `bfo/CommonCoreOntologies-develop/src/cco-modules/AgentOntology.ttl` | 0 |
| `obo:BFO_0000017` | realizable entity | `cco:ont00001379` | Agent Capability | `bfo/CommonCoreOntologies-develop/src/cco-extensions/ModalRelationOntology.ttl` | 3 |
| `obo:BFO_0000017` | realizable entity | `cco:ont00001193` | Fatigability | `bfo/CommonCoreOntologies-develop/src/cco-modules/QualityOntology.ttl` | 0 |
| `obo:BFO_0000017` | realizable entity | `cco:ont00000284` | Strength | `bfo/CommonCoreOntologies-develop/src/cco-modules/QualityOntology.ttl` | 0 |
| `obo:BFO_0000145` | relational quality | `cco:ont00001182` | Phase Angle | `bfo/CommonCoreOntologies-develop/src/cco-modules/QualityOntology.ttl` | 0 |
| `obo:BFO_0000145` | relational quality | `cco:ont00000119` | Spatial Orientation | `bfo/CommonCoreOntologies-develop/src/cco-modules/QualityOntology.ttl` | 4 |
| `obo:BFO_0000023` | role | `cco:ont00000392` | Allegiance Role | `bfo/CommonCoreOntologies-develop/src/cco-modules/AgentOntology.ttl` | 3 |
| `obo:BFO_0000023` | role | `cco:ont00000187` | Authority Role | `bfo/CommonCoreOntologies-develop/src/cco-modules/AgentOntology.ttl` | 0 |
| `obo:BFO_0000023` | role | `cco:ont00000987` | Citizen Role | `bfo/CommonCoreOntologies-develop/src/cco-modules/AgentOntology.ttl` | 0 |
| `obo:BFO_0000023` | role | `cco:ont00000173` | Civilian Role | `bfo/CommonCoreOntologies-develop/src/cco-modules/AgentOntology.ttl` | 0 |
| `obo:BFO_0000023` | role | `cco:ont00000485` | Commercial Role | `bfo/CommonCoreOntologies-develop/src/cco-modules/AgentOntology.ttl` | 0 |
| `obo:BFO_0000023` | role | `cco:ont00000758` | Component Role | `bfo/CommonCoreOntologies-develop/src/cco-modules/ArtifactOntology.ttl` | 0 |
| `obo:BFO_0000023` | role | `cco:ont00000506` | Contractor Role | `bfo/CommonCoreOntologies-develop/src/cco-modules/AgentOntology.ttl` | 0 |
| `obo:BFO_0000023` | role | `cco:ont00000898` | Geopolitical Power Role | `bfo/CommonCoreOntologies-develop/src/cco-modules/AgentOntology.ttl` | 0 |
| `obo:BFO_0000023` | role | `cco:ont00001141` | Infrastructure Role | `bfo/CommonCoreOntologies-develop/src/cco-modules/ArtifactOntology.ttl` | 0 |
| `obo:BFO_0000023` | role | `cco:ont00000599` | Interpersonal Relationship Role | `bfo/CommonCoreOntologies-develop/src/cco-modules/AgentOntology.ttl` | 0 |
| `obo:BFO_0000023` | role | `cco:ont00000984` | Occupation Role | `bfo/CommonCoreOntologies-develop/src/cco-modules/AgentOntology.ttl` | 0 |
| `obo:BFO_0000023` | role | `cco:ont00001006` | Operator Role | `bfo/CommonCoreOntologies-develop/src/cco-modules/AgentOntology.ttl` | 0 |
| `obo:BFO_0000023` | role | `cco:ont00000175` | Organization Member Role | `bfo/CommonCoreOntologies-develop/src/cco-modules/AgentOntology.ttl` | 0 |
| `obo:BFO_0000023` | role | `cco:ont00000929` | Part Role | `bfo/CommonCoreOntologies-develop/src/cco-modules/ArtifactOntology.ttl` | 0 |
| `obo:BFO_0000023` | role | `cco:ont00000917` | Permanent Resident Role | `bfo/CommonCoreOntologies-develop/src/cco-modules/AgentOntology.ttl` | 0 |
| `obo:BFO_0000023` | role | `cco:ont00000038` | System Role | `bfo/CommonCoreOntologies-develop/src/cco-modules/ArtifactOntology.ttl` | 0 |
| `obo:BFO_0000029` | site | `cco:ont00000591` | Artifact Location | `bfo/CommonCoreOntologies-develop/src/cco-modules/ArtifactOntology.ttl` | 0 |
| `obo:BFO_0000029` | site | `cco:ont00000472` | Geospatial Region | `bfo/CommonCoreOntologies-develop/src/cco-modules/GeospatialOntology.ttl` | 19 |
| `obo:BFO_0000203` | temporal instant | `cco:ont00001116` | Reference Time | `bfo/CommonCoreOntologies-develop/src/cco-modules/TimeOntology.ttl` | 0 |
| `obo:BFO_0000203` | temporal instant | `cco:ont00000223` | Time of Day | `bfo/CommonCoreOntologies-develop/src/cco-modules/TimeOntology.ttl` | 2 |
| `obo:BFO_0000203` | temporal instant | `cco:ont00000114` | Unix Temporal Instant | `bfo/CommonCoreOntologies-develop/src/cco-modules/TimeOntology.ttl` | 0 |
| `obo:BFO_0000202` | temporal interval | `cco:ont00000699` | Afternoon | `bfo/CommonCoreOntologies-develop/src/cco-modules/TimeOntology.ttl` | 0 |
| `obo:BFO_0000202` | temporal interval | `cco:ont00000184` | Axial Rotation Period | `bfo/CommonCoreOntologies-develop/src/cco-modules/TimeOntology.ttl` | 0 |
| `obo:BFO_0000202` | temporal interval | `cco:ont00000800` | Day | `bfo/CommonCoreOntologies-develop/src/cco-modules/TimeOntology.ttl` | 3 |
| `obo:BFO_0000202` | temporal interval | `cco:ont00001088` | Decade | `bfo/CommonCoreOntologies-develop/src/cco-modules/TimeOntology.ttl` | 0 |
| `obo:BFO_0000202` | temporal interval | `cco:ont00001110` | Evening | `bfo/CommonCoreOntologies-develop/src/cco-modules/TimeOntology.ttl` | 0 |
| `obo:BFO_0000202` | temporal interval | `cco:ont00001058` | Hour | `bfo/CommonCoreOntologies-develop/src/cco-modules/TimeOntology.ttl` | 0 |
| `obo:BFO_0000202` | temporal interval | `cco:ont00000085` | Minute | `bfo/CommonCoreOntologies-develop/src/cco-modules/TimeOntology.ttl` | 0 |
| `obo:BFO_0000202` | temporal interval | `cco:ont00000225` | Month | `bfo/CommonCoreOntologies-develop/src/cco-modules/TimeOntology.ttl` | 1 |
| `obo:BFO_0000202` | temporal interval | `cco:ont00000550` | Morning | `bfo/CommonCoreOntologies-develop/src/cco-modules/TimeOntology.ttl` | 0 |
| `obo:BFO_0000202` | temporal interval | `cco:ont00001204` | Night | `bfo/CommonCoreOntologies-develop/src/cco-modules/TimeOntology.ttl` | 0 |
| `obo:BFO_0000202` | temporal interval | `cco:ont00000992` | Second | `bfo/CommonCoreOntologies-develop/src/cco-modules/TimeOntology.ttl` | 0 |
| `obo:BFO_0000202` | temporal interval | `cco:ont00000619` | Week | `bfo/CommonCoreOntologies-develop/src/cco-modules/TimeOntology.ttl` | 1 |
| `obo:BFO_0000202` | temporal interval | `cco:ont00000832` | Year | `bfo/CommonCoreOntologies-develop/src/cco-modules/TimeOntology.ttl` | 3 |
| `obo:BFO_0000028` | three-dimensional spatial region | `cco:ont00000068` | Three-Dimensional Path | `bfo/CommonCoreOntologies-develop/src/cco-modules/GeospatialOntology.ttl` | 0 |
| `obo:BFO_0000028` | three-dimensional spatial region | `cco:ont00001348` | Three-Dimensional Position | `bfo/CommonCoreOntologies-develop/src/cco-modules/GeospatialOntology.ttl` | 0 |
| `obo:BFO_0000018` | zero-dimensional spatial region | `cco:ont00000070` | Ground Track Point | `bfo/CommonCoreOntologies-develop/src/cco-modules/GeospatialOntology.ttl` | 0 |
| `obo:BFO_0000018` | zero-dimensional spatial region | `cco:ont00000170` | Object Track Point | `bfo/CommonCoreOntologies-develop/src/cco-modules/GeospatialOntology.ttl` | 1 |

## Direct CCO Relation To BFO Relation Links

| BFO relation | BFO label | CCO relation | CCO label | Source file | Additional CCO descendants |
| --- | --- | --- | --- | --- | ---: |
| `obo:BFO_0000196` | bearer of | `cco:ont00001954` | has capability | `bfo/CommonCoreOntologies-develop/src/cco-modules/AgentOntology.ttl` | 0 |
| `obo:BFO_0000176` | continuant part of | `cco:ont00001944` | spatial part of | `bfo/CommonCoreOntologies-develop/src/cco-modules/GeospatialOntology.ttl` | 2 |
| `obo:BFO_0000183` | environs | `cco:ont00001845` | is site of | `bfo/CommonCoreOntologies-develop/src/cco-modules/ExtendedRelationOntology.ttl` | 0 |
| `obo:BFO_0000084` | generically depends on | `cco:ont00001961` | is measurement unit of | `bfo/CommonCoreOntologies-develop/src/cco-modules/InformationEntityOntology.ttl` | 0 |
| `obo:BFO_0000084` | generically depends on | `cco:ont00001997` | is reference system of | `bfo/CommonCoreOntologies-develop/src/cco-modules/InformationEntityOntology.ttl` | 1 |
| `obo:BFO_0000084` | generically depends on | `cco:ont00001837` | time zone identifier used by | `bfo/CommonCoreOntologies-develop/src/cco-modules/InformationEntityOntology.ttl` | 0 |
| `obo:BFO_0000178` | has continuant part | `cco:ont00001855` | has spatial part | `bfo/CommonCoreOntologies-develop/src/cco-modules/GeospatialOntology.ttl` | 2 |
| `obo:BFO_0000117` | has occurrent part | `cco:ont00001777` | has process part | `bfo/CommonCoreOntologies-develop/src/cco-modules/ExtendedRelationOntology.ttl` | 0 |
| `obo:BFO_0000057` | has participant | `cco:ont00001834` | affects | `bfo/CommonCoreOntologies-develop/src/cco-modules/ExtendedRelationOntology.ttl` | 0 |
| `obo:BFO_0000057` | has participant | `cco:ont00001949` | has accessory | `bfo/CommonCoreOntologies-develop/src/cco-modules/AgentOntology.ttl` | 0 |
| `obo:BFO_0000057` | has participant | `cco:ont00001830` | has accomplice | `bfo/CommonCoreOntologies-develop/src/cco-modules/AgentOntology.ttl` | 0 |
| `obo:BFO_0000057` | has participant | `cco:ont00001833` | has agent | `bfo/CommonCoreOntologies-develop/src/cco-modules/AgentOntology.ttl` | 0 |
| `obo:BFO_0000057` | has participant | `cco:ont00001921` | has input | `bfo/CommonCoreOntologies-develop/src/cco-modules/ExtendedRelationOntology.ttl` | 0 |
| `obo:BFO_0000057` | has participant | `cco:ont00001778` | has object | `bfo/CommonCoreOntologies-develop/src/cco-modules/ExtendedRelationOntology.ttl` | 0 |
| `obo:BFO_0000057` | has participant | `cco:ont00001986` | has output | `bfo/CommonCoreOntologies-develop/src/cco-modules/ExtendedRelationOntology.ttl` | 0 |
| `obo:BFO_0000057` | has participant | `cco:ont00001922` | has recipient | `bfo/CommonCoreOntologies-develop/src/cco-modules/AgentOntology.ttl` | 0 |
| `obo:BFO_0000057` | has participant | `cco:ont00001844` | has sender | `bfo/CommonCoreOntologies-develop/src/cco-modules/AgentOntology.ttl` | 0 |
| `obo:BFO_0000197` | inheres in | `cco:ont00001889` | capability of | `bfo/CommonCoreOntologies-develop/src/cco-modules/AgentOntology.ttl` | 0 |
| `obo:BFO_0000101` | is carrier of | `cco:ont00001863` | uses measurement unit | `bfo/CommonCoreOntologies-develop/src/cco-modules/InformationEntityOntology.ttl` | 0 |
| `obo:BFO_0000101` | is carrier of | `cco:ont00001912` | uses reference system | `bfo/CommonCoreOntologies-develop/src/cco-modules/InformationEntityOntology.ttl` | 1 |
| `obo:BFO_0000101` | is carrier of | `cco:ont00001908` | uses time zone identifier | `bfo/CommonCoreOntologies-develop/src/cco-modules/InformationEntityOntology.ttl` | 0 |
| `obo:BFO_0000132` | occurrent part of | `cco:ont00001857` | is part of process | `bfo/CommonCoreOntologies-develop/src/cco-modules/ExtendedRelationOntology.ttl` | 0 |
| `obo:BFO_0000066` | occurs in | `cco:ont00001918` | occurs at | `bfo/CommonCoreOntologies-develop/src/cco-modules/ExtendedRelationOntology.ttl` | 0 |
| `obo:BFO_0000056` | participates in | `cco:ont00001852` | accessory in | `bfo/CommonCoreOntologies-develop/src/cco-modules/AgentOntology.ttl` | 0 |
| `obo:BFO_0000056` | participates in | `cco:ont00001895` | accomplice in | `bfo/CommonCoreOntologies-develop/src/cco-modules/AgentOntology.ttl` | 0 |
| `obo:BFO_0000056` | participates in | `cco:ont00001787` | agent in | `bfo/CommonCoreOntologies-develop/src/cco-modules/AgentOntology.ttl` | 0 |
| `obo:BFO_0000056` | participates in | `cco:ont00001886` | is affected by | `bfo/CommonCoreOntologies-develop/src/cco-modules/ExtendedRelationOntology.ttl` | 0 |
| `obo:BFO_0000056` | participates in | `cco:ont00001841` | is input of | `bfo/CommonCoreOntologies-develop/src/cco-modules/ExtendedRelationOntology.ttl` | 0 |
| `obo:BFO_0000056` | participates in | `cco:ont00001936` | is object of | `bfo/CommonCoreOntologies-develop/src/cco-modules/ExtendedRelationOntology.ttl` | 0 |
| `obo:BFO_0000056` | participates in | `cco:ont00001816` | is output of | `bfo/CommonCoreOntologies-develop/src/cco-modules/ExtendedRelationOntology.ttl` | 0 |
| `obo:BFO_0000056` | participates in | `cco:ont00001978` | receives | `bfo/CommonCoreOntologies-develop/src/cco-modules/AgentOntology.ttl` | 0 |
| `obo:BFO_0000056` | participates in | `cco:ont00001993` | sends | `bfo/CommonCoreOntologies-develop/src/cco-modules/AgentOntology.ttl` | 0 |
