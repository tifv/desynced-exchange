//! This module implements the blueprint exchange strings generated by
//! the Desynced game as data format for `serde`.
//! 
//! The game allows exporting both blueprints and behaviours
//! as exchange strings.
//! Here they both will be denoted as “blueprints”.
//! 
//! A rough description of a blueprint string structure:
//! * header indicating whether it is a blueprint or a behaviour;
//! * encoded length of uncompressed data, or zero in case of
//!   no compression;
//! * base62-encoding (followed by a checksum digit) of
//! * (optionally) zlib-compression of
//! * low-level binary encoding of Lua data structures.
//! 
//! The last step is the most interesting one and will be discussed in
//! the following sections.
//! The steps before that are fairly straightforward.
//! When serializing, there is a choice to compress or not;
//! this library will opt to compress except the shorter strings
//! (the game may make somewhat different choice when creating strings,
//! but it doesn't seem to matter).
//! 
//! ## Lua data types
//! 
//! The following data types are used in the serialized blueprints:
//! * `nil`;
//! * booleans;
//! * signed 32-bit integers;
//! * double-precision floating point numbers;
//! * UTF-8 encoded strings;
//! * tables.
//! 
//! Tables are Lua's associative arrays. Lua allows keys to be arbitrary
//! values, but in blueprints keys are always integers and strings.
//! 
//! Here is how various data types are mapped to `serde` data model:
//! * `nil` is naturally mapped to none; `serde` unit is also accepted
//!   when serializing.
//! * booleans are mapped naturally.
//! * integers are naturally mapped to `serde`'s `i32`.
//!   Other integers are also accepted when serializing, if the value fits.
//! * floating point numbers are naturally mapped to `f64`.
//!   `f32` is also accepted when serializing.
//! * strings are mapped to `serde`'s strings.
//!   When serializing, unit structs are also mapped to strings.
//! * tables are mapped to `serde`'s maps.
//!   When serializing, structs, seqs and tuples are also mapped to table.
//! 
//! Newtype structs during serialization, unless they are table keys,
//! are mapped to the contained value.
//! 
//! Table keys can map to the following:
//! * integers;
//! * strings
//!   (when serializing, unit structs are also mapped to strings);
//! * dead keys (see below) map to newtype `Dead(u32)`
//!   (when serializing, `Dead(str)` is also accepted).
//!   Other newtype structs are not supported as table keys.
//! 
//! Table value can be any normally accepted value if they correspond
//! to a non-dead key;
//! however, a dead key must always correspond to the `nil` value.
//! 
//! Yeah, also tables with more than 32 associative keys are not supported
//! (where “associative” essentially means string keys, at least in case
//! when integer keys form a continuous range 1..=len;
//! i.e. arrays with hundreds of elements are ok).
//! 
//! ## Nuances
//! 
//! About those dead keys…
//! 
//! The blueprint exchange strings… let's just say that they were not
//! designed with interoperability in mind.
//! Therefore this section will be non-trivial.
//! 
//! When Lua deletes an item from a table (which you can think of as
//! an array of key-value pairs), it sets the value to `nil`.
//! The garbage collector will later find such key and clear it,
//! replacing it with a `deadkey` tombstones.
//! 
//! The game makes zero effort to filter out deleted keys when creating
//! blueprint strings.
//! Upon deserialization, deleted items will show up having `nil` value.
//! Applications are encouraged to ignore them.
//! The keys in such items may come in two flavors:
//! * keys represented as normal integers or strings
//!   (not yet found by garbage collector);
//! * tombstones represented as `Dead(u32)` newtype structs,
//!   where the contained value indicates
//!   the position of the deleted item key in the table
//!   (which coincides with the last bits of key's hash, unless the key
//!   was out of its main position due to hash coliisions).
//! 
//! You may ask at this point, why not remove deleted keys at
//! the deserializer level?
//! Indeed, this way we would be able to completely ignore the existence
//! of dead keys both when deserializing and serializing.
//! 
//! Alas, the latter is not true.
//! There are cases when the game expects there to be a dead key in
//! the serialized table.
//! If you omit it, the blueprint string will not be recognized
//! by the game.
//! 
//! Therefore the dead keys are necessary element of serialization.
//! While deserialization doesn't technically need then, dead keys
//! are retained during it anyway to maintain logical
//! correspondence
//! (but mainly to serve debugging needs and
//! blueprint → `serde::Value` → blueprint conversion).
//! 
//! ### A dead key case
//! 
//! Actually, at this moment only one case is known when
//! the blueprint will not be recognized without a certain dead key.
//! 
//! A behaviour contains a sequence of instructions;
//! each instruction is encoded as a table.
//! An instruction table
//! * must contain `op` field denoting the operation;
//! * may contain numbered fields for instruction's arguments;
//! * may contain `next` field with the index of the next instruction
//!   (omitted when the next instruction sits literally next in the sequence);
//! * may contain other fields such as `cmt`, `nx`, `ny`.
//! 
//! The problem is with the `next` field.
//! It cannot be completely omitted: you must either include it
//! (with a normal value or `nil`) or add the corresponding tombstone.
//! The latter can be done during serialization by writing
//! newtype `Dead("next")` as key.
//! 
//! (Since I can't actually look into the game's code, the previous
//! paragraph is my observation of what seems to work in practice.)

// LINTS: useful
#![warn(unused_unsafe)]
#![deny(unsafe_op_in_unsafe_fn)]
#![warn(clippy::allow_attributes_without_reason)]
#![warn(clippy::as_underscore)]
#![warn(clippy::borrow_as_ptr)]
#![warn(clippy::branches_sharing_code)]
#![warn(clippy::cast_ptr_alignment)]
#![warn(clippy::clone_on_ref_ptr)]
#![warn(clippy::cognitive_complexity)]
#![warn(clippy::copy_iterator)]
#![warn(clippy::debug_assert_with_mut_call)]
#![warn(clippy::deref_by_slicing)]
#![warn(clippy::derive_partial_eq_without_eq)]
#![warn(clippy::enum_glob_use)]
#![warn(clippy::explicit_deref_methods)]
#![warn(clippy::explicit_into_iter_loop)]
#![warn(clippy::explicit_iter_loop)]
#![warn(clippy::fallible_impl_from)]
#![warn(clippy::filter_map_next)]
#![warn(clippy::flat_map_option)]
#![warn(clippy::float_cmp)]
#![warn(clippy::float_cmp_const)]
#![warn(clippy::fn_params_excessive_bools)]
#![warn(clippy::fn_to_numeric_cast_any)]
#![warn(clippy::format_push_string)]
#![warn(clippy::from_iter_instead_of_collect)]
#![warn(clippy::if_then_some_else_none)]
#![warn(clippy::implicit_clone)]
#![warn(clippy::implicit_hasher)]
#![warn(clippy::inconsistent_struct_constructor)]
#![warn(clippy::index_refutable_slice)]
#![warn(clippy::large_digit_groups)]
#![warn(clippy::large_stack_arrays)]
#![warn(clippy::large_types_passed_by_value)]
#![warn(clippy::manual_clamp)]
#![warn(clippy::manual_let_else)]
#![warn(clippy::manual_ok_or)]
#![warn(clippy::manual_rem_euclid)]
#![warn(clippy::manual_string_new)]
#![warn(clippy::many_single_char_names)]
#![warn(clippy::map_unwrap_or)]
#![warn(clippy::match_bool)]
#![warn(clippy::match_on_vec_items)]
#![warn(clippy::match_same_arms)]
#![warn(clippy::match_wild_err_arm)]
#![warn(clippy::match_wildcard_for_single_variants)]
#![warn(clippy::mem_forget)]
#![warn(clippy::mismatching_type_param_order)]
#![warn(clippy::multiple_unsafe_ops_per_block)]
#![warn(clippy::must_use_candidate)]
#![warn(clippy::mut_mut)]
#![warn(clippy::needless_for_each)]
#![warn(clippy::needless_pass_by_value)]
#![warn(clippy::option_if_let_else)]
#![warn(clippy::option_option)]
#![warn(clippy::or_fun_call)]
#![warn(clippy::partial_pub_fields)]
#![warn(clippy::ptr_as_ptr)]
#![warn(clippy::range_minus_one)]
#![warn(clippy::range_plus_one)]
#![warn(clippy::rc_buffer)]
#![warn(clippy::rc_mutex)]
#![warn(clippy::redundant_closure_for_method_calls)]
#![warn(clippy::redundant_else)]
#![warn(clippy::ref_binding_to_reference)]
#![warn(clippy::return_self_not_must_use)]
#![warn(clippy::semicolon_inside_block)]
#![warn(clippy::shadow_unrelated)]
#![warn(clippy::similar_names)]
#![warn(clippy::stable_sort_primitive)]
#![warn(clippy::struct_excessive_bools)]
#![warn(clippy::suboptimal_flops)]
#![warn(clippy::too_many_lines)]
#![warn(clippy::trait_duplication_in_bounds)]
#![warn(clippy::transmute_ptr_to_ptr)]
#![warn(clippy::trivially_copy_pass_by_ref)]
#![warn(clippy::type_repetition_in_bounds)]
#![warn(clippy::undocumented_unsafe_blocks)]
#![warn(clippy::unicode_not_nfc)]
#![warn(clippy::uninlined_format_args)]
#![warn(clippy::unnecessary_join)]
#![warn(clippy::unnecessary_wraps)]
#![warn(clippy::unneeded_field_pattern)]
#![warn(clippy::unnested_or_patterns)]
#![warn(clippy::unreadable_literal)]
#![warn(clippy::unsafe_derive_deserialize)]
#![warn(clippy::unseparated_literal_suffix)]
#![warn(clippy::unused_async)]
#![warn(clippy::unused_peekable)]
#![warn(clippy::unused_self)]
#![warn(clippy::unwrap_in_result)]
#![warn(clippy::use_self)]


// LINTS: harmless
#![warn(clippy::assertions_on_result_states)]
#![warn(clippy::bool_to_int_with_if)]
#![warn(clippy::case_sensitive_file_extension_comparisons)]
#![warn(clippy::cast_lossless)]
#![warn(clippy::checked_conversions)]
#![warn(clippy::cloned_instead_of_copied)]
#![warn(clippy::decimal_literal_representation)]
#![warn(clippy::default_trait_access)]
#![warn(clippy::default_union_representation)]
#![warn(clippy::disallowed_script_idents)]
#![warn(clippy::doc_link_with_quotes)]
#![warn(clippy::empty_drop)]
#![warn(clippy::empty_enum)]
#![warn(clippy::empty_line_after_outer_attr)]
#![warn(clippy::empty_structs_with_brackets)]
#![warn(clippy::equatable_if_let)]
#![warn(clippy::exit)]
#![warn(clippy::expl_impl_clone_on_copy)]
#![warn(clippy::future_not_send)]
#![warn(clippy::imprecise_flops)]
#![warn(clippy::inefficient_to_string)]
#![warn(clippy::inline_asm_x86_att_syntax)]
#![warn(clippy::invalid_upcast_comparisons)]
#![warn(clippy::items_after_statements)]
#![warn(clippy::iter_not_returning_iterator)]
#![warn(clippy::iter_with_drain)]
#![warn(clippy::let_underscore_must_use)]
#![warn(clippy::let_underscore_untyped)]
#![warn(clippy::linkedlist)]
#![warn(clippy::lossy_float_literal)]
#![warn(clippy::macro_use_imports)]
#![warn(clippy::manual_assert)]
#![warn(clippy::manual_instant_elapsed)]
#![warn(clippy::map_err_ignore)]
#![warn(clippy::needless_bitwise_bool)]
#![warn(clippy::needless_continue)]
#![warn(clippy::same_functions_in_if_condition)]
#![warn(clippy::same_name_method)]
#![warn(clippy::str_to_string)]
#![warn(clippy::string_add)]
#![warn(clippy::string_add_assign)]
#![warn(clippy::string_slice)]
#![warn(clippy::string_to_string)]
#![warn(clippy::used_underscore_binding)]
#![warn(clippy::useless_let_if_seq)]
#![warn(clippy::verbose_bit_mask)]
#![warn(clippy::verbose_file_reads)]
#![warn(clippy::wildcard_imports)]
#![warn(clippy::zero_sized_map_values)]

// LINTS: development temporary
#![allow(dead_code)]
#![allow(unreachable_code)]
#![allow(unreachable_patterns)]
#![allow(unused_imports)]
#![allow(unused_variables)]
#![allow(unused_mut)]
#![allow(unused_assignments)]
#![allow(irrefutable_let_patterns)]
#![allow(clippy::diverging_sub_expression)]
#![allow(clippy::needless_pass_by_value)]

// LINTS: production
// #![warn(clippy::todo)]
// #![warn(clippy::unimplemented)]
// #![warn(clippy::dbg_macro)]
// #![warn(clippy::exhaustive_enums)]
// #![warn(clippy::exhaustive_structs)]


mod string;
mod table;

mod ser;
mod de;
