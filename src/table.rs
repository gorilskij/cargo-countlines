use std::cmp::Ordering;

use tabled::{
    builder::Builder,
    settings::{Alignment, Style, object::Segment, style::HorizontalLine},
};

use crate::{
    count::{Counts, OutputCounts},
    languages::Languages,
    util::format_number,
};

fn sort_counts(output: &OutputCounts) -> Vec<(usize, &Counts)> {
    let mut sorted_counts = output
        .counts
        .iter()
        .map(|(lang_id, counts)| (*lang_id, counts))
        .collect::<Vec<_>>();

    // reverse order by number of code lines, forward order by language
    sorted_counts.sort_unstable_by(|(lang_id1, counts1), (lang_id2, counts2)| {
        match counts2.code.cmp(&counts1.code) {
            Ordering::Equal => lang_id1.cmp(lang_id2),
            ord => ord,
        }
    });
    sorted_counts
}

pub fn make_table(output: &OutputCounts, languages: &Languages) -> String {
    let sorted_counts = sort_counts(output);

    let mut builder = Builder::default();

    builder.push_record(["", "files", "code", "comment", "blank", "invalid"]);

    let mut total_files = 0;
    let mut total_code = 0;
    let mut total_comment = 0;
    let mut total_blank = 0;
    let mut total_invalid = 0;
    for &(lang_id, counts) in &sorted_counts {
        builder.push_record([
            languages[lang_id].name.clone(),
            format_number(counts.files),
            format_number(counts.code),
            format_number(counts.comment),
            format_number(counts.blank),
            format_number(counts.invalid),
        ]);

        total_files += counts.files;
        total_code += counts.code;
        total_comment += counts.comment;
        total_blank += counts.blank;
        total_invalid += counts.invalid;
    }

    builder.push_record([
        "Total".to_string(),
        format_number(total_files),
        format_number(total_code),
        format_number(total_comment),
        format_number(total_blank),
        format_number(total_invalid),
    ]);

    let mut table = builder.build();
    table.modify(Segment::new(1.., 1..), Alignment::right());
    // if there are no files, don't add the second internal hline as it makes
    // the bottom of the table look wrong
    if sorted_counts.is_empty() {
        table.with(Style::rounded());
    } else {
        table.with(Style::rounded().horizontals([
            (1, HorizontalLine::inherit(Style::modern_rounded())),
            (
                sorted_counts.len() + 1,
                HorizontalLine::inherit(Style::modern_rounded()),
            ),
        ]));
    }

    format!("{table}")
}
