use std::assert_matches;
use std::collections::{HashMap, HashSet};

use rustc_hash::FxHashMap;
use smallvec::SmallVec;
use topo_sort::{SortResults, TopoSort};

use super::{
    DataType, ModuleId, ModuleLink, OUTPUT_MODULE_ID, RoutingMap,
    routing::{InputSlot, InputSlots, InputSource, MixedSlots},
};

pub(super) fn process_order(
    links: &[ModuleLink],
    all_modules: impl IntoIterator<Item = ModuleId>,
) -> Result<Vec<ModuleId>, String> {
    let mut dependents: HashMap<ModuleId, HashSet<ModuleId>> = HashMap::new();

    for id in all_modules {
        dependents.entry(id).or_default();
    }

    for link in links {
        let src_node = link.src();
        let dst_node = link.dst().module_id;

        // topo_sort ignores an edge from a node to itself, so reject it here.
        if src_node == dst_node {
            return Err("Cycles detected!".to_string());
        }

        dependents.entry(dst_node).or_default().insert(src_node);
        dependents.entry(src_node).or_default();

        if let Some(modulation) = link.modulation() {
            if modulation == dst_node {
                return Err("Cycles detected!".to_string());
            }

            dependents.entry(dst_node).or_default().insert(modulation);
            dependents.entry(modulation).or_default();
        }
    }

    let topo_sort = TopoSort::from_map(dependents);

    match topo_sort.into_vec_nodes() {
        SortResults::Full(mut nodes) => {
            if let Some(pos) = nodes.iter().position(|&id| id == OUTPUT_MODULE_ID) {
                nodes[pos..].rotate_left(1);
            }
            Ok(nodes)
        }
        SortResults::Partial(_) => Err("Cycles detected!".to_string()),
    }
}

pub(super) fn assign_slots(
    modules: &FxHashMap<ModuleId, (DataType, usize)>,
    input_sources: &RoutingMap,
) -> FxHashMap<ModuleId, Vec<InputSlots>> {
    let mut mapped: FxHashMap<_, _> = modules
        .keys()
        .map(|&mod_id| (mod_id, Vec::new()))
        .collect();

    for (input, sources) in input_sources {
        match sources {
            InputSource::Direct(module_id) => {
                let (src_data_type, src_output_slot) =
                    modules.get(module_id).copied().expect("should be in place");

                let dst_module = mapped
                    .get_mut(&input.module_id)
                    .expect("should be in place");

                if src_data_type == DataType::Spectral {
                    dst_module.push(InputSlots::Spectral {
                        input_type: input.input_type,
                        slot: src_output_slot,
                    });
                } else {
                    assert_matches!(src_data_type, DataType::Audio | DataType::Control);

                    dst_module.push(InputSlots::Direct {
                        input_type: input.input_type,
                        slot: src_output_slot,
                    });
                }
            }
            InputSource::Mixed(mixed) => {
                let mut slots = SmallVec::new();

                for src in mixed {
                    let (src_data_type, src_output_slot) = modules
                        .get(&src.module_id)
                        .copied()
                        .expect("should be in place");

                    assert_matches!(src_data_type, DataType::Audio | DataType::Control);

                    let modulation_slot = src.modulation.map(|modulation_src| {
                        let (modulation_data_type, modulation_output_slot) = modules
                            .get(&modulation_src)
                            .copied()
                            .expect("should be in place");

                        assert_matches!(modulation_data_type, DataType::Audio | DataType::Control);

                        modulation_output_slot
                    });

                    slots.push(InputSlot {
                        src_slot: src_output_slot,
                        modulation_slot,
                        amount: src.amount,
                    });
                }

                let dst_module = mapped
                    .get_mut(&input.module_id)
                    .expect("should be in place");

                dst_module.push(InputSlots::Mixed(MixedSlots {
                    input_type: input.input_type,
                    slots,
                }));
            }
        }
    }

    mapped
}
