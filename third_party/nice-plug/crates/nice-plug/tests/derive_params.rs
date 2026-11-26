use nice_plug::prelude::*;

#[derive(Params, Default)]
struct FlatParams {
    #[id = "one"]
    pub one: i32,

    #[id = "two"]
    pub two: i32,

    #[id = "three"]
    pub three: i32,
}

#[derive(Params, Default)]
struct GroupedParams {
    #[id = "one"]
    pub one: i32,

    #[nested(group = "Some Group", id_prefix = "group1")]
    pub group1: FlatParams,

    #[id = "three"]
    pub three: i32,

    #[nested(group = "Another Group", id_prefix = "group2")]
    pub group2: FlatParams,
}

// This should result in the same `.param_map()` as `GroupedParams`
#[derive(Default, Params)]
struct PlainNestedParams {
    #[nested]
    pub inner: GroupedParams,
}

#[derive(Default, Params)]
struct GroupedGroupedParams {
    #[nested(group = "Top-level group")]
    pub one: GroupedParams,
}

#[derive(Params, Default)]
struct NestedParams {
    #[id = "one"]
    pub one: i32,

    #[nested(id_prefix = "two")]
    pub two: FlatParams,

    #[id = "three"]
    pub three: i32,
}

#[derive(Params, Default)]
struct NestedArrayParams {
    #[id = "one"]
    pub one: i32,

    #[nested(array, group = "Nested Params")]
    pub lots_of_twos: [FlatParams; 3],

    #[id = "three"]
    pub three: i32,
}

mod plugin_state {
    use super::*;

    #[test]
    fn derive_builds_plugin_state() {
        assert!(FlatParams::default().serialize_fields().is_empty());
        assert!(GroupedParams::default().serialize_fields().is_empty());
        assert!(PlainNestedParams::default().serialize_fields().is_empty());
        assert!(GroupedGroupedParams::default().serialize_fields().is_empty());
        assert!(NestedParams::default().serialize_fields().is_empty());
        assert!(NestedArrayParams::default().serialize_fields().is_empty());
    }
}
