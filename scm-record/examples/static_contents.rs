#![warn(clippy::all, clippy::as_conversions)]
#![allow(clippy::too_many_arguments, clippy::blocks_in_if_conditions)]

use std::borrow::Cow;
use std::path::Path;

use scm_record::{
    ChangeType, EventSource, File, RecordError, RecordState, Recorder, Section, SectionChangedLine,
};

fn main() {
    let files = vec![
        File {
            path: Cow::Borrowed(Path::new("foo/bar")),
            file_mode: None,
            sections: vec![
                Section::Unchanged {
                    lines: std::iter::repeat(Cow::Borrowed("this is some text\n"))
                        .take(20)
                        .collect(),
                },
                Section::Changed {
                    lines: vec![
                        SectionChangedLine {
                            is_toggled: true,
                            change_type: ChangeType::Removed,
                            line: Cow::Borrowed("before text 1\n"),
                        },
                        SectionChangedLine {
                            is_toggled: true,
                            change_type: ChangeType::Removed,
                            line: Cow::Borrowed("before text 2\n"),
                        },
                        SectionChangedLine {
                            is_toggled: true,
                            change_type: ChangeType::Added,

                            line: Cow::Borrowed("after text 1\n"),
                        },
                        SectionChangedLine {
                            is_toggled: false,
                            change_type: ChangeType::Added,
                            line: Cow::Borrowed("after text 2\n"),
                        },
                    ],
                },
                Section::Unchanged {
                    lines: vec![Cow::Borrowed("this is some trailing text\n")],
                },
            ],
        },
        File {
            path: Cow::Borrowed(Path::new("baz")),
            file_mode: None,
            sections: vec![
                Section::Unchanged {
                    lines: vec![
                        Cow::Borrowed("Some leading text 1\n"),
                        Cow::Borrowed("Some leading text 2\n"),
                    ],
                },
                Section::Changed {
                    lines: vec![
                        SectionChangedLine {
                            is_toggled: true,
                            change_type: ChangeType::Removed,
                            line: Cow::Borrowed("before text 1\n"),
                        },
                        SectionChangedLine {
                            is_toggled: true,
                            change_type: ChangeType::Removed,
                            line: Cow::Borrowed("before text 2\n"),
                        },
                        SectionChangedLine {
                            is_toggled: true,
                            change_type: ChangeType::Added,
                            line: Cow::Borrowed("after text 1\n"),
                        },
                        SectionChangedLine {
                            is_toggled: true,
                            change_type: ChangeType::Added,
                            line: Cow::Borrowed("after text 2\n"),
                        },
                    ],
                },
                Section::Unchanged {
                    lines: vec![Cow::Borrowed("this is some trailing text")],
                },
            ],
        },
    ];
    let record_state = RecordState { files };

    let recorder = Recorder::new(record_state, EventSource::Crossterm);
    let result = recorder.run();
    match result {
        Ok(result) => {
            let RecordState { files } = result;
            for file in files {
                println!("--- Path {:?} final lines: ---", file.path);
                let (selected, _unselected) = file.get_selected_contents();
                print!("{selected}");
            }
        }
        Err(RecordError::Cancelled) => println!("Cancelled!\n"),
        Err(err) => {
            println!("Error: {err}");
        }
    }
}
