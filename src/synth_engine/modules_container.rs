use std::{collections::HashMap, sync::Arc};

use crate::synth_engine::{
    Amplifier, Config, Envelope, Oscillator, SpectralFilter, modules,
    routing::{ModuleId, RoutingNode},
    synth_module::{
        BufferOutputModule, ModuleConfig, ScalarOutputModule, SpectralOutputModule, SynthModule,
    },
};

type Container<T> = HashMap<ModuleId, Option<Box<T>>>;

pub(super) struct TypedContainer<T: SynthModule> {
    pub modules: Container<T>,
}

impl<T: SynthModule> TypedContainer<T> {
    pub fn new() -> Self {
        Self {
            modules: HashMap::new(),
        }
    }

    pub fn add(&mut self, module: T) {
        self.modules.insert(module.get_id(), Some(Box::new(module)));
    }

    pub fn has(&self, id: ModuleId) -> bool {
        self.modules.contains_key(&id)
    }

    #[inline]
    pub fn get(&self, id: ModuleId) -> Option<&T> {
        self.modules.get(&id)?.as_deref()
    }

    #[inline]
    pub fn get_mut(&mut self, id: ModuleId) -> Option<&mut T> {
        self.modules.get_mut(&id)?.as_deref_mut()
    }

    #[inline]
    pub fn take(&mut self, id: ModuleId) -> Box<T> {
        self.modules.get_mut(&id).unwrap().take().unwrap()
    }

    #[inline]
    pub fn return_back(&mut self, module: Box<T>) {
        self.modules
            .get_mut(&module.get_id())
            .unwrap()
            .replace(module);
    }
}

pub(super) struct ModulesContainer {
    pub oscillators: TypedContainer<modules::Oscillator>,
    pub envelopes: TypedContainer<modules::Envelope>,
    pub amplifiers: TypedContainer<modules::Amplifier>,
    pub spectral_filters: TypedContainer<modules::SpectralFilter>,
}

impl ModulesContainer {
    pub(super) fn new() -> Self {
        Self {
            oscillators: TypedContainer::new(),
            envelopes: TypedContainer::new(),
            amplifiers: TypedContainer::new(),
            spectral_filters: TypedContainer::new(),
        }
    }

    pub(super) fn add_node(
        &mut self,
        node: &RoutingNode,
        config: &Config,
    ) -> Result<&mut dyn SynthModule, String> {
        macro_rules! insert_node {
            ($id:ident, $container:ident, $struct_name:ident) => {{
                if !self.$container.has(*$id) {
                    self.$container.add($struct_name::new(ModuleConfig::new(
                        *$id,
                        Arc::clone(&config.$container),
                    )));
                    Ok(self.$container.get_mut(*$id).unwrap() as &mut dyn SynthModule)
                } else {
                    Err("Module id exists".to_string())
                }
            }};
        }

        match node {
            RoutingNode::Amplifier(id) => insert_node!(id, amplifiers, Amplifier),
            RoutingNode::Envelope(id) => insert_node!(id, envelopes, Envelope),
            RoutingNode::Oscillator(id) => insert_node!(id, oscillators, Oscillator),
            RoutingNode::SpectralFilter(id) => insert_node!(id, spectral_filters, SpectralFilter),
            RoutingNode::Output => Err("Can't add output module".to_string()),
        }
    }

    pub(super) fn clear(&mut self) {
        self.oscillators.modules.clear();
        self.envelopes.modules.clear();
        self.amplifiers.modules.clear();
        self.spectral_filters.modules.clear();
    }

    pub(super) fn resolve_buffer_output_node(
        &self,
        node: RoutingNode,
    ) -> Option<&dyn BufferOutputModule> {
        macro_rules! get_node {
            ($id:ident, $container:ident) => {
                self.$container
                    .get($id)
                    .map(|module| module as &dyn BufferOutputModule)
            };
        }

        macro_rules! get_node_error {
            ($node_type:ident) => {
                panic!(concat!(
                    stringify!($node_type),
                    " don't have buffer output."
                ))
            };
        }

        match node {
            RoutingNode::Oscillator(id) => get_node!(id, oscillators),
            RoutingNode::Envelope(id) => get_node!(id, envelopes),
            RoutingNode::Amplifier(id) => get_node!(id, amplifiers),
            RoutingNode::SpectralFilter(_) => get_node_error!(SpectralFilter),
            RoutingNode::Output => get_node_error!(Output),
        }
    }

    pub(super) fn resolve_scalar_output_node(
        &self,
        node: RoutingNode,
    ) -> Option<&dyn ScalarOutputModule> {
        macro_rules! get_node {
            ($id:ident, $container:ident) => {
                self.$container
                    .get($id)
                    .map(|module| module as &dyn ScalarOutputModule)
            };
        }

        macro_rules! get_node_error {
            ($node_type:ident) => {
                panic!(concat!(
                    stringify!($node_type),
                    " don't have scalar output."
                ))
            };
        }

        match node {
            RoutingNode::Envelope(id) => get_node!(id, envelopes),
            RoutingNode::Oscillator(_) => get_node_error!(Oscillator),
            RoutingNode::Amplifier(_) => get_node_error!(Amplifier),
            RoutingNode::SpectralFilter(_) => get_node_error!(SpectralFilter),
            RoutingNode::Output => get_node_error!(Output),
        }
    }

    pub(super) fn resolve_spectral_output_node(
        &self,
        node: RoutingNode,
    ) -> Option<&dyn SpectralOutputModule> {
        macro_rules! get_node {
            ($id:ident, $container:ident) => {
                self.$container
                    .get($id)
                    .map(|module| module as &dyn SpectralOutputModule)
            };
        }

        macro_rules! get_node_error {
            ($node_type:ident) => {
                panic!(concat!(
                    stringify!($node_type),
                    " don't have scalar output."
                ))
            };
        }

        match node {
            RoutingNode::SpectralFilter(id) => get_node!(id, spectral_filters),
            RoutingNode::Oscillator(_) => get_node_error!(Oscillator),
            RoutingNode::Envelope(_) => get_node_error!(Envelope),
            RoutingNode::Amplifier(_) => get_node_error!(Amplifier),
            RoutingNode::Output => get_node_error!(Output),
        }
    }

    pub(super) fn resolve_node_mut(&mut self, node: RoutingNode) -> Option<&mut dyn SynthModule> {
        macro_rules! get_node {
            ($id:ident, $container:ident) => {
                self.$container
                    .get_mut($id)
                    .map(|module| module as &mut dyn SynthModule)
            };
        }

        match node {
            RoutingNode::Oscillator(id) => get_node!(id, oscillators),
            RoutingNode::Envelope(id) => get_node!(id, envelopes),
            RoutingNode::Amplifier(id) => get_node!(id, amplifiers),
            RoutingNode::SpectralFilter(id) => get_node!(id, spectral_filters),
            RoutingNode::Output => panic!("RoutingNode::Output don't have corresponding module."),
        }
    }

    pub(super) fn get_routing_nodes(&self) -> Vec<RoutingNode> {
        let mut nodes = Vec::with_capacity(
            self.amplifiers.modules.len()
                + self.envelopes.modules.len()
                + self.oscillators.modules.len()
                + self.spectral_filters.modules.len(),
        );

        for id in self.amplifiers.modules.keys() {
            nodes.push(RoutingNode::Amplifier(*id));
        }

        for id in self.envelopes.modules.keys() {
            nodes.push(RoutingNode::Envelope(*id));
        }

        for id in self.oscillators.modules.keys() {
            nodes.push(RoutingNode::Oscillator(*id));
        }

        for id in self.spectral_filters.modules.keys() {
            nodes.push(RoutingNode::SpectralFilter(*id));
        }

        nodes
    }

    pub(super) fn is_node_exists(&self, node: RoutingNode) -> bool {
        match node {
            RoutingNode::Oscillator(id) => self.oscillators.modules.contains_key(&id),
            RoutingNode::Envelope(id) => self.envelopes.modules.contains_key(&id),
            RoutingNode::Amplifier(id) => self.amplifiers.modules.contains_key(&id),
            RoutingNode::SpectralFilter(id) => self.spectral_filters.modules.contains_key(&id),
            RoutingNode::Output => true,
        }
    }
}
