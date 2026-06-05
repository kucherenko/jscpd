use cpd_core::models::Location;

pub(crate) struct LineIndex {
    newlines: Vec<usize>,
}

impl LineIndex {
    pub(crate) fn new(content: &[u8]) -> Self {
        let newlines = content
            .iter()
            .enumerate()
            .filter_map(|(i, &b)| (b == b'\n').then_some(i))
            .collect();
        Self { newlines }
    }

    pub(crate) fn location(&self, offset: usize) -> Location {
        let previous_newlines = self.newlines.partition_point(|&nl| nl < offset);
        let line_start = if previous_newlines == 0 {
            0
        } else {
            self.newlines[previous_newlines - 1] + 1
        };
        Location {
            line: previous_newlines as u32 + 1,
            column: (offset - line_start) as u32,
            offset: offset as u32,
        }
    }
}
