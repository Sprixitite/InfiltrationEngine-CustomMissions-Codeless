use std::io;
use console::{style, Term};

use crate::cmterm::input::_InputData;

use super::log::Log;
use super::input::Input;

/// Fits a string s into length l by either truncating the string, or appending char ch repeatedly
pub fn string_to_len(s: impl AsRef<str>, l: usize, ch: char) -> String {
    return console::pad_str_with(
        s.as_ref(),
        l,
        console::Alignment::Left,
        Some("..."), 
        ch
    ).into_owned();
}

pub trait Renderable {
    fn get_log_bufs(&self) -> Vec<&Log>;
    fn get_input_handler(&self) -> &Input;
}

pub struct Renderer;

impl Renderer {
    fn log_lines(&self, log: &Log, rows: usize, columns: usize) -> Vec<String> {
        let log_data = log.data.lock().unwrap();

        let mut lines = log_data.lines.peek_last_n(rows);
        lines.reverse();

        let lines = lines.iter().map(|s| {
            return format!("│ {} │", string_to_len(s, columns-4, ' '))
        }).collect();

        return lines;
    }

    fn log_widths(&self, rendering: &[&Log], columns: usize) -> Box<[usize]> {
        let mut widths = Vec::<usize>::new();
        widths.reserve_exact(rendering.len());

        let width_equal = columns / rendering.len();
        let mut width_remainder = columns % rendering.len();

        for _ in 0..rendering.len() {
            let mut log_size = width_equal;
            if width_remainder > 0 {
                log_size += 1;
                width_remainder -= 1;
            }

            widths.push(log_size);
        }

        return widths.into_boxed_slice();
    }

    fn all_log_lines(&self, rendering: &[&Log], rows: usize, columns: usize) -> Vec<String> {
        let log_columns = self.log_widths(rendering, columns);
        let mut all_log_lines = Vec::<Vec<String>>::with_capacity(rendering.len());
        let mut all_lines = Vec::<String>::with_capacity(rows);

        let mut i = 0;
        for log in rendering {
            all_log_lines.push(self.log_lines(&log, rows, log_columns[i]) );
            i += 1;
        }

        for i in 0..rows {
            let mut full_line_size = rendering.len() - 1;
            for j in 0..rendering.len() {
                full_line_size += all_log_lines[j][i].len()
            }

            let mut full_line = String::with_capacity(full_line_size);
            for j in 0..rendering.len() {
                full_line.push_str(&all_log_lines[j][i]);
            }

            all_lines.push(full_line);
        }

        return all_lines;
    }

    fn log_header(&self, rendering: &[&Log], columns: usize) -> String {
        let log_columns = self.log_widths(rendering, columns);
        let mut header_line = String::with_capacity(columns+1);

        let mut i = 0;
        for s in log_columns {
            let log = &rendering[i];
            let log_data = log.data.lock().unwrap();

            let log_header = format!(
                "╭{}─╮",
                string_to_len(
                    style(
                        format!(" {} Log ", log_data.title)
                    ).bold().to_string(),
                    s-3,
                    '─'
                )
            );

            header_line.push_str(&log_header);
            i += 1;
        }
        header_line.push('\n');

        return header_line;
    }

    fn log_footer(&self, rendering: &[&Log], columns: usize) -> String {
        let log_columns = self.log_widths(rendering, columns);
        let mut footer_line = String::with_capacity(columns+1);

        for s in log_columns {
            let log_header = format!(
                "╰{}╯",
                string_to_len(
                    "",
                    s-2,
                    '─'
                )
            );

            footer_line.push_str(&log_header);
        }
        footer_line.push('\n');

        return footer_line
    }

    fn input_box(&self, inputting: bool, input_data: &_InputData, columns: usize) -> String {
        let input_prompt = &input_data.input_prompt;
        let input_buffer = &input_data.input_buffer;
        let input_thread = &input_data.input_requester;

        let input_title = match inputting {
            false => String::from(" Input "),
            true  => format!(" Input ({}) ", input_thread)
        };
        let input_title = style(input_title).bold().to_string();

        let input_header  = format!(
            "╭{}─╮", 
            string_to_len(
                input_title,
                columns-3,
                '─'
            )
        );
        let input_content = match inputting {
            false => format!(
                "│ {} │",
                style(string_to_len("", columns-4, '/')).dim().to_string()
            ),
            true  => format!(
                "│ {} │", 
                string_to_len(
                    format!(
                        "{}{}",
                        style(input_prompt).bold().to_string(),
                        input_buffer
                    ),
                    columns-4,
                    ' '
                )
            )
        };
        let input_footer = format!("╰{}╯", string_to_len("", columns-2, '─'));

        let mut inputbox_final = String::with_capacity(input_header.len()+input_content.len()+input_footer.len()+2);
        inputbox_final.push_str(&input_header);
        inputbox_final.push('\n');
        inputbox_final.push_str(&input_content);
        inputbox_final.push('\n');
        inputbox_final.push_str(&input_footer);

        return inputbox_final;
    }

    pub fn render(&self, term: &Term, rendering: &impl Renderable) -> io::Result<()> {
        let logs = rendering.get_log_bufs();

        let input_handler = rendering.get_input_handler();

        // let input_handler = {
        //     let mut inputter: Option<&Input> = None;

        //     for handler in rendering.get_input_handlers() {
        //         inputter = Some(handler);
        //         let handler_in_use = cmterm::mutex::mutex_in_use(&handler.inputting);
        //         let handler_valid = { *handler.termread_valid.lock().unwrap() };
        //         if handler_in_use && handler_valid {
        //             break;
        //         }
        //     }

        //     inputter
        // };

        // let input_handler = match input_handler {
        //     Some(i) => i,
        //     None => panic!("No inputter provided")
        // };

        term.move_cursor_to(0, 0)?;
        term.clear_to_end_of_screen()?;
        let (rows, columns) = term.size();
        let rows = rows as usize;
        let columns = columns as usize;
        
        let header = self.log_header(&logs, columns);
        let footer = self.log_footer(&logs, columns);

        let mut content = String::with_capacity((columns+1)*(rows-5));

        for l in self.all_log_lines(&logs, rows-5, columns) {
            content.push_str(&l);
        }

        // Lock disposed of when exiting render function
        let render_input_data = { input_handler.input_state.lock().unwrap().clone() };
        let input = self.input_box(input_handler.is_inputting(), &render_input_data, columns);

        let mut term_str = String::with_capacity(header.len()+content.len());
        term_str.push_str(&header);
        term_str.push_str(&content);
        term_str.push_str(&footer);
        term_str.push_str(&input);

        term.write_str(&term_str)?;

        match input_handler.is_inputting() {
            false => term.hide_cursor()?,
            true => {
                term.show_cursor()?;
                term.move_cursor_to(render_input_data.input_pos+2+render_input_data.input_prompt.len(), rows-2)?;
            }
        }

        return Ok(());
    }
}