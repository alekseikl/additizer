//! Enum parameters. `enum` is a keyword, so `enums` it is.

// Re-export the derive macro
pub use nice_plug_derive::Enum;

/// An enum whose variants are addressed by declaration order.
///
/// Variants are identified either by a stable id, or if those are not set then by their
/// **declaration order**. If you don't provide ids then you can freely rename the variant names,
/// but reordering them will break compatibility with existing presets. The variant's name is used
/// as the display name by default. Override it with `#[name = "..."]` when the name needs spaces:
///
/// ```ignore
/// #[derive(Enum)]
/// enum Foo {
///     Bar,
///     Baz,
///     #[name = "Contains Spaces"]
///     ContainsSpaces,
/// }
/// ```
///
/// Ids can be added with `#[id = "..."]` on each variant:
///
/// ```ignore
/// #[derive(Enum)]
/// enum Foo {
///     #[id = "bar"],
///     Bar,
///     #[id = "baz"],
///     Baz,
/// }
/// ```
///
/// You can move from not using ids to using ids without breaking patches, but you cannot go back
/// to not using ids after that.
pub trait Enum {
    /// The human readable names for the variants. The length of this slice determines how many
    /// variants there are.
    fn variants() -> &'static [&'static str];

    /// Optional identifiers for each variant. This makes it possible to reorder variants while
    /// maintaining save compatibility. The length of this slice needs to be equal to
    /// [`variants()`][Self::variants()].
    fn ids() -> Option<&'static [&'static str]>;

    /// Variant index corresponding to the active variant. The index matches the name in
    /// [`variants()`][Self::variants()].
    fn to_index(self) -> usize;

    /// Variant for the index in [`variants()`][Self::variants()]. This must always return a value.
    /// If the index is out of range, return the first variant.
    fn from_index(index: usize) -> Self;
}
