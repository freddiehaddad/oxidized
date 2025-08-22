use log::{debug, info};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SplitDirection {
    Horizontal,      // Default horizontal split (below)
    Vertical,        // Default vertical split (right)
    HorizontalAbove, // Split above current window
    HorizontalBelow, // Split below current window
    VerticalLeft,    // Split to the left of current window
    VerticalRight,   // Split to the right of current window
}

#[derive(Debug, Clone)]
pub struct Window {
    pub id: usize,
    pub buffer_id: Option<usize>,
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
    pub viewport_top: usize,
    /// Cursor position within this window's buffer
    pub cursor_row: usize,
    pub cursor_col: usize,
    /// Horizontal scroll offset (in columns) for long lines when wrap is disabled
    pub horiz_offset: usize,
}

impl Window {
    pub fn new(id: usize, x: u16, y: u16, width: u16, height: u16) -> Self {
        debug!(
            "Creating new window {} at position ({}x{}) with size {}x{}",
            id, x, y, width, height
        );
        Self {
            id,
            buffer_id: None,
            x,
            y,
            width,
            height,
            viewport_top: 0,
            cursor_row: 0,
            cursor_col: 0,
            horiz_offset: 0,
        }
    }

    pub fn set_buffer(&mut self, buffer_id: usize) {
        self.buffer_id = Some(buffer_id);
    }

    pub fn save_cursor_position(&mut self, row: usize, col: usize) {
        self.cursor_row = row;
        self.cursor_col = col;
    }

    pub fn get_cursor_position(&self) -> (usize, usize) {
        (self.cursor_row, self.cursor_col)
    }

    pub fn content_height(&self) -> usize {
        // Reserve 1 line for status bar at bottom of each split
        self.height.saturating_sub(1) as usize
    }

    pub fn is_point_inside(&self, x: u16, y: u16) -> bool {
        x >= self.x && x < self.x + self.width && y >= self.y && y < self.y + self.height
    }
}

#[derive(Clone)]
pub struct WindowManager {
    windows: HashMap<usize, Window>,
    next_window_id: usize,
    current_window_id: Option<usize>,
    terminal_width: u16,
    terminal_height: u16,
    /// Number of rows reserved at the bottom for global UI (status/command lines)
    reserved_rows: u16,
}

impl WindowManager {
    pub fn new(terminal_width: u16, terminal_height: u16) -> Self {
        let mut manager = Self {
            windows: HashMap::new(),
            next_window_id: 1,
            current_window_id: None,
            terminal_width,
            terminal_height,
            reserved_rows: 2,
        };

        // Create initial window that fills the entire screen (minus status line)
        let initial_window = Window::new(
            1,
            0,
            0,
            terminal_width,
            terminal_height.saturating_sub(manager.reserved_rows), // Reserve rows for command/status
        );

        manager.windows.insert(1, initial_window);
        manager.current_window_id = Some(1);
        manager.next_window_id = 2;

        manager
    }

    pub fn current_window(&self) -> Option<&Window> {
        self.current_window_id.and_then(|id| self.windows.get(&id))
    }

    pub fn current_window_mut(&mut self) -> Option<&mut Window> {
        self.current_window_id
            .and_then(|id| self.windows.get_mut(&id))
    }

    pub fn current_window_id(&self) -> Option<usize> {
        self.current_window_id
    }

    pub fn set_current_window(&mut self, window_id: usize) -> bool {
        if self.windows.contains_key(&window_id) {
            self.current_window_id = Some(window_id);
            true
        } else {
            false
        }
    }

    pub fn get_window(&self, id: usize) -> Option<&Window> {
        self.windows.get(&id)
    }

    pub fn get_window_mut(&mut self, id: usize) -> Option<&mut Window> {
        self.windows.get_mut(&id)
    }

    pub fn all_windows(&self) -> &HashMap<usize, Window> {
        &self.windows
    }

    pub fn split_current_window(&mut self, direction: SplitDirection) -> Option<usize> {
        let current_id = self.current_window_id?;
        let current_window = self.windows.get(&current_id)?.clone();

        info!(
            "Splitting window {} in direction: {:?}",
            current_id, direction
        );

        let new_window_id = self.next_window_id;
        self.next_window_id += 1;

        let (window1, window2) = match direction {
            SplitDirection::Horizontal | SplitDirection::HorizontalBelow => {
                // Split horizontally - new window below (default behavior)
                let half_height = current_window.height / 2;

                let top_window = Window::new(
                    current_id,
                    current_window.x,
                    current_window.y,
                    current_window.width,
                    half_height,
                );

                let bottom_window = Window::new(
                    new_window_id,
                    current_window.x,
                    current_window.y + half_height,
                    current_window.width,
                    current_window.height - half_height,
                );

                (top_window, bottom_window)
            }
            SplitDirection::HorizontalAbove => {
                // Split horizontally - new window above
                let half_height = current_window.height / 2;

                let top_window = Window::new(
                    new_window_id,
                    current_window.x,
                    current_window.y,
                    current_window.width,
                    half_height,
                );

                let bottom_window = Window::new(
                    current_id,
                    current_window.x,
                    current_window.y + half_height,
                    current_window.width,
                    current_window.height - half_height,
                );

                (bottom_window, top_window)
            }
            SplitDirection::Vertical | SplitDirection::VerticalRight => {
                // Split vertically - new window to the right (default behavior)
                let half_width = current_window.width / 2;

                let left_window = Window::new(
                    current_id,
                    current_window.x,
                    current_window.y,
                    half_width,
                    current_window.height,
                );

                let right_window = Window::new(
                    new_window_id,
                    current_window.x + half_width,
                    current_window.y,
                    current_window.width - half_width,
                    current_window.height,
                );

                (left_window, right_window)
            }
            SplitDirection::VerticalLeft => {
                // Split vertically - new window to the left
                let half_width = current_window.width / 2;

                let left_window = Window::new(
                    new_window_id,
                    current_window.x,
                    current_window.y,
                    half_width,
                    current_window.height,
                );

                let right_window = Window::new(
                    current_id,
                    current_window.x + half_width,
                    current_window.y,
                    current_window.width - half_width,
                    current_window.height,
                );

                (right_window, left_window)
            }
        };

        // Preserve buffer assignment and viewport
        let mut modified_window1 = window1;
        modified_window1.buffer_id = current_window.buffer_id;
        modified_window1.viewport_top = current_window.viewport_top;

        let mut new_window = window2;
        new_window.buffer_id = current_window.buffer_id; // Same buffer initially

        // Update windows
        self.windows.insert(current_id, modified_window1);
        self.windows.insert(new_window_id, new_window);

        Some(new_window_id)
    }

    pub fn close_current_window(&mut self) -> bool {
        let current_id = match self.current_window_id {
            Some(id) => id,
            None => return false,
        };

        // Don't close if it's the only window
        if self.windows.len() <= 1 {
            return false;
        }

        self.windows.remove(&current_id);

        // Switch to another window
        if let Some(&next_id) = self.windows.keys().next() {
            self.current_window_id = Some(next_id);
            // TODO: Implement smart window resizing to fill the gap
            self.resize_windows_to_fill_space();
            true
        } else {
            false
        }
    }

    /// Close a specific window by id. Returns true if closed.
    pub fn close_window_by_id(&mut self, id: usize) -> bool {
        if !self.windows.contains_key(&id) {
            return false;
        }
        if self.windows.len() <= 1 {
            return false;
        }
        self.windows.remove(&id);
        if self.current_window_id == Some(id) {
            if let Some(&next_id) = self.windows.keys().next() {
                self.current_window_id = Some(next_id);
            } else {
                self.current_window_id = None;
            }
        }
        self.resize_windows_to_fill_space();
        self.fill_horizontal_gaps();
        true
    }

    // layout_summary removed (was for debug :Windows command)

    pub fn move_to_window_left(&mut self) -> bool {
        let current_window = match self.current_window() {
            Some(window) => window.clone(),
            None => return false,
        };
        let current_x = current_window.x;
        let current_y = current_window.y + current_window.height / 2; // Middle of current window

        // Find leftmost window to the left of current window
        let mut best_window = None;
        let mut best_distance = u16::MAX;

        for window in self.windows.values() {
            if window.id != current_window.id
                && window.x < current_x
                && window.y <= current_y
                && window.y + window.height > current_y
            {
                let distance = current_x - (window.x + window.width);
                if distance < best_distance {
                    best_distance = distance;
                    best_window = Some(window.id);
                }
            }
        }

        if let Some(window_id) = best_window {
            self.current_window_id = Some(window_id);
            true
        } else {
            false
        }
    }

    pub fn move_to_window_right(&mut self) -> bool {
        let current_window = match self.current_window() {
            Some(window) => window.clone(),
            None => return false,
        };
        let current_x = current_window.x + current_window.width;
        let current_y = current_window.y + current_window.height / 2; // Middle of current window

        // Find leftmost window to the right of current window
        let mut best_window = None;
        let mut best_distance = u16::MAX;

        for window in self.windows.values() {
            if window.id != current_window.id
                && window.x >= current_x
                && window.y <= current_y
                && window.y + window.height > current_y
            {
                let distance = window.x - current_x;
                if distance < best_distance {
                    best_distance = distance;
                    best_window = Some(window.id);
                }
            }
        }

        if let Some(window_id) = best_window {
            self.current_window_id = Some(window_id);
            true
        } else {
            false
        }
    }

    pub fn move_to_window_up(&mut self) -> bool {
        let current_window = match self.current_window() {
            Some(window) => window.clone(),
            None => return false,
        };
        let current_x = current_window.x + current_window.width / 2; // Middle of current window
        let current_y = current_window.y;

        // Find bottommost window above current window
        let mut best_window = None;
        let mut best_distance = u16::MAX;

        for window in self.windows.values() {
            if window.id != current_window.id
                && window.y < current_y
                && window.x <= current_x
                && window.x + window.width > current_x
            {
                let distance = current_y - (window.y + window.height);
                if distance < best_distance {
                    best_distance = distance;
                    best_window = Some(window.id);
                }
            }
        }

        if let Some(window_id) = best_window {
            self.current_window_id = Some(window_id);
            true
        } else {
            false
        }
    }

    pub fn move_to_window_down(&mut self) -> bool {
        let current_window = match self.current_window() {
            Some(window) => window.clone(),
            None => return false,
        };
        let current_x = current_window.x + current_window.width / 2; // Middle of current window
        let current_y = current_window.y + current_window.height;

        // Find topmost window below current window
        let mut best_window = None;
        let mut best_distance = u16::MAX;

        for window in self.windows.values() {
            if window.id != current_window.id
                && window.y >= current_y
                && window.x <= current_x
                && window.x + window.width > current_x
            {
                let distance = window.y - current_y;
                if distance < best_distance {
                    best_distance = distance;
                    best_window = Some(window.id);
                }
            }
        }

        if let Some(window_id) = best_window {
            self.current_window_id = Some(window_id);
            true
        } else {
            false
        }
    }

    pub fn resize_terminal(&mut self, width: u16, height: u16) {
        self.terminal_width = width;
        self.terminal_height = height;

        // If only one window, resize it to fill the screen
        if self.windows.len() == 1 {
            if let Some(window) = self.windows.values_mut().next() {
                window.width = width;
                window.height = height.saturating_sub(self.reserved_rows); // Reserve for status/command line
            }
        } else {
            // TODO: Implement smart resizing for multiple windows
            self.resize_windows_to_fill_space();
        }
    }

    // Window resizing methods
    pub fn resize_current_window_wider(&mut self, amount: u16) -> bool {
        let current_id = match self.current_window_id {
            Some(id) => id,
            None => return false,
        };

        if let Some(current_window) = self.windows.get_mut(&current_id) {
            // Find windows to the right that we can shrink
            let mut windows_to_shrink = Vec::new();
            let right_edge = current_window.x + current_window.width;

            for window in self.windows.values() {
                if window.id != current_id && window.x == right_edge {
                    windows_to_shrink.push(window.id);
                }
            }

            if !windows_to_shrink.is_empty() && amount > 0 {
                // Check if we can shrink the right windows by the requested amount
                let mut can_shrink = true;
                for &window_id in &windows_to_shrink {
                    if let Some(window) = self.windows.get(&window_id)
                        && window.width <= amount
                    {
                        can_shrink = false;
                        break;
                    }
                }

                if can_shrink {
                    // Expand current window
                    if let Some(current_window) = self.windows.get_mut(&current_id) {
                        current_window.width += amount;
                    }

                    // Shrink and move right windows
                    for &window_id in &windows_to_shrink {
                        if let Some(window) = self.windows.get_mut(&window_id) {
                            window.width -= amount;
                            window.x += amount;
                        }
                    }
                    return true;
                }
            }
        }
        false
    }

    pub fn resize_current_window_narrower(&mut self, amount: u16) -> bool {
        let current_id = match self.current_window_id {
            Some(id) => id,
            None => return false,
        };

        if let Some(current_window) = self.windows.get(&current_id) {
            if current_window.width <= amount {
                return false; // Can't shrink below minimum size
            }

            let current_right = current_window.x + current_window.width;

            // Find windows to the right that we can expand
            let mut windows_to_expand = Vec::new();
            for window in self.windows.values() {
                if window.id != current_id && window.x == current_right {
                    windows_to_expand.push(window.id);
                }
            }

            if !windows_to_expand.is_empty() {
                // Shrink current window
                if let Some(current_window) = self.windows.get_mut(&current_id) {
                    current_window.width -= amount;
                }

                // Expand and move right windows
                for &window_id in &windows_to_expand {
                    if let Some(window) = self.windows.get_mut(&window_id) {
                        window.width += amount;
                        window.x -= amount;
                    }
                }
                return true;
            }
        }
        false
    }

    pub fn resize_current_window_taller(&mut self, amount: u16) -> bool {
        let current_id = match self.current_window_id {
            Some(id) => id,
            None => return false,
        };

        if let Some(current_window) = self.windows.get_mut(&current_id) {
            // Find windows below that we can shrink
            let mut windows_to_shrink = Vec::new();
            let bottom_edge = current_window.y + current_window.height;

            for window in self.windows.values() {
                if window.id != current_id && window.y == bottom_edge {
                    windows_to_shrink.push(window.id);
                }
            }

            if !windows_to_shrink.is_empty() && amount > 0 {
                // Check if we can shrink the bottom windows by the requested amount
                let mut can_shrink = true;
                for &window_id in &windows_to_shrink {
                    if let Some(window) = self.windows.get(&window_id)
                        && window.height <= amount
                    {
                        can_shrink = false;
                        break;
                    }
                }

                if can_shrink {
                    // Expand current window
                    if let Some(current_window) = self.windows.get_mut(&current_id) {
                        current_window.height += amount;
                    }

                    // Shrink and move bottom windows
                    for &window_id in &windows_to_shrink {
                        if let Some(window) = self.windows.get_mut(&window_id) {
                            window.height -= amount;
                            window.y += amount;
                        }
                    }
                    return true;
                }
            }
        }
        false
    }

    pub fn resize_current_window_shorter(&mut self, amount: u16) -> bool {
        let current_id = match self.current_window_id {
            Some(id) => id,
            None => return false,
        };

        if let Some(current_window) = self.windows.get(&current_id) {
            if current_window.height <= amount {
                return false; // Can't shrink below minimum size
            }

            let current_bottom = current_window.y + current_window.height;

            // Find windows below that we can expand
            let mut windows_to_expand = Vec::new();
            for window in self.windows.values() {
                if window.id != current_id && window.y == current_bottom {
                    windows_to_expand.push(window.id);
                }
            }

            if !windows_to_expand.is_empty() {
                // Shrink current window
                if let Some(current_window) = self.windows.get_mut(&current_id) {
                    current_window.height -= amount;
                }

                // Expand and move bottom windows
                for &window_id in &windows_to_expand {
                    if let Some(window) = self.windows.get_mut(&window_id) {
                        window.height += amount;
                        window.y -= amount;
                    }
                }
                return true;
            }
        }
        false
    }

    fn resize_windows_to_fill_space(&mut self) {
        // Simple implementation: if only one window left, make it fill the screen
        if self.windows.len() == 1
            && let Some(window) = self.windows.values_mut().next()
        {
            window.x = 0;
            window.y = 0;
            window.width = self.terminal_width;
            window.height = self.terminal_height.saturating_sub(self.reserved_rows);
        }
        // TODO: More sophisticated window management for multiple windows
    }

    /// Expand the right-most window to reclaim any horizontal gap left after a window was removed.
    pub fn fill_horizontal_gaps(&mut self) {
        if self.windows.is_empty() {
            return;
        }
        // Find maximum right edge among existing windows
        let mut right_most_id = None;
        let mut max_right: u16 = 0;
        for w in self.windows.values() {
            let right_edge = w.x.saturating_add(w.width);
            if right_edge > max_right {
                max_right = right_edge;
                right_most_id = Some(w.id);
            }
        }
        if max_right < self.terminal_width
            && let Some(rid) = right_most_id
            && let Some(w) = self.windows.get_mut(&rid)
        {
            let delta = self.terminal_width.saturating_sub(max_right);
            w.width = w.width.saturating_add(delta);
        }
    }

    pub fn window_count(&self) -> usize {
        self.windows.len()
    }

    /// Set the number of reserved rows at the bottom and resize single-window layouts
    pub fn set_reserved_rows(&mut self, reserved_rows: u16) {
        self.reserved_rows = reserved_rows;
        // Adjust layout for simple cases
        if self.windows.len() == 1 {
            if let Some(window) = self.windows.values_mut().next() {
                window.height = self.terminal_height.saturating_sub(self.reserved_rows);
                window.width = self.terminal_width;
                window.x = 0;
                window.y = 0;
            }
        } else {
            // For multiple windows, keep positions but ensure boundaries don't exceed new area
            // A more sophisticated reflow can be implemented later
            // For now, clamp any windows that overflow
            let max_height = self.terminal_height.saturating_sub(self.reserved_rows);
            for window in self.windows.values_mut() {
                if window.y + window.height > max_height {
                    window.height = max_height.saturating_sub(window.y);
                }
            }
        }
    }
}
