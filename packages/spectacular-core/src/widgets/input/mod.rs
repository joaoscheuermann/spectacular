use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Modifier, Style},
    widgets::{Block, Widget},
};

#[derive(Debug)]
pub struct Input<'a> {
    value: &'a str,
    block: Option<Block<'a>>,
    cursor_position: usize,
    cursor_style: Style,
    cursor_visible: bool,
    focused: bool,
    mask: Option<char>,
    style: Style,
}

impl<'a> Input<'a> {
    pub fn new(value: &'a str) -> Self {
        Self {
            value,
            block: None,
            cursor_position: value.chars().count(),
            cursor_style: Style::default().add_modifier(Modifier::REVERSED),
            cursor_visible: true,
            focused: false,
            mask: None,
            style: Style::default(),
        }
    }

    pub fn block(mut self, block: Block<'a>) -> Self {
        self.block = Some(block);
        self
    }

    pub fn cursor_position(mut self, cursor_position: usize) -> Self {
        self.cursor_position = cursor_position.min(self.value.chars().count());
        self
    }

    pub fn cursor_style(mut self, style: Style) -> Self {
        self.cursor_style = style;
        self
    }

    pub fn cursor_visible(mut self, visible: bool) -> Self {
        self.cursor_visible = visible;
        self
    }

    pub fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }

    pub fn mask(mut self, mask: char) -> Self {
        self.mask = Some(mask);
        self
    }

    pub fn style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }
}

impl Widget for Input<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        let display_chars = self.display_chars();
        let input_area = match self.block {
            Some(block) => {
                let inner_area = block.inner(area);
                block.render(area, buffer);
                inner_area
            }
            None => area,
        };

        if input_area.is_empty() {
            return;
        }

        let width = input_area.width as usize;
        let cursor_position = self.cursor_position.min(display_chars.len());
        let visible_start = visible_start(cursor_position, width);
        let visible_chars = display_chars
            .iter()
            .skip(visible_start)
            .take(width)
            .copied()
            .collect::<Vec<_>>();

        for offset in 0..input_area.width {
            buffer
                .cell_mut((input_area.x + offset, input_area.y))
                .expect("input cell should be in bounds")
                .set_symbol(" ")
                .set_style(self.style);
        }

        for (offset, character) in visible_chars.iter().enumerate() {
            buffer
                .cell_mut((input_area.x + offset as u16, input_area.y))
                .expect("input text cell should be in bounds")
                .set_char(*character)
                .set_style(self.style);
        }

        if !self.focused || !self.cursor_visible || width == 0 {
            return;
        }

        let cursor_offset = cursor_position.saturating_sub(visible_start).min(width - 1);
        buffer
            .cell_mut((input_area.x + cursor_offset as u16, input_area.y))
            .expect("input cursor cell should be in bounds")
            .set_style(self.cursor_style);
    }
}

impl Input<'_> {
    fn display_chars(&self) -> Vec<char> {
        match self.mask {
            Some(mask) => self.value.chars().map(|_| mask).collect(),
            None => self.value.chars().collect(),
        }
    }
}

fn visible_start(cursor_position: usize, width: usize) -> usize {
    if width == 0 || cursor_position < width {
        return 0;
    }

    cursor_position + 1 - width
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::widgets::Borders;

    #[test]
    fn input_masks_value() {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 20, 1));

        Input::new("secret")
            .mask('*')
            .render(buffer.area, &mut buffer);

        let rendered = buffer
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();
        assert!(rendered.starts_with("******"));
        assert!(!rendered.contains("secret"));
    }

    #[test]
    fn focused_input_marks_cursor_cell() {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 20, 1));

        Input::new("secret")
            .mask('*')
            .focused(true)
            .cursor_position(6)
            .render(buffer.area, &mut buffer);

        let cell = buffer.cell((6, 0)).expect("cursor cell should exist");
        assert!(cell.style().add_modifier.contains(Modifier::REVERSED));
    }

    #[test]
    fn focused_input_can_hide_cursor_cell() {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 20, 1));

        Input::new("secret")
            .mask('*')
            .focused(true)
            .cursor_visible(false)
            .cursor_position(6)
            .render(buffer.area, &mut buffer);

        let cell = buffer.cell((6, 0)).expect("cursor cell should exist");
        assert!(!cell.style().add_modifier.contains(Modifier::REVERSED));
    }

    #[test]
    fn input_renders_block() {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 20, 3));

        Input::new("secret")
            .block(Block::default().borders(Borders::ALL).title("API KEY"))
            .render(buffer.area, &mut buffer);

        let rendered = buffer
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();
        assert!(rendered.contains("API KEY"));
    }
}
