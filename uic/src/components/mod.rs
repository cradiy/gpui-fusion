//! UIC component building blocks.
//!
//! Public renderable components follow GPUI's [`gpui::Styled`] model. Their
//! outer layout and visual style use normal style methods, and text styling is
//! inherited through the component tree. Appearance types describe interaction
//! states and internal geometry that belong to the component itself.

pub mod badge;
pub mod bottom_sheet;
pub mod calendar;
pub mod collapsible;
pub mod color_picker;
pub mod context_menu;
pub mod dropdown;
pub mod glass;
pub mod input;
pub mod modal;
pub mod notification;
pub mod number_input;
mod overlay_anchor;
pub mod pager;
pub mod popover;
pub mod progress;
mod range;
pub mod refresh;
pub mod reorderable_list;
pub mod screen_color_picker;
pub mod scrollbar;
pub mod select;
pub mod selection;
pub mod slider;
pub mod swipe_actions;
pub mod tabs;
pub mod toast;
pub mod tree_picker;
pub mod ui;
pub mod zoom_view;

pub use ui::space;
