#[macro_export]
macro_rules! show_modal {
    ($self:ident, $state:ident, $func:ident, $synth:ident, $ui:ident) => {
        if let Some(mut state) = $self.$state.take()
            && $self.$func($synth, $ui, &mut state)
        {
            $self.$state.replace(state);
        }
    };
}
