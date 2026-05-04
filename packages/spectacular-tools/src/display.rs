use anstyle::Style;

pub(crate) fn paint(style: Style, value: impl AsRef<str>) -> String {
    format!("{style}{}{style:#}", value.as_ref())
}
