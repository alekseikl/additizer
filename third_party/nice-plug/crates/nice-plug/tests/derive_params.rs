use nice_plug::prelude::*;

#[derive(Params)]
struct FlatParams {
    #[id = "one"]
    pub one: BoolParam,

    #[id = "two"]
    pub two: FloatParam,

    #[id = "three"]
    pub three: IntParam,
}

impl Default for FlatParams {
    fn default() -> Self {
        FlatParams {
            one: BoolParam::new("one", true),
            two: FloatParam::new("two", 0.0, FloatRange::Linear { min: 0.0, max: 1.0 }),
            three: IntParam::new("three", 0, IntRange::Linear { min: 0, max: 100 }),
        }
    }
}

#[derive(Params)]
struct GroupedParams {
    #[id = "one"]
    pub one: BoolParam,

    #[nested(group = "Some Group", id_prefix = "group1")]
    pub group1: FlatParams,

    #[id = "three"]
    pub three: IntParam,

    #[nested(group = "Another Group", id_prefix = "group2")]
    pub group2: FlatParams,
}

impl Default for GroupedParams {
    fn default() -> Self {
        GroupedParams {
            one: BoolParam::new("one", true),
            group1: FlatParams::default(),
            three: IntParam::new("three", 0, IntRange::Linear { min: 0, max: 100 }),
            group2: FlatParams::default(),
        }
    }
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

#[derive(Params)]
struct NestedParams {
    #[id = "one"]
    pub one: BoolParam,

    #[nested(id_prefix = "two")]
    pub two: FlatParams,

    #[id = "three"]
    pub three: IntParam,
}

impl Default for NestedParams {
    fn default() -> Self {
        NestedParams {
            one: BoolParam::new("one", true),
            two: FlatParams::default(),
            three: IntParam::new("three", 0, IntRange::Linear { min: 0, max: 100 }),
        }
    }
}

#[derive(Params)]
struct NestedArrayParams {
    #[id = "one"]
    pub one: BoolParam,

    #[nested(array, group = "Nested Params")]
    pub lots_of_twos: [FlatParams; 3],

    #[id = "three"]
    pub three: IntParam,
}

impl Default for NestedArrayParams {
    fn default() -> Self {
        NestedArrayParams {
            one: BoolParam::new("one", true),
            lots_of_twos: [
                FlatParams::default(),
                FlatParams::default(),
                FlatParams::default(),
            ],
            three: IntParam::new("three", 0, IntRange::Linear { min: 0, max: 100 }),
        }
    }
}

mod plugin_state {
    use super::*;

    #[test]
    fn derive_builds_plugin_state() {
        assert!(FlatParams::default().params().is_empty());
        assert!(GroupedParams::default().params().is_empty());
        assert!(PlainNestedParams::default().params().is_empty());
        assert!(GroupedGroupedParams::default().params().is_empty());
        assert!(NestedParams::default().params().is_empty());
        assert!(NestedArrayParams::default().params().is_empty());
        assert!(FlatParams::default().serialize_fields().is_empty());
    }
}
