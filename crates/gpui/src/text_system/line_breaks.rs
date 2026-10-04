use unicode_segmentation::UnicodeSegmentation;

pub(super) struct LineBreaks {
    pub graphemes: Vec<usize>,
    opportunities: Vec<usize>,
}

impl LineBreaks {
    pub fn new(text: &str) -> Self {
        Self {
            graphemes: text
                .grapheme_indices(true)
                .map(|(ix, _)| ix)
                .chain([text.len()])
                .collect(),
            opportunities: unicode_linebreak::linebreaks(text)
                .map(|(ix, _)| ix)
                .collect(),
        }
    }

    pub fn is_grapheme_boundary(&self, ix: usize) -> bool {
        self.graphemes.binary_search(&ix).is_ok()
    }

    pub fn can_wrap(&self, ix: usize) -> bool {
        self.opportunities.binary_search(&ix).is_ok() && self.is_grapheme_boundary(ix)
    }
}
