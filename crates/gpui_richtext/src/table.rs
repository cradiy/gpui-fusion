use gpui::{Pixels, TextAlign};
use std::ops::Range;

/// Width allocated to a table column.
#[derive(Clone, Copy, Debug)]
pub enum ColumnWidth {
    Fixed(Pixels),
    Flex(f32),
}

/// Vertical placement of content within a table row.
#[derive(Clone, Copy, Debug, Default)]
pub enum CellVerticalAlign {
    #[default]
    Top,
    Center,
    Bottom,
}

/// Column sizing and horizontal text alignment.
#[derive(Clone, Copy, Debug)]
pub struct TableColumn {
    pub width: ColumnWidth,
    pub align: TextAlign,
    pub vertical_align: CellVerticalAlign,
}

impl Default for TableColumn {
    fn default() -> Self {
        Self {
            width: ColumnWidth::Flex(1.),
            align: TextAlign::Left,
            vertical_align: CellVerticalAlign::Top,
        }
    }
}

impl TableColumn {
    pub fn fixed(width: Pixels) -> Self {
        Self {
            width: ColumnWidth::Fixed(width),
            ..Self::default()
        }
    }
    pub fn flex(weight: f32) -> Self {
        Self {
            width: ColumnWidth::Flex(weight),
            ..Self::default()
        }
    }
    pub fn align(mut self, align: TextAlign) -> Self {
        self.align = align;
        self
    }

    pub fn vertical_align(mut self, align: CellVerticalAlign) -> Self {
        self.vertical_align = align;
        self
    }
}

/// A block containing table rows. Unspecified columns receive equal widths.
#[derive(Clone, Debug, Default)]
pub struct Table {
    pub columns: Vec<TableColumn>,
}

impl Table {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn columns(mut self, columns: impl IntoIterator<Item = TableColumn>) -> Self {
        self.columns = columns.into_iter().collect();
        self
    }
}

/// A row containing only table cells.
#[derive(Clone, Copy, Debug, Default)]
pub struct TableRow {
    pub header: bool,
}

/// A cell containing inline nodes. Empty cells are supported.
pub struct TableCell;

/// Compiled cell content in the document's text stream.
#[derive(Clone, Debug)]
pub struct TableCellSpan {
    pub node: crate::NodeId,
    pub range: Range<usize>,
}

#[derive(Clone, Debug)]
pub struct TableRowSpan {
    pub node: crate::NodeId,
    pub header: bool,
    pub cells: Vec<TableCellSpan>,
}

/// Compiled row and column metadata, independent of the input node tree.
#[derive(Clone, Debug)]
pub struct TableBlock {
    pub columns: Vec<TableColumn>,
    pub rows: Vec<TableRowSpan>,
}

/// Table surface and cell spacing, independent of inline text styles.
#[derive(Clone)]
pub struct TableStyle {
    pub background: gpui::Hsla,
    pub header_background: gpui::Hsla,
    pub alternate_background: Option<gpui::Hsla>,
    pub border_color: gpui::Hsla,
    pub row_borders: bool,
    pub column_borders: bool,
    pub cell_padding: gpui::Size<Pixels>,
    pub min_column_width: Pixels,
    pub corner_radius: Pixels,
}

impl Default for TableStyle {
    fn default() -> Self {
        Self {
            background: gpui::rgb(0xffffff).into(),
            header_background: gpui::rgb(0xf1f5f9).into(),
            alternate_background: Some(gpui::rgb(0xf8fafc).into()),
            border_color: gpui::rgb(0xe2e8f0).into(),
            row_borders: true,
            column_borders: true,
            cell_padding: gpui::size(gpui::px(12.), gpui::px(8.)),
            min_column_width: gpui::px(128.),
            corner_radius: gpui::px(8.),
        }
    }
}
